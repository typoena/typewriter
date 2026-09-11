//! Over-the-air firmware update — the logic behind the editor's `:update`.
//!
//! Runs on the net thread (see [`crate::infrastructure::net`]), which already
//! owns the Wi-Fi modem and has brought the radio + clock up via `ensure_online`
//! before calling in here. No libgit2, no git: just esp-idf's HTTPS client and
//! the A/B OTA machinery ([`EspOta`]). Two GETs against a plain-text manifest —
//! deliberately no JSON parser on-device:
//!
//! ```text
//!   <base>/latest              → the newest release's semver, one line
//!   <base>/typoena-<ver>.bin   → that release's app image
//! ```
//!
//! The image streams straight into the **inactive** OTA slot; the running slot
//! is never touched, so a mid-download power loss is safe — the next boot still
//! runs the current image. Only on a clean `complete()` does the new slot become
//! the boot target; the caller then reboots into it, and the boot path
//! ([`mark_running_firmware_valid`]) self-tests and confirms it, or the
//! bootloader rolls back here (needs `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE`).

use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use embedded_svc::http::Method;
use esp_idf_svc::http::client::{
    Configuration as HttpConfig, EspHttpConnection, FollowRedirectsPolicy,
};
use esp_idf_svc::ota::{EspOta, SlotState};

use app::Phase;

/// The running firmware's semantic version, baked from the crate version so a
/// bump in `firmware/Cargo.toml` is the single source of truth the update check
/// compares against. Also stamped into the app descriptor for the bootloader
/// (see `CONFIG_APP_PROJECT_VER` in `sdkconfig.defaults`).
pub const FW_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where release artifacts live (no trailing slash). Baked at build time from
/// `TW_UPDATE_BASE_URL` when set, else the public site. A later step can move
/// this to `typoena.conf` alongside the other provisioning fields.
const DEFAULT_UPDATE_BASE_URL: &str = "https://typoena.dev/firmware";

fn update_base_url() -> &'static str {
    option_env!("TW_UPDATE_BASE_URL").unwrap_or(DEFAULT_UPDATE_BASE_URL)
}

/// Ask the release manifest whether a newer image exists. Reads one short URL
/// and writes nothing — no download, no OTA slot touched — so it is safe to run
/// unprompted; `install` is the half that earns a confirm.
///
/// Returns `Ok(Some(version))` when the release is newer than the running image,
/// `Ok(None)` when the device is already current, and `Err` on a transport
/// failure. The slot probe lives here rather than in `install` so a device with
/// nowhere to put an image says so in a second, instead of offering an install
/// it could only refuse.
pub fn check_for_update() -> Result<Option<String>> {
    let latest = fetch_latest_version().context("checking the latest release")?;
    log::info!("OTA — running {FW_VERSION}, latest available {latest}");

    if !is_newer(&latest, FW_VERSION) {
        return Ok(None);
    }

    if !has_update_slot() {
        bail!("no OTA slot - reflash over USB");
    }

    Ok(Some(latest))
}

/// Download `version` into the inactive OTA slot and make it the boot target
/// (`complete()`). The caller reboots into it.
///
/// Takes the version the check reported rather than re-reading the manifest, so
/// the image installed is the one the writer said yes to. On `Err` the running
/// slot is untouched and the device keeps booting the current image.
pub fn install_update(version: &str, progress: &dyn Fn(Phase)) -> Result<()> {
    let url = format!("{}/typoena-{version}.bin", update_base_url());
    let written =
        download_and_install(&url, progress).with_context(|| format!("installing {version}"))?;
    log::info!("OTA — installed {version} ({written} bytes); new slot is the boot target");
    Ok(())
}

/// Whether this device has a second app slot to receive an update.
///
/// `:update` needs the dual A/B layout (`partitions-ota.csv`). A device still on
/// the single `factory` partition — what the dev `just flash` path writes — has
/// nowhere to put a new image, and it cannot grow a slot while running: that is
/// the one-time USB migration `just flash-ota` exists for. esp-idf only
/// discovers the absence at `esp_ota_begin`, i.e. AFTER a ~2.5 MB download, so
/// we probe up front and fail in a second with something actionable.
fn has_update_slot() -> bool {
    // Null `configured` asks esp-idf to pick the slot itself; NULL back means
    // the partition table has no ota_0/ota_1 at all.
    !unsafe { esp_idf_svc::sys::esp_ota_get_next_update_partition(core::ptr::null()) }.is_null()
}

/// GET `<base>/latest` and return the trimmed version line. The manifest is
/// a single short token, so a small bounded read is enough (and caps a bad URL
/// that returns HTML from flooding the heap).
fn fetch_latest_version() -> Result<String> {
    let url = format!("{}/latest", update_base_url());
    let mut conn = http_get(&url)?;
    let status = conn.status();
    if status != 200 {
        bail!("version manifest {url} → HTTP {status}");
    }
    // Drain up to a small cap; the manifest is one line like "0.8.0".
    let mut body = Vec::new();
    let mut buf = [0u8; 64];
    loop {
        let n = conn.read(&mut buf).context("reading version manifest")?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(buf.get(..n).unwrap_or_default());
        if body.len() > 256 {
            bail!("version manifest at {url} is not a short version line");
        }
    }
    let version = core::str::from_utf8(&body)
        .context("version manifest is not UTF-8")?
        .trim()
        .to_string();
    if version.is_empty() {
        bail!("empty version manifest at {url}");
    }
    Ok(version)
}

/// Stream the image at `url` into the inactive OTA slot and finalize it as the
/// boot target. Returns the byte count written. On any error the in-progress
/// `EspOtaUpdate` is dropped, which aborts the write — the running slot and the
/// boot pointer are left untouched.
fn download_and_install(url: &str, progress: &dyn Fn(Phase)) -> Result<usize> {
    let mut conn = http_get(url)?;
    let status = conn.status();
    if status != 200 {
        bail!("firmware image {url} → HTTP {status}");
    }

    let mut ota = EspOta::new().context("opening the OTA subsystem")?;
    // Erases the inactive slot and begins the write (esp_ota_begin).
    let mut update = ota
        .initiate_update()
        .context("beginning the OTA write (erase inactive slot)")?;

    let mut buf = [0u8; 4096];
    let mut written = 0usize;
    // Time-gated, not chunk-gated: a ~2 MB image is ~500 of these 4 KB reads, and
    // one panel line per chunk would spend the whole ghosting budget on a byte
    // counter (same rule as the sync counters — see `app::Phase`).
    let mut last_line: Option<Instant> = None;
    loop {
        let n = conn.read(&mut buf).context("reading firmware image body")?;
        if n == 0 {
            break;
        }
        update.write(buf.get(..n).unwrap_or_default()).context("writing the OTA slot")?;
        written += n;
        if last_line.is_none_or(|t: Instant| t.elapsed() >= Duration::from_secs(2)) {
            last_line = Some(Instant::now());
            progress(Phase::InstallingImage { kb: written / 1024 });
        }
    }

    if written == 0 {
        bail!("firmware image {url} was empty");
    }
    // esp_ota_end (validates the image) + esp_ota_set_boot_partition.
    update
        .complete()
        .context("finalizing the image (validate + set boot slot)")?;
    Ok(written)
}

/// Open an HTTPS GET, validating the server chain against the bundled roots (the
/// same CA bundle the git push uses) and following redirects (a release asset
/// may 302 to a CDN). Returns the connection with response headers already read,
/// ready for `status()` + `read()`. Mirrors the proven flow in the `wifi_tls`
/// spike (`initiate_request` → `initiate_response`).
fn http_get(url: &str) -> Result<EspHttpConnection> {
    let mut conn = EspHttpConnection::new(&HttpConfig {
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        follow_redirects_policy: FollowRedirectsPolicy::FollowAll,
        buffer_size: Some(4096),
        ..Default::default()
    })
    .context("creating the HTTPS connection (TLS init)")?;

    // GitHub (and most hosts) reject requests without a User-Agent.
    conn.initiate_request(Method::Get, url, &[("User-Agent", "typoena-ota")])
        .context("TLS handshake / request send failed")?;
    conn.initiate_response().context("reading response headers")?;
    Ok(conn)
}

/// True if `candidate` is a strictly higher semantic version than `current`.
/// Compares the dotted numeric components left to right; a leading `v` and any
/// pre-release/build suffix (`-`/`+`) are ignored, and an unparseable component
/// sorts as 0 — so a malformed manifest never reads as an upgrade over a valid
/// running version.
fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> [u64; 3] {
        let core = v.trim().trim_start_matches('v');
        let core = core.split(['-', '+']).next().unwrap_or(core);
        let mut out = [0u64; 3];
        for (slot, seg) in out.iter_mut().zip(core.split('.')) {
            *slot = seg.trim().parse().unwrap_or(0);
        }
        out
    }
    parts(candidate) > parts(current)
}

/// Whether the running image is still in the bootloader's pending-verify state
/// — i.e. this is the first boot after an OTA `:update` and nothing has
/// confirmed the image yet.
///
/// Read at boot, before the splash, so the panel can warn that a reset right now
/// rolls the device back (see `Frame::confirming_update`). Any failure to read
/// the slot answers `false`: the warning is a courtesy, and a boot must never
/// hinge on it. Goes `false` for the rest of the session once
/// [`mark_running_firmware_valid`] confirms the slot.
pub fn running_slot_pending_verify() -> bool {
    EspOta::new()
        .and_then(|ota| ota.get_running_slot())
        .map(|slot| slot.state == SlotState::Unverified)
        .unwrap_or(false)
}

/// Confirm the running firmware is healthy so the bootloader keeps it.
///
/// With rollback enabled (`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE`), an image
/// installed over the air boots in the *pending-verify* state and is rolled back
/// on the next reset UNLESS it marks itself valid. Reaching this call means boot
/// got all the way to cursor-ready — SD mounted, note loaded, panel painting,
/// input running — which is our self-test bar, so we confirm.
///
/// We call `mark_running_slot_valid` ONLY when the running slot is actually in
/// the pending-verify (`Unverified`) state. A USB-flashed image (`just flash-ota`
/// / `just ship`) has empty otadata and reads back as `Unknown`/`Factory`;
/// marking-valid there is a no-op in esp-idf, but it logs `esp_ota_ops: Running
/// firmware is factory` at ERROR level on *every* such boot (until the first
/// OTA) — alarming for a freshly-shipped unit. Gating on `Unverified` keeps that
/// noisy path out of the normal boot log while still confirming a real OTA image.
pub fn mark_running_firmware_valid() {
    let mut ota = match EspOta::new() {
        Ok(ota) => ota,
        Err(e) => {
            log::debug!("OTA — skipping mark-valid; opening the OTA subsystem failed ({e})");
            return;
        }
    };
    match ota.get_running_slot().map(|slot| slot.state) {
        Ok(SlotState::Unverified) => match ota.mark_running_slot_valid() {
            Ok(()) => log::info!("OTA — running slot confirmed valid (rollback cancelled)"),
            Err(e) => {
                log::warn!("OTA — could not confirm running slot ({e}); it may roll back on reset")
            }
        },
        Ok(state) => log::debug!("OTA — running slot is {state:?}; nothing to confirm"),
        Err(e) => log::debug!("OTA — could not read running slot state ({e})"),
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn strictly_higher_is_an_upgrade() {
        assert!(is_newer("0.8.0", "0.7.7"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.7.8", "0.7.7"));
    }

    #[test]
    fn same_or_lower_is_not() {
        assert!(!is_newer("0.7.7", "0.7.7"));
        assert!(!is_newer("0.7.6", "0.7.7"));
        assert!(!is_newer("0.6.9", "0.7.0"));
    }

    #[test]
    fn tolerates_v_prefix_and_suffixes() {
        assert!(is_newer("v0.8.0", "0.7.7"));
        assert!(is_newer("0.8.0-rc1", "0.7.7"));
        assert!(!is_newer("0.7.7+build9", "0.7.7"));
    }

    #[test]
    fn a_garbage_manifest_never_upgrades() {
        assert!(!is_newer("garbage", "0.7.7"));
        assert!(!is_newer("", "0.0.1"));
    }
}
