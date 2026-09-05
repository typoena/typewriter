//! Shared networking helpers: joining the AP, and setting the wall clock over
//! SNTP once we are on it.
//!
//! Every caller that touches the radio needs both, in that order — the net
//! thread's `:gs`/`:gl`/`:update`/`:inbox` cycles, the onboarding wizard, and the
//! `wifi_tls` bench bin — so both live here rather than once per caller. Neither
//! is feature-gated: `wifi_tls` builds without libgit2.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::sntp::{EspSntp, SyncStatus};
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

/// Association + DHCP attempts before giving up. The first attempt after a
/// reset frequently fails: the AP may still hold the pre-reset association and
/// reject the re-join until it ages out, the radio may not be settled, and DHCP
/// can drop the first DISCOVER on a cold interface. Retrying a handful of times
/// turns those transient misses into a clean connect on attempt 2–3 instead of
/// a failed boot.
const MAX_ATTEMPTS: u32 = 5;

/// Backoff before the first retry; doubles each attempt up to [`MAX_BACKOFF_MS`].
const INITIAL_BACKOFF_MS: u32 = 500;
const MAX_BACKOFF_MS: u32 = 4000;

/// Configure the station, start the radio, and associate with `ssid`, retrying
/// association + DHCP with exponential backoff.
///
/// `set_configuration` and `start` run once; only the association + netif-up
/// wait is retried — restarting the radio each attempt is wasteful and can wedge
/// the driver. Between attempts the station is disconnected to clear any
/// half-open state before the next `connect`.
///
/// An empty `pass` selects an open network; otherwise WPA2-Personal.
pub fn connect_wifi(
    wifi: &mut BlockingWifi<EspWifi<'_>>,
    ssid: &str,
    pass: &str,
) -> Result<()> {
    let auth_method = if pass.is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().ok().context("SSID > 32 bytes")?,
        password: pass.try_into().ok().context("password > 64 bytes")?,
        auth_method,
        ..Default::default()
    }))?;
    wifi.start()?;

    let mut backoff_ms = INITIAL_BACKOFF_MS;
    let mut attempt = 1;
    loop {
        log::info!("associating with \"{ssid}\" (attempt {attempt}/{MAX_ATTEMPTS})…");
        match associate_once(wifi) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < MAX_ATTEMPTS => {
                log::warn!("Wi-Fi attempt {attempt} failed: {e:#}; retrying in {backoff_ms} ms");
                // Clear any half-open association before the next connect. Ignore
                // the result — it errors harmlessly when we never associated.
                let _ = wifi.disconnect();
                FreeRtos::delay_ms(backoff_ms);
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                attempt += 1;
            }
            Err(e) => {
                return Err(e).with_context(|| format!("Wi-Fi failed after {MAX_ATTEMPTS} attempts"));
            }
        }
    }
}

/// One association + DHCP wait. Split out so the retry loop reads cleanly and to
/// keep the two `wifi` borrows in separate statements.
fn associate_once(wifi: &mut BlockingWifi<EspWifi<'_>>) -> Result<()> {
    wifi.connect().context("Wi-Fi association failed")?;
    wifi.wait_netif_up().context("DHCP / netif never came up")?;
    Ok(())
}

/// SNTP first-sync budget. Home networks resolve pool.ntp.org and answer well
/// within this; failing past it is a real problem worth surfacing rather than
/// waiting out.
pub const SNTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Kick off SNTP and block until the wall clock is real. Must run on the thread
/// that owns the radio, after the AP join.
///
/// Everything downstream depends on it: TLS checks cert validity against wall
/// time, a git commit signs a timestamp, and `:inbox` dates the fleeting note —
/// there is no battery-backed RTC, so the clock boots at the epoch every power
/// cycle. A `Completed` status alone is not enough (it can settle on a clock
/// that never moved), so the seconds are re-checked against a date safely in the
/// past before we call it synced.
pub fn sync_clock() -> Result<()> {
    let sntp = EspSntp::new_default()?;
    log::info!("SNTP started, waiting for first sync…");
    let start = Instant::now();
    while sntp.get_sync_status() != SyncStatus::Completed {
        if start.elapsed() >= SNTP_TIMEOUT {
            bail!("SNTP did not sync within {SNTP_TIMEOUT:?}");
        }
        FreeRtos::delay_ms(100);
    }
    let unix = now_unix();
    // 2023-11-14. Anything below means the clock never actually advanced, and
    // TLS would reject on validity while a commit would carry a 1970 date.
    if unix < 1_700_000_000 {
        bail!("clock still at {unix} after SNTP — refusing TLS/commit with a bad wall clock");
    }
    log::info!("clock synced — unix {unix}");
    Ok(())
}

/// Current wall-clock seconds since the Unix epoch (meaningful after
/// [`sync_clock`]).
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
