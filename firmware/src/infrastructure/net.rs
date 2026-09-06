//! The net thread — the sole owner of the Wi-Fi modem, and the transport behind
//! everything the editor does over the wire: git push (`:gs`) and pull
//! (`:gl`), firmware update (`:update`, whose logic lives in the sibling
//! [`crate::infrastructure::ota`] module — this file only dispatches to it), and
//! the clock-only sync that dates a fleeting note ([`NetRequest::Clock`]).
//! One thread because there is one radio the editor loop can never reclaim, so
//! they all multiplex over a single [`NetRequest`]/[`NetOutcome`] channel and
//! back the app's [`app::NetService`] port. Most of what follows is the git
//! machinery; the OTA hop is [`NetRequest::Update`] → [`update_cycle`].
//!
//! Graduated from the `src/bin/git_sync.rs` spike (milestone #2A, hardware-
//! verified 2026-07-07). The spike proved `open` + fast-forward `push` over
//! mbedTLS HTTPS+PAT against a persistent clone; this module lifts that logic
//! into a service the editor drives, with three changes for the product:
//!
//! 1. **Storage is the SD card `/sd/repo`** (the same working copy the editor
//!    saves `notes.md` into via [`crate::infrastructure::storage_sd`]), not the spike's 4 MB
//!    flash-FAT `/spiflash/repo`. The real notes repo can't fit in flash, so the
//!    card is the only viable home — and there's a single source of truth: git
//!    commits the exact file the editor just wrote. The net thread reaches the
//!    card through plain `std::fs`; FatFS's per-volume reentrancy lock serialises
//!    it against the UI task's saves (see [`crate::infrastructure::storage_sd::Storage`]).
//! 2. **`open` only — never clone-and-wipe.** The spike re-cloned into a
//!    throwaway flash dir; doing that to the user's card would delete their
//!    notes. A `/sd/repo` that isn't a valid repo is a provisioning error
//!    (`just init`), surfaced as such, not papered over.
//! 3. **No synthetic content.** The spike appended a marker line; here the
//!    editor has already saved the user's buffers before `:gs` signals us,
//!    so we just commit + push what's on disk.
//! 4. **The commit is an O(depth) TreeBuilder splice, not an index pass.**
//!    The request carries the repo-relative paths saved/deleted since the last
//!    confirmed push (`Storage`'s journaled dirty set); `stage_and_commit`
//!    patches exactly those onto HEAD's tree. The index pipeline it replaced
//!    (`add_all` → `index.write` → `write_tree`) is O(N_tree) and measured up
//!    to 611 s on the real 1179-file / 570 MB-pack clone — see
//!    docs/tradeoff-curves/sync-commit-staging.md for the whole trail.
//!
//! Runs on a dedicated 96 KB thread (libgit2's init→push chain nests ~67 KB of
//! `GIT_PATH_MAX` stack buffers — see git_push.rs / postmortem #3). Config
//! comes from the card's `/sd/typoena.conf` (installer- or wizard-written,
//! parsed at boot — v0.9 onboarding slice 0), falling back per field to the
//! build-time `TW_*` values (ADR-007's dev path, `firmware/.env`).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use git2::{
    CertificateCheckStatus, Commit, Cred, CredentialType, FetchOptions, ObjectType, Oid,
    PushOptions, RemoteCallbacks, Repository, Signature, Tree,
};

use app::Phase;

use crate::drivers::wifi_esp::{connect_wifi, now_unix, sync_clock};
use crate::infrastructure::sd_log::DIAG;
use crate::infrastructure::storage_sd::{LOCAL_DIR, REPO_DIR};

// Baked in at build time from firmware/.env (see build.rs). Empty when unset.
// These are the per-field FALLBACK: the card's /sd/typoena.conf overrides them,
// so a provisioned card works on a firmware built with an empty .env. A field
// empty in both is caught by the push/pull guards with a clear message.
const BAKED_WIFI_SSID: &str = env!("TW_WIFI_SSID");
const BAKED_WIFI_PASS: &str = env!("TW_WIFI_PASS");
const BAKED_REMOTE_URL: &str = env!("TW_REMOTE_URL");
const BAKED_GH_USER: &str = env!("TW_GH_USER");
const BAKED_TOKEN: &str = env!("TW_TOKEN");
const BAKED_AUTHOR_NAME: &str = env!("TW_AUTHOR_NAME");
const BAKED_AUTHOR_EMAIL: &str = env!("TW_AUTHOR_EMAIL");

/// The card's parsed `typoena.conf`, installed once by `main` after the SD
/// mount and before the net thread spawns. `OnceLock` because the net thread
/// reads it concurrently with the UI thread from then on.
static CARD_CONF: std::sync::OnceLock<conf::Conf> = std::sync::OnceLock::new();

/// Install the card config (once; later calls are ignored).
pub fn set_card_conf(c: conf::Conf) {
    let _ = CARD_CONF.set(c);
}

/// The per-field card-over-baked merge as a value, WITHOUT installing the
/// card conf: the wizard gate needs the effective view before deciding
/// whether to run (and what to prefill), and `set_card_conf` is called once
/// afterwards with the final result. The Wi-Fi password follows the SSID's
/// source, exactly like [`wifi_pass`].
pub fn effective_conf_from(card: &conf::Conf) -> conf::Conf {
    let pick = |v: &str, baked: &'static str| {
        if v.trim().is_empty() {
            baked.to_string()
        } else {
            v.to_string()
        }
    };
    conf::Conf {
        wifi_pass: if card.wifi_ssid.trim().is_empty() {
            BAKED_WIFI_PASS.to_string()
        } else {
            card.wifi_pass.clone()
        },
        wifi_ssid: pick(&card.wifi_ssid, BAKED_WIFI_SSID),
        remote_url: pick(&card.remote_url, BAKED_REMOTE_URL),
        gh_user: pick(&card.gh_user, BAKED_GH_USER),
        token: pick(&card.token, BAKED_TOKEN),
        author_name: pick(&card.author_name, BAKED_AUTHOR_NAME),
        author_email: pick(&card.author_email, BAKED_AUTHOR_EMAIL),
    }
}

/// Card value if present and non-blank, else the baked fallback. `&'static`
/// works because `CARD_CONF` is a static — the parsed strings live forever.
fn cfg(field: conf::Field, baked: &'static str) -> &'static str {
    match CARD_CONF.get() {
        Some(c) if !c.get(field).trim().is_empty() => c.get(field),
        _ => baked,
    }
}

fn wifi_ssid() -> &'static str {
    cfg(conf::Field::WifiSsid, BAKED_WIFI_SSID)
}
/// The pass follows whichever source supplied the SSID: a card SSID with a
/// blank pass is an OPEN NETWORK, not "fall back to the baked pass" — mixing
/// the card's SSID with the .env password of a different network must never
/// happen.
fn wifi_pass() -> &'static str {
    match CARD_CONF.get() {
        Some(c) if !c.wifi_ssid.trim().is_empty() => &c.wifi_pass,
        _ => BAKED_WIFI_PASS,
    }
}
fn remote_url() -> &'static str {
    cfg(conf::Field::RemoteUrl, BAKED_REMOTE_URL)
}
fn gh_user() -> &'static str {
    cfg(conf::Field::GhUser, BAKED_GH_USER)
}
fn token() -> &'static str {
    cfg(conf::Field::Token, BAKED_TOKEN)
}
fn author_name() -> &'static str {
    cfg(conf::Field::AuthorName, BAKED_AUTHOR_NAME)
}
fn author_email() -> &'static str {
    cfg(conf::Field::AuthorEmail, BAKED_AUTHOR_EMAIL)
}

/// GitHub's root CAs, embedded so the push can verify the server's TLS chain.
/// Shared with the spikes. Written to the card and handed to libgit2 via
/// `GIT_OPT_SET_SSL_CERT_LOCATIONS`. Path is relative to this source file —
/// `../bin/` reaches `src/bin/` now that this module lives under `infrastructure/`.
const GITHUB_ROOTS_PEM: &str = include_str!("../bin/github_roots.pem");
/// CA bundle on the card root — outside `/sd/repo`, so it's never staged.
const CA_BUNDLE_PATH: &str = "/sd/ca.pem";

/// Stack for the dedicated net thread. The init→push chain measured ~67 KB;
/// keep the proven 96 KB (see git_push.rs / postmortem #3). Wi-Fi association
/// now also runs here, but it's shallow next to libgit2's path-buffer nesting.
pub const GIT_STACK: usize = 96 * 1024;

/// Cap on libgit2's odb object cache (default max: 256 MB — unbounded on this
/// device). Run 4 (2026-07-13): the real-repo push ground through pack-building
/// for 66 s and PSRAM hit the floor — the UI aborted on a 27 KB framebuffer
/// alloc. Every tree/commit the push's walks read lands in this cache, and
/// nothing bounded it. 1 MB still holds the whole tree set the push's two
/// full-tree walks share (mark-uninteresting over origin's tip, then the insert
/// over ours — near-identical trees), so the second walk stays off the SD card.
const ODB_CACHE_MAX_BYTES: isize = 1024 * 1024;

/// Process-global libgit2 tuning, applied once before any repo work. The 32-bit
/// defaults (32 MB window / 256 MB mapped budget, mwindow.c) would git__malloc
/// past PSRAM on the first pack access — the p_mmap emulation (esp_map.c) makes
/// every window a real PSRAM malloc, so this budget decides whether a
/// push/clone survives. 64 KB / 1.5 MB leaves ~2 MB headroom even with the
/// 5-pack card and shrinks read amplification (a window miss costs a 64 KB SPI
/// read, ~65 ms, not 256 KB). Both the service thread and the onboarding
/// wizard's one-shot clone call this on their own thread before opening a repo;
/// re-applying the same values is harmless.
///
/// SAFETY: set before any Repository is opened on the calling thread.
pub fn tune_libgit2() {
    unsafe {
        if let Err(e) = git2::opts::set_mwindow_size(64 * 1024) {
            log::error!("set_mwindow_size failed ({e}); first pack access may OOM");
        }
        if let Err(e) = git2::opts::set_mwindow_mapped_limit(1536 * 1024) {
            log::error!("set_mwindow_mapped_limit failed ({e}); first pack access may OOM");
        }
        // Bound the OPEN-PACK descriptor count. libgit2's default is 0 =
        // unlimited, so its LRU-close path never runs and every pack the odb
        // touches holds an fd until the Repository drops. Against the shared
        // 16-slot mount (storage_sd::MAX_FILES_GIT) that is the one term of the
        // budget that grows without bound: a downloading `:gl` writes a new pack
        // and the device never repacks.
        //
        // 8, not 4: the eviction this enables runs per pack OPEN, not once per
        // session, so a limit under the card's real pack count (documented at 5,
        // and growing) makes a push's two full-tree walks close and reopen a pack
        // — an SD read plus a fresh 64 KB window — every time objects interleave
        // across them. 8 sits above the observed count and still leaves half the
        // 16-slot pool free. Confirm against the `odb inventory` log line on the
        // bench and raise it if the card carries more.
        // Behaviour-safe — a closed pack is reopened on demand, so the cost is a
        // reopen, not a correctness change.
        if let Err(e) = git2::opts::set_mwindow_file_limit(8) {
            log::error!("set_mwindow_file_limit failed ({e}); packs keep their fds until the repo closes");
        }
        // Odb cache cap (see ODB_CACHE_MAX_BYTES). git2 0.20 wraps only the
        // per-object-type limit, not the total, so this one is a raw call.
        let rc = libgit2_sys::git_libgit2_opts(
            libgit2_sys::GIT_OPT_SET_CACHE_MAX_SIZE as i32,
            ODB_CACHE_MAX_BYTES,
        );
        if rc < 0 {
            log::error!("set cache_max_size failed (rc {rc}); a push/clone may exhaust the heap");
        }
    }
}

/// What the UI task asks the net thread to do.
pub enum NetRequest {
    /// `:gs` — commit the dirty paths and push (the upload half).
    Push(PushRequest),
    /// `:gl` — fetch, then fast-forward or rebase (the download half).
    Pull(PullRequest),
    /// `:update` — check for a newer firmware release and, if one exists, stream
    /// it into the inactive OTA slot over HTTPS. Carries nothing: the running
    /// version and the manifest URL are known to the firmware. Rides this thread
    /// because it already owns the Wi-Fi modem, not because it touches git.
    Update,
    /// Set the wall clock, and nothing else: join the AP and run SNTP, no fetch,
    /// no libgit2, no TLS handshake to the remote. The cheapest thing this thread
    /// does, and the reason `:inbox` can date a note without paying for a pull.
    /// The UI sends one unprompted at boot and one per `:inbox` that finds the
    /// clock still at the epoch.
    Clock,
}

/// A request to push. The UI task has already saved every dirty buffer to
/// the card before sending this; `paths` is `Storage::take_dirty`'s snapshot —
/// the repo-relative paths saved or `:delete`d since the last confirmed
/// push. The working tree stays the source of truth: at commit time a path
/// that exists on the card is spliced into the tree from disk, a missing one
/// is spliced out. An unchanged path is a no-op, so over-reporting is safe.
pub struct PushRequest {
    pub paths: BTreeSet<String>,
}

/// A request to pull. `paths` is `Storage::take_dirty`'s snapshot — the
/// saved-but-unpushed paths — and `discard` decides their fate before the
/// fetch, per the answer given on the unsynced card.
pub struct PullRequest {
    /// The dirty-journal snapshot, repo-relative. Empty for a plain fetch
    /// (nothing was dirty), in which case `discard` is moot.
    pub paths: BTreeSet<String>,
    /// Throw this work away instead of committing it: restore each path from
    /// HEAD and unlink the ones HEAD never had ([`discard_paths`]), so the pull
    /// arrives at a working copy that matches the last sync.
    ///
    /// Irreversible, and the confirmation for it is two deliberate steps up in
    /// the UI (the card names every file, then a y/n names the count). Nothing
    /// down here re-asks.
    pub discard: bool,
}

/// What the net thread reports back, tagged by the request kind so the UI can
/// settle the dirty snapshot for a push and refresh buffers for a pull.
pub enum NetOutcome {
    Push(PushOutcome),
    Pull(PullOutcome),
    Update(UpdateOutcome),
    Clock(ClockOutcome),
    /// A non-terminal status line for the panel from an operation still running
    /// (see [`app::NetOutcome::Progress`]). Sent from inside the transport's own
    /// progress callbacks, so it shares this channel rather than owning one —
    /// which also keeps it ordered against the terminal outcome behind it.
    Progress(String),
}

/// Result of a push attempt, sent back to the UI task for the snackbar. The
/// detailed error always goes to the serial log; the panel gets a short line.
pub enum PushOutcome {
    /// Committed and pushed. Carries the short commit id for the panel.
    Pushed(String),
    /// The working tree matched HEAD — nothing new to push.
    UpToDate,
    /// Something failed; the string is a short reason for the panel (full error
    /// is logged).
    Failed(String),
}

/// Result of a `:gl` pull attempt. The device never does a content merge; a
/// clean fast-forward applies origin directly, and a divergence is resolved by
/// rebasing our local commit(s) onto origin (last-writer-wins per note) rather
/// than left for a computer.
pub enum PullOutcome {
    /// Fast-forwarded onto origin's tip. Carries the short commit id; the UI
    /// must treat every tracked file as possibly rewritten (reload buffers,
    /// re-walk the palette list).
    Pulled(String),
    /// Histories diverged: origin's changes were integrated and our local
    /// commit(s) replanted on top (a rebase, not a merge). The working copy
    /// moved — same UI refresh as `Pulled` — and the device is now `LocalAhead`,
    /// so the user finishes with `:gs`. Carries the rebased commit's short id.
    Rebased(String),
    /// Origin's tip is our HEAD — nothing to pull.
    UpToDate,
    /// We are strictly ahead of origin (e.g. a stranded commit whose push
    /// failed) — nothing to pull; the next `:gs` pushes it.
    LocalAhead,
    /// Something failed; short reason for the panel (full error is logged).
    Failed(String),
}

/// Result of a `:update` firmware check, sent back to the UI task for the
/// snackbar. Not a git operation — it shares this channel only because the OTA
/// download runs on the same Wi-Fi-owning thread.
pub enum UpdateOutcome {
    /// A newer image was written to the inactive OTA slot, which is now the boot
    /// target. Carries the new version string; the UI reboots into it.
    Installed(String),
    /// The running firmware is already the newest release — nothing to install.
    /// Carries the running version, shown in the notice.
    UpToDate(String),
    /// Something failed (a missing newer release is *not* a failure — that is
    /// `UpToDate`); short reason for the panel, full error logged. The running
    /// slot is untouched, so the device keeps booting the current image.
    Failed(String),
}

/// Result of a clock-only sync. Carries no time: the wall clock itself is the
/// result, and the UI reads the day back through [`app::Clock`].
pub enum ClockOutcome {
    /// The wall clock holds a real date now. A request that found it already
    /// synced this session lands here too — that is a success, not a special case.
    Synced,
    /// No date; short reason for the panel, full error logged. Shown only if an
    /// `:inbox` was waiting on it (the boot request is unprompted).
    Failed(String),
}

/// The net service loop, run on the dedicated net thread. Owns the Wi-Fi stack,
/// bringing it up lazily on the first request and keeping it up afterwards.
/// Blocks on `rx`; for each request it ensures connectivity + clock + trust
/// store, runs one push cycle, and reports the outcome on `tx`. Returns when
/// the request channel closes (UI task gone). Errors are reported, never
/// panicked — a failed push must not take the thread (and its Wi-Fi) down.
pub fn run_net_service(
    modem: Modem<'static>,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    rx: Receiver<NetRequest>,
    tx: Sender<NetOutcome>,
) {
    tune_libgit2();

    // Lazily initialised on the first request, then reused across pushes.
    let mut wifi: Option<BlockingWifi<EspWifi<'static>>> = None;
    let mut modem = Some(modem);
    let mut nvs = Some(nvs);
    let mut clock_synced = false;
    let mut tls_ready = false;

    // Panel status lines from an in-flight operation, over the same channel as
    // the terminal outcome. The one place a `Phase` becomes a line — see
    // `progress::Phase` before adding any decoration to it. A closed channel means
    // the UI is gone; the send failure after the cycle finishes is what ends the
    // loop, so drop it here.
    let progress = |phase: Phase| {
        let _ = tx.send(NetOutcome::Progress(phase.label()));
    };

    while let Ok(req) = rx.recv() {
        let msg = match req {
            NetRequest::Push(req) => NetOutcome::Push(
                match push_cycle(
                    &sys_loop,
                    &mut wifi,
                    &mut modem,
                    &mut nvs,
                    &mut clock_synced,
                    &mut tls_ready,
                    &req.paths,
                    &progress,
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        log::error!("❌ :gs failed: {e:?}");
                        PushOutcome::Failed(short_reason("sync", &e))
                    }
                },
            ),
            NetRequest::Pull(req) => NetOutcome::Pull(
                match pull_cycle(
                    &sys_loop,
                    &mut wifi,
                    &mut modem,
                    &mut nvs,
                    &mut clock_synced,
                    &mut tls_ready,
                    &req,
                    &progress,
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        log::error!("❌ :gl failed: {e:?}");
                        PullOutcome::Failed(short_reason("pull", &e))
                    }
                },
            ),
            // No progress lines: most of these are the unprompted boot request,
            // and a phase line costs a full-panel partial to announce work the
            // writer never asked for.
            NetRequest::Clock => NetOutcome::Clock(
                match clock_cycle(&sys_loop, &mut wifi, &mut modem, &mut nvs, &mut clock_synced) {
                    Ok(()) => ClockOutcome::Synced,
                    Err(e) => {
                        log::error!("❌ clock sync failed: {e:?}");
                        ClockOutcome::Failed(short_reason("clock", &e))
                    }
                },
            ),
            NetRequest::Update => NetOutcome::Update(
                match update_cycle(
                    &sys_loop,
                    &mut wifi,
                    &mut modem,
                    &mut nvs,
                    &mut clock_synced,
                    &mut tls_ready,
                    &progress,
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        log::error!("❌ :update failed: {e:?}");
                        UpdateOutcome::Failed(short_reason("update", &e))
                    }
                },
            ),
        };
        if tx.send(msg).is_err() {
            break;
        }
    }
    log::info!("git service: request channel closed — exiting");
}

/// Shallow-clone `remote_url` into `REPO_DIR` for the onboarding wizard
/// (v0.9 slice 4): init, learn the default branch, fetch it at depth 1, then
/// materialize the tip tree to the working copy (media skipped, like the pull).
/// Credentials are passed explicitly — the wizard runs before `set_card_conf`,
/// so the global accessors are still empty. Returns the number of working-tree
/// files written.
///
/// Runs on a `GIT_STACK` thread (libgit2's path-buffer nesting overflows the
/// default) and applies `tune_libgit2` + installs the TLS trust store itself,
/// since the service thread that normally does so (via `ensure_online`) hasn't
/// started yet — without the trust store libgit2 rejects github.com as
/// `NOT_TRUSTED`. Wi-Fi is already up and the clock already SNTP-synced by the
/// wizard's device-flow step, so only the CA bundle is missing here.
/// `progress` receives short status lines for the panel.
pub fn clone_repo(
    remote_url: &str,
    gh_user: &str,
    token: &str,
    progress: &dyn Fn(Phase),
) -> Result<usize> {
    tune_libgit2();
    // libgit2's mbedTLS stream has no CA set until this runs; the service thread
    // that normally installs it hasn't started during the wizard.
    install_tls_trust_store()?;
    log::info!(
        "clone: init {REPO_DIR} <- {remote_url} (free heap {})",
        free_heap()
    );
    let repo = Repository::init(REPO_DIR).context("git init")?;
    // A previous failed attempt may have left `origin` behind (init is
    // idempotent, but re-adding a remote is not). Reuse or repoint it — this
    // also covers the user re-picking a different repo on retry.
    let mut remote = match repo.remote("origin", remote_url) {
        Ok(r) => r,
        Err(_) => {
            repo.remote_set_url("origin", remote_url)
                .context("repointing existing origin")?;
            repo.find_remote("origin").context("reopening origin")?
        }
    };

    progress(Phase::ContactingOrigin);
    {
        let (u, t) = (gh_user.to_string(), token.to_string());
        let mut cbs = RemoteCallbacks::new();
        cbs.credentials(move |_url, _u, allowed| {
            if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
                Cred::userpass_plaintext(&u, &t)
            } else {
                Err(git2::Error::from_str("server did not offer USER_PASS_PLAINTEXT"))
            }
        });
        cbs.certificate_check(|_c, host| {
            log::info!("verifying {host} TLS chain against embedded GitHub CA bundle");
            Ok(CertificateCheckStatus::CertificatePassthrough)
        });
        remote
            .connect_auth(git2::Direction::Fetch, Some(cbs), None)
            .context("connecting to origin")?;
    }
    let default = remote
        .default_branch()
        .context("origin advertised no default branch")?;
    let refname = default.as_str().context("default branch not UTF-8")?;
    let branch = refname
        .strip_prefix("refs/heads/")
        .unwrap_or(refname)
        .to_string();
    let _ = remote.disconnect();

    progress(Phase::Downloading { current: 0, total: 0 });
    {
        let (u, t) = (gh_user.to_string(), token.to_string());
        let mut cbs = RemoteCallbacks::new();
        cbs.credentials(move |_url, _u, allowed| {
            if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
                Cred::userpass_plaintext(&u, &t)
            } else {
                Err(git2::Error::from_str("server did not offer USER_PASS_PLAINTEXT"))
            }
        });
        cbs.certificate_check(|_c, _host| Ok(CertificateCheckStatus::CertificatePassthrough));
        // Throttled transfer progress: a line every ~512 objects, not per-object
        // (each line repaints the panel).
        let mut last = 0usize;
        cbs.transfer_progress(|p| {
            let recv = p.received_objects();
            let total = p.total_objects();
            if recv >= last + 512 || (total > 0 && recv == total) {
                last = recv;
                progress(Phase::Downloading { current: recv, total });
            }
            true
        });
        let mut fo = FetchOptions::new();
        fo.depth(1);
        fo.remote_callbacks(cbs);
        remote
            .fetch(&[branch.as_str()], Some(&mut fo), None)
            .context("shallow fetch")?;
    }

    let tip = repo
        .find_reference("FETCH_HEAD")
        .context("no FETCH_HEAD after fetch")?
        .peel_to_commit()
        .context("FETCH_HEAD is not a commit")?
        .id();

    // Establish branch + HEAD + tracking ref so the next boot sees a real repo,
    // and a power-pull between here and the working-copy write still resumes
    // (the wizard re-enters on missing/partial repo).
    repo.reference(&format!("refs/heads/{branch}"), tip, true, "typoena clone")
        .context("creating local branch")?;
    repo.set_head(&format!("refs/heads/{branch}"))
        .context("setting HEAD")?;
    repo.reference(
        &format!("refs/remotes/origin/{branch}"),
        tip,
        true,
        "typoena clone",
    )
    .context("creating tracking ref")?;

    // Materialize the tip tree (media skipped — never writes a big blob to RAM).
    progress(Phase::WritingFiles);
    let tree = repo.find_commit(tip)?.tree().context("tip tree")?;
    let mut count = 0usize;
    materialize_tree(&repo, &tree, "", &mut count)?;

    // The file palette walks /sd/local too; make sure it exists.
    let _ = fs::create_dir_all(LOCAL_DIR);
    log::info!(
        "clone: wrote {count} file(s), branch {branch} @ {} (free heap {})",
        short(tip),
        free_heap()
    );
    Ok(count)
}

/// Recursively write a tree's blobs to the working copy under `REPO_DIR`,
/// skipping media (a pulled image would materialize its whole blob in RAM — the
/// OOM the pull avoids). Atomic writes (tmp + rename, FAT won't overwrite) like
/// the pull, so a power-pull mid-clone leaves partial files the next attempt
/// overwrites idempotently.
fn materialize_tree(
    repo: &Repository,
    tree: &git2::Tree,
    prefix: &str,
    count: &mut usize,
) -> Result<()> {
    for entry in tree.iter() {
        let Some(name) = entry.name() else { continue };
        let rel = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(ObjectType::Tree) => {
                let obj = entry
                    .to_object(repo)
                    .with_context(|| format!("reading subtree {rel}"))?;
                if let Some(sub) = obj.as_tree() {
                    materialize_tree(repo, sub, &rel, count)?;
                }
            }
            Some(ObjectType::Blob) => {
                if is_media_path(&rel) {
                    continue;
                }
                let abs = format!("{REPO_DIR}/{rel}");
                if let Some(dir) = std::path::Path::new(&abs).parent() {
                    fs::create_dir_all(dir).with_context(|| format!("mkdir for {rel}"))?;
                }
                let blob = repo
                    .find_blob(entry.id())
                    .with_context(|| format!("reading blob for {rel}"))?;
                let tmp = format!("{abs}.gltmp");
                fs::write(&tmp, blob.content()).with_context(|| format!("writing {rel}"))?;
                let _ = fs::remove_file(&abs);
                fs::rename(&tmp, &abs).with_context(|| format!("landing {rel}"))?;
                *count += 1;
            }
            _ => {}
        }
    }
    Ok(())
}

/// One full push: ensure Wi-Fi + clock + trust store (each done once), then
/// open the repo, stage, commit, and fast-forward push. `progress` receives the
/// short panel lines described on [`NetOutcome::Progress`].
#[allow(clippy::too_many_arguments)]
fn push_cycle(
    sys_loop: &EspSystemEventLoop,
    wifi: &mut Option<BlockingWifi<EspWifi<'static>>>,
    modem: &mut Option<Modem<'static>>,
    nvs: &mut Option<EspDefaultNvsPartition>,
    clock_synced: &mut bool,
    tls_ready: &mut bool,
    paths: &BTreeSet<String>,
    progress: &dyn Fn(Phase),
) -> Result<PushOutcome> {
    if remote_url().is_empty() || gh_user().is_empty() || token().is_empty() || wifi_ssid().is_empty() {
        bail!("git config missing — provision the card's typoena.conf (installer / wizard) or set TW_* in firmware/.env and rebuild");
    }

    // Nothing recorded dirty and origin's tracking ref already has HEAD: this
    // `:gs` has nothing to do — say so without touching the radio (~150 ms
    // instead of a Wi-Fi + TLS round). A stranded local commit (committed but
    // never pushed, e.g. a push that failed mid-air) makes the check false and
    // takes the full path below, where push_once pushes it.
    if paths.is_empty() && remote_current().unwrap_or(false) {
        log::info!(":gs — no dirty paths and origin has HEAD; up to date, radio untouched");
        return Ok(PushOutcome::UpToDate);
    }

    // Phases are timed so a cold :gs reports where the seconds go. Wi-Fi, clock
    // and TLS run only on the first sync of a session; a warm sync skips them, so
    // they read 0 ms and the total collapses to just push(fetch+commit+push).
    let t_total = Instant::now();
    ensure_online(sys_loop, wifi, modem, nvs, clock_synced, tls_ready, progress)?;

    let t_push = Instant::now();
    let outcome = push_once(paths, progress)?;
    log::info!(
        ":gs timing — push(commit+push) {}ms, total {}ms",
        t_push.elapsed().as_millis(),
        t_total.elapsed().as_millis(),
    );
    Ok(outcome)
}

/// One full pull (`:gl`): ensure connectivity, then fetch + fast-forward/rebase.
/// Always needs the network — there is no radio-free shortcut like push's
/// up-to-date check, because the whole point is asking origin what's new.
/// `paths` is the dirty-journal snapshot to commit locally before fetching.
/// `progress` receives the short panel lines described on [`NetOutcome::Progress`].
#[allow(clippy::too_many_arguments)]
fn pull_cycle(
    sys_loop: &EspSystemEventLoop,
    wifi: &mut Option<BlockingWifi<EspWifi<'static>>>,
    modem: &mut Option<Modem<'static>>,
    nvs: &mut Option<EspDefaultNvsPartition>,
    clock_synced: &mut bool,
    tls_ready: &mut bool,
    req: &PullRequest,
    progress: &dyn Fn(Phase),
) -> Result<PullOutcome> {
    if remote_url().is_empty() || gh_user().is_empty() || token().is_empty() || wifi_ssid().is_empty() {
        bail!("git config missing — provision the card's typoena.conf (installer / wizard) or set TW_* in firmware/.env and rebuild");
    }
    let t_total = Instant::now();
    ensure_online(sys_loop, wifi, modem, nvs, clock_synced, tls_ready, progress)?;

    let t_pull = Instant::now();
    let outcome = pull_once(req, progress)?;
    log::info!(
        ":gl timing — fetch+ff {}ms, total {}ms",
        t_pull.elapsed().as_millis(),
        t_total.elapsed().as_millis(),
    );
    Ok(outcome)
}

/// One firmware-update check (`:update`): ensure connectivity, then hand off to
/// the OTA module, which compares the running version against the release
/// manifest and — only if it is newer — streams the image into the inactive OTA
/// slot. Always needs the network (the whole point is asking what the latest
/// release is). Reuses the net thread's Wi-Fi + clock bring-up; it needs no git
/// config (no remote/token), only Wi-Fi.
fn update_cycle(
    sys_loop: &EspSystemEventLoop,
    wifi: &mut Option<BlockingWifi<EspWifi<'static>>>,
    modem: &mut Option<Modem<'static>>,
    nvs: &mut Option<EspDefaultNvsPartition>,
    clock_synced: &mut bool,
    tls_ready: &mut bool,
    progress: &dyn Fn(Phase),
) -> Result<UpdateOutcome> {
    if wifi_ssid().is_empty() {
        bail!("Wi-Fi not provisioned — run :setup / the installer first");
    }
    let t_total = Instant::now();
    ensure_online(sys_loop, wifi, modem, nvs, clock_synced, tls_ready, progress)?;

    let t_ota = Instant::now();
    let outcome = match crate::infrastructure::ota::run_update(progress)? {
        Some(version) => UpdateOutcome::Installed(version),
        None => UpdateOutcome::UpToDate(crate::infrastructure::ota::FW_VERSION.to_string()),
    };
    log::info!(
        ":update timing — ota(check+download) {}ms, total {}ms",
        t_ota.elapsed().as_millis(),
        t_total.elapsed().as_millis(),
    );
    Ok(outcome)
}

/// Bring Wi-Fi + wall clock + TLS trust store up, each once per session; a warm
/// call is a no-op. Shared by push, pull and update, on the net thread. Logs one
/// timing line whenever any step actually ran (the session's first git operation
/// pays them all; every later one skips straight to git).
///
/// Each step reports only when it actually runs: a warm operation skips all
/// three, and a line for a 0 ms phase would cost a full-panel partial to say
/// nothing. Cold, they are ~3.65 s / ~2.1 s / ~0.3 s — the longest silent stretch
/// a sync has, which is why they are three lines and not one.
fn ensure_online(
    sys_loop: &EspSystemEventLoop,
    wifi: &mut Option<BlockingWifi<EspWifi<'static>>>,
    modem: &mut Option<Modem<'static>>,
    nvs: &mut Option<EspDefaultNvsPartition>,
    clock_synced: &mut bool,
    tls_ready: &mut bool,
    progress: &dyn Fn(Phase),
) -> Result<()> {
    let wifi_ms = ensure_wifi(sys_loop, wifi, modem, nvs, progress)?;
    let clock_ms = ensure_clock(clock_synced, progress)?;
    let tls_ms = if !*tls_ready {
        let t = Instant::now();
        progress(Phase::VerifyingTls);
        install_tls_trust_store()?;
        *tls_ready = true;
        t.elapsed().as_millis()
    } else {
        0
    };
    if wifi_ms + clock_ms + tls_ms > 0 {
        log::info!("online — wifi {wifi_ms}ms, clock {clock_ms}ms, tls {tls_ms}ms");
    }
    Ok(())
}

/// Associate with the AP unless we already are. Returns the milliseconds it
/// cost, 0 when the session was already on the network.
///
/// The `EspWifi` driver is built once — it swallows the modem and the NVS
/// partition, and neither comes back — but it is then kept whatever the
/// association does. A bring-up that fails must leave a driver a later request
/// can retry against: the clock sync runs unprompted at boot, so with the modem
/// merely spent, one boot out of Wi-Fi range would cost the whole session its
/// radio (and a `:gs` an hour later would have nothing left to join with).
fn ensure_wifi(
    sys_loop: &EspSystemEventLoop,
    wifi: &mut Option<BlockingWifi<EspWifi<'static>>>,
    modem: &mut Option<Modem<'static>>,
    nvs: &mut Option<EspDefaultNvsPartition>,
    progress: &dyn Fn(Phase),
) -> Result<u128> {
    if wifi.as_ref().is_some_and(|w| w.is_connected().unwrap_or(false)) {
        return Ok(0);
    }
    let t = Instant::now();
    progress(Phase::JoiningWifi);
    if wifi.is_none() {
        log::info!("bringing the Wi-Fi driver up; free heap {}", free_heap());
        let m = modem.take().context("Wi-Fi modem already spent")?;
        let n = nvs.take().context("NVS partition already spent")?;
        *wifi = Some(BlockingWifi::wrap(
            EspWifi::new(m, sys_loop.clone(), Some(n))?,
            sys_loop.clone(),
        )?);
    }
    let Some(w) = wifi.as_mut() else {
        bail!("Wi-Fi driver missing right after building it");
    };
    connect_wifi(w, wifi_ssid(), wifi_pass()).context("connecting Wi-Fi")?;
    let ip = w.wifi().sta_netif().get_ip_info()?;
    log::info!("Wi-Fi up — IP {}", ip.ip);
    Ok(t.elapsed().as_millis())
}

/// Set the wall clock over SNTP unless this session already did. Returns the
/// milliseconds it cost, 0 when warm. Needs Wi-Fi up first.
fn ensure_clock(clock_synced: &mut bool, progress: &dyn Fn(Phase)) -> Result<u128> {
    if *clock_synced {
        return Ok(0);
    }
    let t = Instant::now();
    progress(Phase::SettingClock);
    sync_clock()?;
    *clock_synced = true;
    Ok(t.elapsed().as_millis())
}

/// The cheap half of [`ensure_online`]: join the AP and set the wall clock, then
/// stop. No trust store, no libgit2, no handshake with the remote — this exists
/// so `:inbox` can date a fleeting note in the seconds a join and an SNTP packet
/// take instead of the tens a pull takes. Silent by design (see the request arm).
///
/// The clock is a once-per-power-cycle fact, so a warm request answers without
/// touching the radio at all, and the association this one opens is closed again
/// on the way out ([`park_wifi`]) — the request arrives unprompted at every cold
/// boot, and a station held up for a whole session of typing is battery spent on
/// a date already read.
fn clock_cycle(
    sys_loop: &EspSystemEventLoop,
    wifi: &mut Option<BlockingWifi<EspWifi<'static>>>,
    modem: &mut Option<Modem<'static>>,
    nvs: &mut Option<EspDefaultNvsPartition>,
    clock_synced: &mut bool,
) -> Result<()> {
    if *clock_synced {
        return Ok(());
    }
    if wifi_ssid().is_empty() {
        bail!("Wi-Fi not provisioned — run :setup / the installer first");
    }
    let silent = |_: Phase| {};
    let wifi_ms = match ensure_wifi(sys_loop, wifi, modem, nvs, &silent) {
        Ok(ms) => ms,
        Err(e) => {
            park_wifi(wifi);
            return Err(e);
        }
    };
    let sntp = ensure_clock(clock_synced, &silent);
    // Only ever what this cycle woke: a station a `:gs` brought up belongs to
    // that session's git work, and stopping it under the next push would cost a
    // re-join mid-sync.
    if wifi_ms > 0 {
        park_wifi(wifi);
    }
    let clock_ms = sntp?;
    log::info!("clock-only sync — wifi {wifi_ms}ms, sntp {clock_ms}ms");
    Ok(())
}

/// Stop the station, keeping the driver. [`ensure_wifi`] re-associates against
/// it on the next request that needs the wire, so this costs a later `:gs` the
/// same join a cold one already pays. Never fatal — by the time it runs, the
/// clock it woke the radio for has either landed or failed on its own.
fn park_wifi(wifi: &mut Option<BlockingWifi<EspWifi<'static>>>) {
    let Some(w) = wifi.as_mut() else {
        return;
    };
    match w.stop() {
        Ok(()) => log::info!("radio parked; next :gs/:gl re-joins"),
        Err(e) => log::warn!("could not park the radio: {e}"),
    }
}

/// Open `/sd/repo`, commit the working tree on the current branch, and push.
///
/// Optimistic: it pushes onto the current tip *without* a pre-fetch, so the
/// common case (nothing else touched the remote) costs a single TLS handshake.
/// If the remote has moved under us — a foreign push, e.g. maintenance — the push
/// is rejected non-fast-forward; we then reconcile onto origin, replay our note on
/// the new tip, and retry once.
///
/// Never clones or wipes: a `/sd/repo` that isn't a valid repo is a provisioning
/// error, surfaced as such.
fn push_once(paths: &BTreeSet<String>, progress: &dyn Fn(Phase)) -> Result<PushOutcome> {
    log::info!(
        "push started — {} dirty path(s), free heap {} ({} internal)",
        paths.len(),
        free_heap(),
        internal_free_heap()
    );
    log_odb_inventory();
    let repo = Repository::open(REPO_DIR).with_context(|| {
        format!("opening git repo at {REPO_DIR} — provision the card with a clone (just init) whose origin is your remote")
    })?;

    let branch = repo
        .head()?
        .shorthand()
        .context("HEAD has no branch shorthand")?
        .to_string();
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");

    let mut oid = match stage_and_commit(&repo, paths)? {
        Some(oid) => oid,
        None => {
            // Nothing new to commit. Usually genuinely up to date — but a
            // previous cycle may have committed and then failed to push,
            // stranding a local-only commit (the old add_all path silently
            // never retried those). Push whenever origin's tracking ref
            // doesn't already have HEAD.
            let head = repo.head()?.peel_to_commit()?.id();
            if tracking_tip(&repo, &branch) == Some(head) {
                return Ok(PushOutcome::UpToDate);
            }
            log::info!(
                "tree unchanged but origin/{branch} lacks HEAD {} — pushing the stranded commit",
                short(head)
            );
            head
        }
    };

    // Optimistic push. A non-fast-forward *rejection* means the remote moved
    // under us: reconcile onto origin and replay the dirty paths on the new
    // tip, then retry once (reconcile_onto_origin soft-resets — ref move only —
    // so the notes stay on the card and stage_and_commit splices them on top of
    // origin). A transport-level failure is surfaced as-is: its fetch would die
    // the same way, and the commit is safe locally — the stranded-commit check
    // above pushes it once the transport works again.
    if let Err(failure) = try_push(&repo, &refspec, progress) {
        let rejection = match failure {
            PushFailure::Rejected(msg) => msg,
            PushFailure::Other(e) => return Err(e),
        };
        log::warn!("push rejected ({rejection}); reconciling onto origin and replaying the note");
        // The retry is the slow path (a fetch, a replay, a second handshake) and
        // it lands well after the first "sending" lines — say why the wait grew
        // rather than letting a stale count sit there.
        progress(Phase::Retrying);
        reconcile_onto_origin(&repo, &branch).context("reconciling after a rejected push")?;
        match stage_and_commit(&repo, paths)? {
            Some(replayed) => {
                oid = replayed;
                try_push(&repo, &refspec, progress)
                    .map_err(PushFailure::into_error)
                    .context("push after reconcile")?;
            }
            // The note was already on origin (nothing to replay) — treat as done.
            None => {
                log::info!("nothing to replay after reconcile — already up to date");
                return Ok(PushOutcome::UpToDate);
            }
        }
    }

    log::info!(
        "push done — free heap {} ({} internal), min-ever {}",
        free_heap(),
        internal_free_heap(),
        min_free_heap()
    );
    Ok(PushOutcome::Pushed(short(oid)))
}

/// Roll `paths` back to their last-synced state — the confirmed `d` on the
/// unsynced card. A path HEAD has is overwritten from HEAD's blob; a path HEAD
/// has never seen (a note written on the device since the last sync) has no
/// version to return to, so it leaves the card. The device's `git checkout --`
/// plus `git clean`, restricted to the journal's paths: O(paths), the same
/// reason [`stage_and_commit`] never walks the tree.
///
/// **Destroys writing, and nothing here can undo it** — the confirmations are
/// two steps up in the UI (see [`PullRequest::discard`]). The caller has
/// already taken the journal, so the paths are forgotten whatever happens next.
///
/// Never fails the pull: a per-path error is logged and skipped. A path that
/// couldn't be restored is simply still dirty on the card while the journal has
/// forgotten it — recoverable by editing and saving it again, whereas aborting
/// the pull here would strand the ones already rolled back.
fn discard_paths(repo: &Repository, paths: &BTreeSet<String>) {
    // `None` on an unborn branch (a card provisioned from an empty remote):
    // nothing is committed yet, so every recorded path is a device-side
    // creation and the loop below unlinks them all.
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let (mut restored, mut removed) = (0usize, 0usize);
    for path in paths {
        let abs = format!("{REPO_DIR}/{path}");
        let blob = head_tree
            .as_ref()
            .and_then(|t| t.get_path(std::path::Path::new(path)).ok())
            .and_then(|e| repo.find_blob(e.id()).ok());
        match blob {
            Some(blob) => {
                // The parent dir may itself have gone with a `:delete`, so
                // recreate the chain before writing HEAD's bytes back.
                if let Some(dir) = std::path::Path::new(&abs).parent() {
                    if let Err(e) = fs::create_dir_all(dir) {
                        log::warn!("discard: mkdir for {path} FAILED ({e:#}); left as-is");
                        continue;
                    }
                }
                match fs::write(&abs, blob.content()) {
                    Ok(()) => restored += 1,
                    Err(e) => log::warn!("discard: restoring {path} FAILED ({e:#}); left as-is"),
                }
            }
            None => match fs::remove_file(&abs) {
                Ok(()) => removed += 1,
                // Already gone: a `:delete` the remote never saw. The discard
                // wanted exactly this state, so it is a success, not an error.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => removed += 1,
                Err(e) => log::warn!("discard: unlinking {path} FAILED ({e:#}); left as-is"),
            },
        }
    }
    log::info!("discard — {restored} path(s) restored from HEAD, {removed} removed from the card");
}

/// Build the commit for `paths` as an O(depth) TreeBuilder splice onto HEAD's
/// tree and return the new commit id — or `None` when the result matches the
/// parent (nothing to push). Called on the first attempt and again to
/// replay the dirty paths after a reconcile.
///
/// This replaces the index pipeline (`add_all` → `index.write` → `write_tree`),
/// which is O(N_tree) and cannot run on the real 1179-file / 570 MB-pack clone:
/// `index.write`'s racy-clean pass re-hashes ~every entry on FAT's 2 s mtimes
/// (measured up to **611 s**), and even the index-free `read_tree` walk was
/// 77 s. The splice reads and writes only the dirty paths' ancestor chains —
/// O(depth × dirty), flat in repo size, **~2–2.8 s measured on the real
/// clone** — and carries every untouched entry (including the ~150 MB of
/// images) forward by OID without ever opening it. Trail + bench numbers:
/// docs/tradeoff-curves/sync-commit-staging.md.
///
/// The working tree is the source of truth: a recorded path that exists on the
/// card is spliced in from disk, a missing one is spliced out (a `:delete`).
/// Unrecorded paths are never visited — so Finder cruft (`._*`, `.DS_Store`)
/// on the FAT card can no longer ride into a commit the way it once did with
/// `add_all` (07d87772), and the old cruft filter is gone with the walk.
fn stage_and_commit(repo: &Repository, paths: &BTreeSet<String>) -> Result<Option<Oid>> {
    // Commit on top of the current branch tip (None on an empty/unborn remote,
    // where the splice starts from an empty base and makes a parentless commit).
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let base = match &parent {
        Some(c) => Some(c.tree().context("loading HEAD tree")?),
        None => None,
    };

    // I/O attribution for the ~360 ms/loose-write residual: bracket the splice
    // with the p_mmap counters so the log says how many
    // mmap windows (≈ unique pack reads) and how many KB the whole splice
    // issued. Divided by the loose writes (~4/path: blob + tree chain), this
    // pins whether the residual is pack-read I/O or FAT directory ops — the
    // two candidates left after FASTSEEK.
    let (maps_before, read_kb_before) = map_counters();
    let t_splice = Instant::now();
    let mut tree = base;
    for path in paths {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let blob = match fs::read(format!("{REPO_DIR}/{path}")) {
            Ok(bytes) => Some(
                repo.blob(&bytes)
                    .with_context(|| format!("writing blob for {path}"))?,
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {path}")),
        };
        let spliced = splice(repo, tree.as_ref(), &parts, blob)
            .with_context(|| format!("splicing {path}"))?;
        tree = Some(repo.find_tree(spliced).context("loading spliced tree")?);
    }
    let Some(tree) = tree else {
        return Ok(None);
    };
    let splice_ms = t_splice.elapsed().as_millis();

    if let Some(p) = &parent {
        if p.tree_id() == tree.id() {
            log::info!("nothing to push — tree unchanged @ {}", short(p.id()));
            return Ok(None);
        }
    }

    let sig = Signature::now(author_name(), author_email()).context("building signature")?;
    let message = format!("Typoena push — unix {}", now_unix());
    let parents: Vec<&Commit> = parent.iter().collect();
    let t_commit = Instant::now();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parents)
        .context("creating commit")?;
    let (maps_after, read_kb_after) = map_counters();
    log::info!(
        "commit split — splice {splice_ms}ms ({} path(s), {} mmaps / {} KB read), commit-obj {}ms; committed {} — free heap {} ({} internal)",
        paths.len(),
        maps_after - maps_before,
        read_kb_after - read_kb_before,
        t_commit.elapsed().as_millis(),
        short(oid),
        free_heap(),
        internal_free_heap()
    );
    Ok(Some(oid))
}

/// Return a new tree = `base` with `path` set to `blob` (`Some` inserts or
/// replaces, `None` removes). Recurses down the path's subtree chain: reads
/// ~depth tree objects and writes ~depth new ones, leaving every sibling entry
/// untouched (carried by OID — never opened). A missing intermediate directory
/// is synthesized on the way down; a directory emptied by a remove is pruned
/// on the way up rather than left behind as an empty tree entry.
fn splice(repo: &Repository, base: Option<&Tree>, path: &[&str], blob: Option<Oid>) -> Result<Oid> {
    let (head, rest) = path.split_first().context("splice: empty path")?;
    let mut tb = repo.treebuilder(base).context("treebuilder")?;
    if rest.is_empty() {
        match blob {
            Some(oid) => {
                tb.insert(*head, oid, 0o100644)
                    .context("inserting blob entry")?;
            }
            // Removing a never-committed path is a no-op, not an error (a note
            // created and deleted between two syncs).
            None => {
                let _ = tb.remove(*head);
            }
        }
    } else {
        let sub = match base.and_then(|b| b.get_name(head)) {
            Some(e) if e.kind() == Some(ObjectType::Tree) => {
                Some(repo.find_tree(e.id()).context("loading subtree")?)
            }
            // Absent (a new directory) or a non-tree shadowing the name —
            // build the subtree from scratch either way.
            _ => None,
        };
        let new_sub = splice(repo, sub.as_ref(), rest, blob)?;
        if repo.find_tree(new_sub)?.is_empty() {
            let _ = tb.remove(*head);
        } else {
            tb.insert(*head, new_sub, 0o040000)
                .context("inserting subtree entry")?;
        }
    }
    tb.write().context("writing spliced tree")
}

/// Origin's remote-tracking tip for `branch`, if the ref exists. libgit2
/// updates it after a successful push/fetch, so it is "the newest commit we
/// know origin has" — without touching the network.
fn tracking_tip(repo: &Repository, branch: &str) -> Option<Oid> {
    repo.find_reference(&format!("refs/remotes/origin/{branch}"))
        .ok()?
        .peel_to_commit()
        .ok()
        .map(|c| c.id())
}

/// Whether origin is known to already have HEAD (local refs only, no network).
/// Errors read as "not current", so the caller falls through to the full
/// push path where the real failure surfaces with context.
fn remote_current() -> Result<bool> {
    let repo = Repository::open(REPO_DIR)?;
    let head = repo.head()?.peel_to_commit()?.id();
    let branch = repo
        .head()?
        .shorthand()
        .context("HEAD has no branch shorthand")?
        .to_string();
    Ok(tracking_tip(&repo, &branch) == Some(head))
}

/// How a push attempt failed — this decides whether reconciling can help.
enum PushFailure {
    /// The server processed the push but refused the ref update (arrives via
    /// the `push_update_reference` callback — e.g. non-fast-forward): the
    /// remote moved under us, and reconcile + replay is the right response.
    Rejected(String),
    /// Transport / TLS / auth / URL — the push never reached a ref decision,
    /// so a reconcile (whose fetch needs the same transport) cannot help.
    /// Surfaced directly; the 2026-07-13 on-device run burned a doomed
    /// reconcile on an "unsupported URL protocol" because this wasn't split.
    Other(anyhow::Error),
}

impl PushFailure {
    fn into_error(self) -> anyhow::Error {
        match self {
            Self::Rejected(msg) => anyhow::anyhow!("remote rejected ref: {msg}"),
            Self::Other(e) => e,
        }
    }
}

/// One push attempt over HTTPS. Binds the PAT credential + the cert-verify
/// callback, and separates a server-side ref rejection (reconcilable) from a
/// transport-level failure (not).
fn try_push(
    repo: &Repository,
    refspec: &str,
    progress: &dyn Fn(Phase),
) -> Result<(), PushFailure> {
    let mut remote = repo
        .find_remote("origin")
        .map_err(|e| PushFailure::Other(anyhow::Error::new(e).context("finding remote origin")))?;
    let rejection: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let mut cbs = auth_callbacks();
    {
        let rejection = rejection.clone();
        cbs.push_update_reference(move |refname, status| {
            if let Some(msg) = status {
                *rejection.borrow_mut() = Some(format!("{refname}: {msg}"));
            }
            Ok(())
        });
    }
    // Progress + heap tracing through the otherwise-silent stretch between the
    // TLS verify and the first byte sent (66 s in runs 4 and 5, OOM both times).
    {
        // Time-gated, NOT count-gated: during AddingObjects libgit2 reports
        // `total` = 0 and `current` = objects inserted so far, and a two-commit
        // push only inserts a few dozen objects — an object-count gate swallows
        // every callback and the grind goes silent. libgit2 already rate-limits
        // to ~2/s (MIN_PROGRESS_UPDATE_INTERVAL); gate to ~1 line per 2 s on top.
        let mut last: Option<Instant> = None;
        cbs.pack_progress(move |stage, current, total| {
            if last.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) {
                last = Some(Instant::now());
                log_push_heap(&format!("pack {stage:?} {current}/{total}"));
                progress(Phase::Packing { current, total });
            }
        });
        let mut next_bytes: usize = 0;
        // The panel line is time-gated where the log line is byte-gated: 64 KB is
        // the right grain for a heap post-mortem, but a multi-MB first sync would
        // spend its whole 64-partial ghosting budget repainting a byte counter.
        // Bound the panel cost by duration instead, so it can't scale with payload.
        let mut last_line: Option<Instant> = None;
        cbs.push_transfer_progress(move |current, total, bytes| {
            if bytes >= next_bytes || (total > 0 && current == total) {
                next_bytes = bytes + 64 * 1024;
                log_push_heap(&format!("send {current}/{total} objects, {bytes} B"));
            }
            let done = total > 0 && current == total;
            if done || last_line.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) {
                last_line = Some(Instant::now());
                progress(Phase::Sending { current, total });
            }
        });
    }

    let mut opts = PushOptions::new();
    opts.remote_callbacks(cbs);
    log_push_heap("pre-push");

    // A non-fast-forward can also surface here, not just via the callback:
    // libgit2 compares against origin's advertised tips during negotiation and
    // errors out of push() with ErrorCode::NotFastForward before sending
    // anything, so `push_update_reference` never fires. It's still the
    // remote-moved-under-us case — reconcilable, not a transport failure. Misread
    // it and the rejection surfaces as "push transport" and skips the reconcile.
    remote.push(&[refspec], Some(&mut opts)).map_err(|e| {
        // Heap post-mortem. The known failure is a ~7 KB inflateInit inside the
        // pack build ("failed to init zlib stream on unpack"); the min-ever lines
        // say which pool zeroed and whether it was exhaustion or fragmentation,
        // even when no progress callback got a chance to fire.
        log_push_heap("push failed");
        if e.code() == git2::ErrorCode::NotFastForward {
            PushFailure::Rejected(format!("{refspec}: {}", e.message()))
        } else {
            PushFailure::Other(anyhow::Error::new(e).context("push transport"))
        }
    })?;

    if let Some(msg) = rejection.borrow().clone() {
        return Err(PushFailure::Rejected(msg));
    }
    log_push_heap("post-push");
    log::info!("push accepted by remote");
    Ok(())
}

/// Fetch origin and *soft*-reset the local branch onto it, so our changes can
/// be replayed on the current tip. Only runs after a non-fast-forward push
/// rejection — i.e. the remote moved under us.
///
/// **SOFT**, deliberately: it moves only the branch ref. The previous Mixed
/// reset also rewrote the index — pure waste now that the splice commit never
/// reads the index, and on the real repo an index write is exactly the
/// racy-clean wall the splice exists to avoid. Neither flavor touches the
/// working tree, so the notes being pushed survive on the card and the
/// replay splices them onto the new tip. For a single-writer appliance this
/// resolves last-writer-wins: a concurrent remote *edit* to a note we're
/// pushing loses to ours, while a remote-only added/changed file is simply
/// carried forward — origin's tree is now the splice base, so the replay
/// keeps it (an improvement over the old `add --all` replay, which dropped
/// files the card didn't have). A real merge stays increment-B work.
fn reconcile_onto_origin(repo: &Repository, branch: &str) -> Result<()> {
    let theirs = fetch_origin(repo, branch)?;
    log::info!(
        "reconcile: resetting local {branch} onto origin @ {} (soft — ref move only, notes stay on the card)",
        short(theirs)
    );
    let their_obj = repo.find_object(theirs, None)?;
    repo.reset(&their_obj, git2::ResetType::Soft, None)
        .context("soft reset onto origin")?;
    Ok(())
}

/// Fetch `branch` from origin and return the fetched tip's commit id. Shared
/// by the pull and the post-rejection reconcile. Also refreshes the
/// remote-tracking ref, keeping [`tracking_tip`] (and with it push's
/// radio-free up-to-date check) honest about what origin has.
fn fetch_origin(repo: &Repository, branch: &str) -> Result<Oid> {
    let mut remote = repo.find_remote("origin")?;
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(auth_callbacks());
    remote
        .fetch(&[branch], Some(&mut fo), None)
        .context("fetch origin")?;
    let theirs = repo
        .find_reference("FETCH_HEAD")
        .context("no FETCH_HEAD after fetch")?
        .peel_to_commit()
        .context("FETCH_HEAD is not a commit")?
        .id();
    update_tracking(repo, branch, theirs)?;
    Ok(theirs)
}

/// Point the remote-tracking ref at `tip` (which must already be in the local
/// odb). Keeps [`tracking_tip`] — and with it push's radio-free up-to-date
/// check — honest about what origin has.
fn update_tracking(repo: &Repository, branch: &str, tip: Oid) -> Result<()> {
    repo.reference(
        &format!("refs/remotes/origin/{branch}"),
        tip,
        true,
        "typoena fetch",
    )
    .context("updating remote-tracking ref")?;
    Ok(())
}

/// Open `/sd/repo`, fetch origin, and integrate — **fast-forward when we can,
/// rebase when we must**, never a content merge. The non-failure shapes map to
/// [`PullOutcome`]: already current, we're strictly ahead (a stranded commit —
/// `:gs`'s job), a clean fast-forward, or a divergence — where instead of
/// refusing we replant our local commit(s) onto origin ([`rebase_local_onto`])
/// and end `LocalAhead` for `:gs` to push.
///
/// `req.paths` is the dirty-journal snapshot. Before touching the network we
/// fold those saved-but-unpushed paths into a local commit ([`stage_and_commit`],
/// the commit half of `:gs` without the push): that makes `:gl` self-sufficient
/// — the ff/rebase below replants the commit onto origin so a plain `:gs`
/// finishes it, no computer needed — and, because the working copy now matches
/// the new HEAD, the rehash belt can't fight a device-side save. The UI has
/// already confirmed this commit (it is user-visible). Empty `paths` is a plain
/// fetch.
///
/// `req.discard` swaps that fold for [`discard_paths`] — the confirmed "throw
/// it away and pull" answer. Either way the card ends matching HEAD before the
/// fetch, which is what keeps the rehash belt below quiet.
///
/// The fast-forward is apply-then-ref-move, and the apply is a tree-to-tree diff
/// ([`apply_tree_diff`]) rather than a working-copy checkout; the ref moves last,
/// so a power-pull mid-apply re-runs identically. Its rehash belt refuses to
/// clobber a file edited behind git's back; after the pre-fetch commit the card
/// matches HEAD, so a device-side save never trips it.
///
/// An edit the journal never saw would, though — and a refusal cannot clear
/// itself, so it would refuse forever. Drift found after the fetch
/// ([`drifted_paths`]) is therefore committed too, which keeps the card's bytes
/// and leaves the rebase to replant them.
fn pull_once(req: &PullRequest, progress: &dyn Fn(Phase)) -> Result<PullOutcome> {
    let paths = &req.paths;
    log::info!(
        "pull started — {} unpushed path(s) to {}, free heap {} ({} internal)",
        paths.len(),
        if req.discard { "DISCARD" } else { "fold in" },
        free_heap(),
        internal_free_heap()
    );
    log_odb_inventory();
    let repo = Repository::open(REPO_DIR).with_context(|| {
        format!("opening git repo at {REPO_DIR} — provision the card with a clone (just init) whose origin is your remote")
    })?;
    let branch = repo
        .head()?
        .shorthand()
        .context("HEAD has no branch shorthand")?
        .to_string();
    let mut head = repo.head()?.peel_to_commit()?.id();

    // Fold any saved-but-unpushed work into a local commit before the fetch,
    // so a divergence rebases it onto origin instead of refusing (and the card
    // ends matching HEAD, keeping the SAFE belt below quiet). Local only — no
    // network. stage_and_commit moves the branch ref (commits onto "HEAD") and
    // returns None when the paths are already committed (a no-op splice), so
    // `head` only advances when there was genuinely new work.
    //
    // Or, if the writer answered `d` on the unsynced card, roll those same
    // paths back instead — no commit, and `head` stays put.
    if !paths.is_empty() {
        if req.discard {
            discard_paths(&repo, paths);
        } else if let Some(committed) = stage_and_commit(&repo, paths)? {
            log::info!(
                "pull: committed {} unpushed path(s) locally as {} before fetch",
                paths.len(),
                short(committed)
            );
            head = committed;
        }
    }

    // ls-refs first, download only if needed: the ref advertisement alone
    // answers "anything new?", so the common shapes (up to date, local ahead)
    // never enter pack negotiation — the first on-device pull paid a 9.7 s
    // fetch just to learn it was up to date. When a download IS needed it
    // rides the same open connection (no second TLS handshake).
    let mut remote = repo.find_remote("origin")?;
    let t_ls = Instant::now();
    // The ref advertisement is its own wait (9.7 s on the first on-device pull)
    // and both no-download shapes end here, so this is the only line most pulls
    // ever show.
    progress(Phase::ContactingOrigin);
    remote
        .connect_auth(git2::Direction::Fetch, Some(auth_callbacks()), None)
        .context("connecting to origin")?;
    let refname = format!("refs/heads/{branch}");
    let theirs = remote
        .list()
        .context("listing origin refs")?
        .iter()
        .find(|h| h.name() == refname)
        .map(|h| h.oid())
        .with_context(|| format!("origin does not advertise {refname}"))?;
    let ls_ms = t_ls.elapsed().as_millis();

    if theirs == head {
        let _ = remote.disconnect();
        update_tracking(&repo, &branch, theirs)?;
        log::info!("pull: origin @ {} == HEAD — up to date (ls-refs {ls_ms}ms, no fetch)", short(head));
        return Ok(PullOutcome::UpToDate);
    }
    // `theirs` an ancestor of HEAD ⇒ it is already in the local odb (all
    // ancestors of a local commit are local) — no download needed either.
    if repo.odb()?.exists(theirs)
        && repo
            .graph_descendant_of(head, theirs)
            .context("descendant check (local ahead)")?
    {
        let _ = remote.disconnect();
        update_tracking(&repo, &branch, theirs)?;
        log::info!(
            "pull: HEAD {} is ahead of origin {} — nothing to pull, :gs pushes it (ls-refs {ls_ms}ms, no fetch)",
            short(head),
            short(theirs)
        );
        return Ok(PullOutcome::LocalAhead);
    }

    // Origin has commits we lack: download them over the already-open
    // connection (callbacks were bound at connect_auth).
    let t_fetch = Instant::now();
    let mut fo = FetchOptions::new();
    let mut cbs = auth_callbacks();
    // Time-gated, not object-gated: a big first pull must not turn its object
    // counter into dozens of full-panel partials (see `try_push`'s send line).
    let mut last_line: Option<Instant> = None;
    cbs.transfer_progress(move |p| {
        let (recv, total) = (p.received_objects(), p.total_objects());
        let done = total > 0 && recv == total;
        if done || last_line.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) {
            last_line = Some(Instant::now());
            progress(Phase::Downloading { current: recv, total });
        }
        true
    });
    fo.remote_callbacks(cbs);
    remote
        .download(&[branch.as_str()], Some(&mut fo))
        .context("downloading from origin")?;
    let _ = remote.disconnect();
    update_tracking(&repo, &branch, theirs)?;
    let fetch_ms = t_fetch.elapsed().as_millis();
    log::info!(
        "pull: downloaded origin @ {} — ls-refs {ls_ms}ms, download {fetch_ms}ms, free heap {} ({} internal)",
        short(theirs),
        free_heap(),
        internal_free_heap()
    );

    // A working-copy file can differ from HEAD with the journal knowing nothing
    // about it — a card seeded on a computer, a desktop edit made straight onto
    // it. The belt in the apply would then refuse this pull and every later one,
    // since a refusal changes nothing that would clear it, and its advice to
    // resolve on a computer is exactly what `:gl` is not allowed to require.
    //
    // So fold that drift into a local commit, the same answer the pre-fetch step
    // gives a journaled save: the bytes on the card are what get committed (the
    // belt exists to keep them, and last-writer-wins is this sync's rule), the
    // rebase below replants them onto origin, and `:gs` finishes it. Costs one
    // extra hash of the paths this pull would touch — O(changed), and the belt
    // inside the apply still guards the writes, including the post-rebase ones
    // this pre-pass never sees.
    let drifted = drifted_paths(&tree_diff(&repo, head, theirs)?)?;
    if !drifted.is_empty() {
        log::warn!(
            "pull: {} working-copy path(s) differ from HEAD with no journal record ({}) — committing them locally so the pull is not refused",
            drifted.len(),
            drifted.iter().map(String::as_str).collect::<Vec<_>>().join(", ")
        );
        if let Some(committed) = stage_and_commit(&repo, &drifted)? {
            log::info!("pull: folded the drift into {}", short(committed));
            head = committed;
        }
    }

    if !repo
        .graph_descendant_of(theirs, head)
        .context("descendant check (fast-forward)")?
    {
        // Diverged: both sides moved. Rather than refuse, replant our local
        // commit(s) onto origin's tip so a plain `:gs` pushes them — no
        // computer needed. The branch ref moves LAST (after the card reflects
        // the rebased tree), so a power-pull mid-rebase leaves HEAD at the old
        // tip and the next `:gl` recomputes the identical commit idempotently.
        log::info!(
            "pull: origin {} and HEAD {} diverged — rebasing local work onto origin (last-writer-wins, no merge)",
            short(theirs),
            short(head)
        );
        // Covers the replay *and* the tree-apply behind it: one line for what the
        // writer experiences as a single phase, rather than a partial per step.
        progress(Phase::Rebasing);
        let t_rebase = Instant::now();
        let rebased = rebase_local_onto(&repo, head, theirs)?;
        let rebase_ms = t_rebase.elapsed().as_millis();

        // Nothing of ours survived the replay (our edits were already upstream):
        // collapse to a plain fast-forward onto origin.
        if rebased == theirs {
            let t_apply = Instant::now();
            let changed = apply_tree_diff(&repo, head, theirs)?;
            repo.reference(
                &format!("refs/heads/{branch}"),
                theirs,
                true,
                "typoena pull: fast-forward (local work already upstream)",
            )
            .context("fast-forwarding the branch ref")?;
            log::info!(
                "pull: local work already upstream — fast-forwarded {branch} to {} — apply {}ms ({changed} file(s))",
                short(theirs),
                t_apply.elapsed().as_millis()
            );
            return Ok(PullOutcome::Pulled(short(theirs)));
        }

        // Bring the card from our old tree to the rebased tree: origin's
        // remote-only changes are written, our own edits are already on disk
        // (unchanged in the diff, so untouched). Ref moves last.
        let t_apply = Instant::now();
        let changed = apply_tree_diff(&repo, head, rebased)?;
        repo.reference(
            &format!("refs/heads/{branch}"),
            rebased,
            true,
            "typoena pull: rebase local onto origin",
        )
        .context("moving the branch ref to the rebased commit")?;
        log::info!(
            "pull: rebased {} onto origin {} -> {} — rebase {rebase_ms}ms, apply {}ms ({changed} file(s)), free heap {} ({} internal)",
            short(head),
            short(theirs),
            short(rebased),
            t_apply.elapsed().as_millis(),
            free_heap(),
            internal_free_heap()
        );
        return Ok(PullOutcome::Rebased(short(rebased)));
    }

    // The ordinary pull shape: the SD writes are the tail of the wait, and they
    // scale with how much origin changed.
    progress(Phase::WritingFiles);
    let t_co = Instant::now();
    let changed = apply_tree_diff(&repo, head, theirs)?;
    repo.reference(
        &format!("refs/heads/{branch}"),
        theirs,
        true,
        "typoena pull: fast-forward",
    )
    .context("fast-forwarding the branch ref")?;
    log::info!(
        "pull: fast-forwarded {branch} {} -> {} — fetch {fetch_ms}ms, apply {}ms ({changed} file(s)), free heap {} ({} internal)",
        short(head),
        short(theirs),
        t_co.elapsed().as_millis(),
        free_heap(),
        internal_free_heap()
    );
    Ok(PullOutcome::Pulled(short(theirs)))
}

/// The `a`..`b` tree-to-tree diff both the belt and the apply work from.
fn tree_diff<'r>(repo: &'r Repository, a: Oid, b: Oid) -> Result<git2::Diff<'r>> {
    let a_tree = repo.find_commit(a)?.tree().context("old tree")?;
    let b_tree = repo.find_commit(b)?.tree().context("new tree")?;
    repo.diff_tree_to_tree(Some(&a_tree), Some(&b_tree), None)
        .context("diffing trees")
}

/// The paths in `diff` whose working copy no longer hashes to the OLD tree's
/// blob — the files an apply would clobber, edited behind git's back. Returned
/// repo-relative and sorted, ready for [`stage_and_commit`].
///
/// A missing file is not drift (nothing to clobber), and an Added delta has
/// nothing on disk to protect. Media paths are skipped for the same reason the
/// apply skips them: hashing one means materializing the whole blob in RAM.
fn drifted_paths(diff: &git2::Diff<'_>) -> Result<BTreeSet<String>> {
    use git2::Delta;

    let mut drifted = BTreeSet::new();
    for d in diff.deltas() {
        let (old, path) = match d.status() {
            Delta::Modified | Delta::Typechange | Delta::Deleted => {
                (d.old_file().id(), d.old_file().path())
            }
            _ => continue,
        };
        let Some(rel) = path.and_then(|p| p.to_str()) else {
            continue;
        };
        if is_media_path(rel) {
            continue;
        }
        match fs::read(format!("{REPO_DIR}/{rel}")) {
            Ok(bytes) => {
                let disk = Oid::hash_object(ObjectType::Blob, &bytes)
                    .with_context(|| format!("hashing {rel}"))?;
                if disk != old {
                    drifted.insert(rel.to_string());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("reading {rel}")),
        }
    }
    Ok(drifted)
}

/// Bring the working copy from `head`'s tree to `theirs`' tree by applying the
/// tree-to-tree diff directly: write each added/modified blob, unlink each
/// deleted path, and touch nothing else. Returns the number of files changed.
///
/// This is `checkout_tree`'s job, done the splice way: do NOT swap libgit2's
/// SAFE checkout back in — it readdirs the whole working directory over SPI
/// (O(tree), the wall the splice commit exists to avoid) and OOM'd internal
/// DRAM on the first on-device ff attempt (2026-07-14; spi_master null-derefs
/// on failed DMA alloc, see docs/postmortems/2026-07-11-editor-freeze-spi-dma-
/// oom-during-sync.md). The tree-to-tree diff skips identical subtree OIDs
/// wholesale, so diff and apply are both O(changed).
///
/// Safety belt (a rehash, kept O(changed)): before touching anything,
/// any to-be-overwritten/deleted file whose disk content no longer hashes to
/// the OLD tree's blob aborts the pull — edits made behind git's back (e.g.
/// desktop edits directly on the card) must not be clobbered. Device-side
/// saves are already covered by the pre-fetch commit in [`pull_once`].
///
/// A consequence of diffing *commits* rather than the working copy, hit on the
/// bench with `.typoena.toml` (2026-09-06): a path the two trees agree on is
/// never written, so a working copy that drifted on such a path stays drifted
/// through every later pull. The host re-reads the prefs file after a pull for
/// exactly this reason (`Runtime::reload_prefs`).
///
/// A refusal here is therefore self-perpetuating — nothing about it clears the
/// drift — which is why [`pull_once`] commits any drift it finds before calling
/// this, and why reaching the `bail!` means a path it could not fold.
///
/// Writes are unlink + tmp + rename (FAT f_rename won't overwrite), so a
/// power-pull mid-apply leaves at worst a `.gltmp` orphan with the ref NOT
/// yet moved — the next `:gl` re-applies idempotently.
///
/// Media paths are invisible to both passes — writing one materializes the
/// whole blob in RAM (16 MB PNGs vs 8 MB PSRAM, the one OOM path left in
/// `:gl`), and the belt hash skips them for the same reason. Blobs still
/// arrive in `.git` via the fetch; only the working-tree copy is skipped,
/// which is commit-safe because the splice stages explicit journal paths.
/// Full rationale: docs/notes/git-sync-images-and-repo-size.md.
fn apply_tree_diff(repo: &Repository, head: Oid, theirs: Oid) -> Result<usize> {
    use git2::Delta;

    let diff = tree_diff(repo, head, theirs)?;

    // Pass 1 — verify: refuse before the first write if any file we are about to
    // replace or remove was edited behind git's back. `pull_once` normally clears
    // this by committing the drift first, so reaching it here means a path it
    // could not fold, and a write would lose the only copy of those bytes.
    let drifted = drifted_paths(&diff)?;
    if let Some(rel) = drifted.iter().next() {
        bail!("local change in {rel} — pull refused (edit made behind git; resolve on a computer)");
    }

    // Pass 2 — apply.
    let mut changed = 0usize;
    let mut media_skipped = 0usize;
    for d in diff.deltas() {
        match d.status() {
            Delta::Added | Delta::Modified | Delta::Typechange => {
                let Some(rel) = d.new_file().path().and_then(|p| p.to_str()) else {
                    continue;
                };
                if is_media_path(rel) {
                    media_skipped += 1;
                    continue;
                }
                let abs = format!("{REPO_DIR}/{rel}");
                if let Some(dir) = std::path::Path::new(&abs).parent() {
                    fs::create_dir_all(dir).with_context(|| format!("mkdir for {rel}"))?;
                }
                let blob = repo
                    .find_blob(d.new_file().id())
                    .with_context(|| format!("reading blob for {rel}"))?;
                let tmp = format!("{abs}.gltmp");
                fs::write(&tmp, blob.content()).with_context(|| format!("writing {rel}"))?;
                let _ = fs::remove_file(&abs);
                fs::rename(&tmp, &abs).with_context(|| format!("landing {rel}"))?;
                changed += 1;
            }
            Delta::Deleted => {
                let Some(rel) = d.old_file().path().and_then(|p| p.to_str()) else {
                    continue;
                };
                if is_media_path(rel) {
                    media_skipped += 1;
                    continue;
                }
                match fs::remove_file(format!("{REPO_DIR}/{rel}")) {
                    Ok(()) => changed += 1,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e).with_context(|| format!("deleting {rel}")),
                }
            }
            _ => {}
        }
    }
    if media_skipped > 0 {
        log::info!(
            "pull: skipped {media_skipped} media file(s) — blobs live in .git, the card's working copy stays text-only"
        );
    }
    Ok(changed)
}

/// Rebase the device's local-only work onto origin's tip and return the new
/// commit id — a single squashed commit whose tree is origin's tree with our
/// edits spliced back on top. This is `:gl`'s answer to a divergence (a
/// stranded local commit while origin also moved): rather than refuse and send
/// the user to a computer, we replant our work on the new base so a plain `:gs`
/// pushes it.
///
/// Last-writer-wins by design, exactly like push's post-rejection reconcile
/// ([`reconcile_onto_origin`]): the replay set is the paths our side changed
/// since the fork point (`merge_base..head`), each spliced from the **card**
/// (the source of truth) onto origin's tree. A note both sides edited resolves
/// to ours; every remote-only change rides along by OID from origin's tree.
/// It is a rebase of one squashed commit, not a content merge — the device
/// still has no merge engine (that stays increment-B work).
///
/// Returns `theirs` unchanged when nothing of ours survives the replay (our
/// edits were already upstream) so the caller can collapse to a fast-forward
/// instead of writing an empty commit. The commit is created with
/// `commit(None, …)`: it does **not** move the branch ref. The caller applies
/// the merged tree to the card and moves the ref last, so a power-pull
/// mid-rebase leaves HEAD at the old tip and the next `:gl` recomputes the
/// identical commit idempotently.
fn rebase_local_onto(repo: &Repository, head: Oid, theirs: Oid) -> Result<Oid> {
    let base = repo
        .merge_base(head, theirs)
        .context("finding the merge-base to rebase onto origin")?;
    let base_tree = repo.find_commit(base)?.tree().context("merge-base tree")?;
    let head_tree = repo.find_commit(head)?.tree().context("HEAD tree")?;
    let their_commit = repo.find_commit(theirs)?;
    let their_tree = their_commit.tree().context("origin tree")?;
    let their_tree_id = their_tree.id();

    // Our side's changes since the fork are the replay set: splice each onto
    // origin's tree, read from the card (a path missing on disk splices out — a
    // local delete), mirroring stage_and_commit's working-tree-as-truth model.
    let diff = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)
        .context("diffing merge-base..HEAD for the replay set")?;
    let paths: BTreeSet<String> = diff
        .deltas()
        .filter_map(|d| {
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .and_then(|p| p.to_str())
                .map(str::to_string)
        })
        .collect();

    let mut tree = their_tree;
    for path in &paths {
        // Media is never a device commit (the splice stages journal paths only,
        // and the card is text-only) — but reading a stray 16 MB blob would OOM,
        // so skip it and keep origin's version, consistent with apply_tree_diff.
        if is_media_path(path) {
            log::warn!("rebase: skipping media path {path} in the replay set (kept origin's version)");
            continue;
        }
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let blob = match fs::read(format!("{REPO_DIR}/{path}")) {
            Ok(bytes) => Some(repo.blob(&bytes).with_context(|| format!("blob for {path}"))?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {path}")),
        };
        let spliced = splice(repo, Some(&tree), &parts, blob)
            .with_context(|| format!("splicing {path} onto origin"))?;
        tree = repo.find_tree(spliced).context("loading spliced tree")?;
    }

    // Nothing of ours survived (edits already upstream) — signal a fast-forward.
    if tree.id() == their_tree_id {
        return Ok(theirs);
    }

    let sig = Signature::now(author_name(), author_email()).context("building signature")?;
    let message = format!("Typoena rebase onto origin — unix {}", now_unix());
    repo.commit(None, &sig, &sig, &message, &tree, &[&their_commit])
        .context("creating the rebased commit")
}

/// Paths [`apply_tree_diff`] never writes, deletes, or belt-hashes: binary
/// media the device can't render and can't afford to hold in RAM. Matched by
/// extension, case-insensitive. Text-ish assets (svg, csv…) stay eligible —
/// the criterion is blob size risk, not "is it a note".
fn is_media_path(rel: &str) -> bool {
    const MEDIA_EXT: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "bmp", "webp", "heic", "tiff", "ico", "pdf", "mp3", "mp4",
        "m4a", "wav", "mov", "avi", "mkv", "zip",
    ];
    std::path::Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA_EXT.iter().any(|m| e.eq_ignore_ascii_case(m)))
}

/// Auth + cert callbacks shared by fetch and push. Captures only statics
/// (card conf / baked consts), so a fresh set can be built per operation. The
/// token is handed to libgit2 here and never logged.
fn auth_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut cbs = RemoteCallbacks::new();
    cbs.credentials(|_url, _user_from_url, allowed| {
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            return Cred::userpass_plaintext(gh_user(), token());
        }
        Err(git2::Error::from_str(
            "server did not offer USER_PASS_PLAINTEXT — cannot authenticate with a token",
        ))
    });
    cbs.certificate_check(|_cert, host| {
        log::info!("verifying {host} TLS chain against embedded GitHub CA bundle");
        Ok(CertificateCheckStatus::CertificatePassthrough)
    });
    cbs
}

/// Write the embedded GitHub root CAs to the card and point libgit2's mbedTLS
/// stream at them. Must run before any TLS. Mirrors the spike, but writes to the
/// card root (`/sd/ca.pem`) instead of flash-FAT.
fn install_tls_trust_store() -> Result<()> {
    std::fs::write(CA_BUNDLE_PATH, GITHUB_ROOTS_PEM)
        .with_context(|| format!("writing CA bundle to {CA_BUNDLE_PATH}"))?;
    // SAFETY: sets a process-global libgit2 option once, before any TLS work.
    unsafe { git2::opts::set_ssl_cert_file(CA_BUNDLE_PATH) }
        .context("git2::opts::set_ssl_cert_file")?;
    log::info!(
        "TLS trust store installed — {} B of GitHub roots at {CA_BUNDLE_PATH}",
        GITHUB_ROOTS_PEM.len()
    );
    Ok(())
}

/// A short, panel-friendly reason from an error chain (first line, clamped),
/// prefixed with the operation ("sync" / "pull"). The full chain is logged
/// separately; the editor clamps this to the panel width.
fn short_reason(op: &str, e: &anyhow::Error) -> String {
    // The ROOT cause, not the outermost context. Our `.context(...)` strings are
    // gerund phrases naming the step ("installing 0.11.0", "connecting Wi-Fi"),
    // so reporting the outer layer renders every failure as a progress line the
    // writer waits on forever. The root is the thing that actually broke.
    let full = format!("{}", e.root_cause());
    let first = full.lines().next().unwrap_or("failed");
    // "FAILED" so a glance can never read it as status; the panel word-wraps this
    // over at most NOTICE_MAX_LINES x PANEL_COLS, and `{e:?}` above has already
    // put the whole chain in the log and on the card.
    format!("{op} FAILED: {}", first.chars().take(40).collect::<String>())
}

/// First 8 hex chars of an OID, for readable logs and the panel.
fn short(oid: git2::Oid) -> String {
    let mut s = oid.to_string();
    s.truncate(8);
    s
}

fn free_heap() -> u32 {
    unsafe { sys::esp_get_free_heap_size() }
}

/// Free INTERNAL RAM (DRAM), excluding PSRAM. `free_heap` is dominated by the
/// 8 MB PSRAM pool and masks internal exhaustion — which is what actually
/// killed the first real-repo push (mbedTLS's ssl_setup could not get its
/// ~33 KB while Wi-Fi + USB + editor + libgit2 were resident).
fn internal_free_heap() -> u32 {
    unsafe { sys::heap_caps_get_free_size(sys::MALLOC_CAP_INTERNAL) as u32 }
}

/// The two accumulating terms of the FD budget, once per sync (never per
/// keystroke): how many packs the odb can hold descriptors for, against the cap
/// `tune_libgit2` set, and how often TLS teardown took the branch that used to
/// leak a socket. One pack arrives per downloading `:gl` and nothing ever
/// repacks, so a count that climbs session over session next to a `load_path
/// FAILED … errno=23` is the descriptor-exhaustion story in two lines.
fn log_odb_inventory() {
    let packs = fs::read_dir(format!("{REPO_DIR}/.git/objects/pack"))
        .map(|d| {
            d.flatten()
                .filter(|e| {
                    let p = e.path();
                    p.extension().and_then(|x| x.to_str()) == Some("pack")
                })
                .count()
        })
        .unwrap_or(0);
    // SAFETY: reads a libgit2 global and a C file-static counter; the net thread
    // is the only caller and `tune_libgit2` has already run on it.
    let (limit, tls_teardown_failures) = unsafe {
        (
            git2::opts::get_mwindow_file_limit().unwrap_or(0),
            esp_tls_teardown_failures(),
        )
    };
    log::info!(
        target: DIAG,
        "odb inventory: {packs} pack(s), mwindow_file_limit={limit}, tls_teardown_failures={tls_teardown_failures}"
    );
}

fn min_free_heap() -> u32 {
    unsafe { sys::esp_get_minimum_free_heap_size() }
}

unsafe extern "C" {
    /// Counters from the p_mmap emulation in `components/libgit2/esp_map.c`.
    /// Post cache-removal: `hits` is always 0, `misses` counts every mapping,
    /// `cached_kb` reports the LIVE mapped bytes — every libgit2 "mmap"
    /// (mwindow windows AND whole-file pack .idx maps) is a real PSRAM malloc
    /// there, so this splits map memory from everything else git allocates.
    fn esp_map_stats(hits: *mut u32, misses: *mut u32, read_kb: *mut u32, cached_kb: *mut u32);

    /// TLS teardowns whose `close_notify` errored this power session
    /// (`components/libgit2/esp_mbedtls_stream.c`). Routine when the peer resets
    /// first; tracked because that branch used to leak the socket fd.
    fn esp_tls_teardown_failures() -> u32;
}

/// The p_mmap emulation's cumulative counters: (mappings created, KB read).
/// Deltas around an operation attribute its pack-read I/O (each mapping is a
/// real lseek+read over SPI in esp_map.c).
fn map_counters() -> (u32, u32) {
    let (mut maps, mut read_kb) = (0u32, 0u32);
    // SAFETY: esp_map_stats only writes the non-null out-params.
    unsafe { esp_map_stats(std::ptr::null_mut(), &mut maps, &mut read_kb, std::ptr::null_mut()) };
    (maps, read_kb)
}

/// One-line heap + odb-cache + mmap snapshot for the push path. Runs 4–6
/// (2026-07-13) each spent ~65 s inside `remote.push()` while something
/// consumed ~6 MB of PSRAM (run 4: the UI aborted on a framebuffer alloc;
/// runs 5–6: a ~7 KB inflateInit failed inside the pack build; run 6 pinned
/// min-ever PSRAM at 684 B with the odb cache at 59 KB — exonerated). These
/// lines exist to name the consumer: if `mmap live` tracks the PSRAM drop the
/// eater is mwindow windows / idx maps, otherwise it's non-map allocations
/// (parsed objects, delta chains). `largest PSRAM` distinguishes exhaustion
/// from fragmentation, and the min-evers survive to the failure log even when
/// the spike itself fell between two callbacks.
fn log_push_heap(stage: &str) {
    let (mut cached, mut allowed): (isize, isize) = (0, 0);
    // SAFETY: GET_CACHED_MEMORY only writes the two out-params.
    unsafe {
        libgit2_sys::git_libgit2_opts(
            libgit2_sys::GIT_OPT_GET_CACHED_MEMORY as i32,
            &mut cached as *mut isize,
            &mut allowed as *mut isize,
        );
    }
    let (largest_psram, min_psram, min_internal) = unsafe {
        (
            sys::heap_caps_get_largest_free_block(sys::MALLOC_CAP_SPIRAM),
            sys::heap_caps_get_minimum_free_size(sys::MALLOC_CAP_SPIRAM),
            sys::heap_caps_get_minimum_free_size(sys::MALLOC_CAP_INTERNAL),
        )
    };
    let (mut maps, mut read_kb, mut live_kb) = (0u32, 0u32, 0u32);
    // SAFETY: esp_map_stats only writes the non-null out-params.
    unsafe { esp_map_stats(std::ptr::null_mut(), &mut maps, &mut read_kb, &mut live_kb) };
    log::info!(
        "push heap [{stage}]: free {} ({} internal), largest PSRAM {}, min-ever PSRAM {} / internal {}, mmap live {live_kb} KB ({maps} maps, {read_kb} KB read), odb cache {}/{} KB",
        free_heap(),
        internal_free_heap(),
        largest_psram,
        min_psram,
        min_internal,
        cached / 1024,
        allowed / 1024
    );
}

use crate::infrastructure::storage_sd::Storage;

/// [`app::NetService`] backed by the net thread (git `:gs`/`:gl` + `:update`
/// OTA). Owns the request/outcome channels and a handle to the card's dirty
/// journal, which it takes on push and settles when the outcome lands (OTA
/// touches no journal). Behind the `full` feature — it pulls libgit2.
pub struct NetService {
    card: Rc<Storage>,
    tx: Sender<NetRequest>,
    rx: Receiver<NetOutcome>,
    /// Whether the in-flight pull is a discard — read by
    /// [`poll_outcome`](app::NetService::poll_outcome) to settle the journal.
    /// A discard rolls the working copy back *before* the fetch, so its paths
    /// are clean even if the fetch then fails, and must not be returned to
    /// pending the way a failed commit-and-pull's are. `Cell` because the port
    /// takes `&self`; this whole adapter lives on the UI task only.
    discarding: std::cell::Cell<bool>,
}

impl NetService {
    pub fn new(card: Rc<Storage>, tx: Sender<NetRequest>, rx: Receiver<NetOutcome>) -> Self {
        Self { card, tx, rx, discarding: std::cell::Cell::new(false) }
    }
}

impl app::NetService for NetService {
    fn push(&self) -> app::PushDispatch {
        let paths = self.card.take_dirty();
        match self.tx.send(NetRequest::Push(PushRequest { paths })) {
            Ok(()) => app::PushDispatch::Dispatched,
            Err(_) => {
                // Thread gone — nothing will report back, so return the snapshot
                // to pending ourselves.
                self.card.push_failed();
                app::PushDispatch::ThreadDown
            }
        }
    }

    fn pull(&self, intent: editor::PullIntent) -> app::PullDispatch {
        // A bare `:gl` with unpushed saves doesn't refuse anymore — it folds
        // them into a local commit first so the fetch can rebase them onto
        // origin. But that commit is user-visible, so the first pass hands the
        // UI the journal's paths to show and ask about; the answer arrives as a
        // second pull carrying Commit or Discard.
        if intent == editor::PullIntent::Ask && self.card.has_dirty() {
            let files = self
                .card
                .dirty_paths()
                .into_iter()
                .map(|path| {
                    // A journal entry whose file is gone is a `:delete` the
                    // remote hasn't seen — worth tagging on the card, since it
                    // is the one row a discard *restores* rather than removes.
                    let deleted = !std::path::Path::new(&format!("{REPO_DIR}/{path}")).is_file();
                    editor::Unsynced { path, deleted }
                })
                .collect::<Vec<_>>();
            log::info!(
                ":gl — {} unsynced path(s); asking the UI before the pre-fetch commit",
                files.len()
            );
            return app::PullDispatch::NeedsConfirm(files);
        }
        let discard = intent == editor::PullIntent::Discard;
        self.discarding.set(discard);
        // Snapshot the journal into `in_flight` (empty when nothing was dirty);
        // the net thread commits (or discards) those paths before fetching, and
        // poll_outcome settles the snapshot when the outcome lands — succeeded
        // forgets it, failed returns it to pending — exactly as push does.
        let paths = self.card.take_dirty();
        match self.tx.send(NetRequest::Pull(PullRequest { paths, discard })) {
            Ok(()) => app::PullDispatch::Dispatched,
            Err(_) => {
                // Thread gone — nothing will report back, so return the snapshot
                // to pending ourselves (mirrors push's ThreadDown path). Nothing
                // was discarded either: the rollback runs on that thread.
                self.discarding.set(false);
                self.card.push_failed();
                app::PullDispatch::ThreadDown
            }
        }
    }

    fn update(&self) -> app::UpdateDispatch {
        // No dirty journal to snapshot — OTA doesn't touch the working copy.
        match self.tx.send(NetRequest::Update) {
            Ok(()) => app::UpdateDispatch::Dispatched,
            Err(_) => app::UpdateDispatch::ThreadDown,
        }
    }

    fn sync_clock(&self) -> app::ClockDispatch {
        // Touches neither the working copy nor the dirty journal.
        match self.tx.send(NetRequest::Clock) {
            Ok(()) => app::ClockDispatch::Dispatched,
            Err(_) => app::ClockDispatch::ThreadDown,
        }
    }

    fn poll_outcome(&self) -> Option<app::NetOutcome> {
        // Coalesce progress: the UI drains one message per loop pass and paints
        // it, so a burst that queued while it was busy elsewhere would otherwise
        // replay line by line — at a full-panel partial each — and trail the
        // terminal notice with stale counts. Keep only the newest line, and let a
        // terminal outcome behind it win outright.
        let mut latest = None;
        let outcome = loop {
            match self.rx.try_recv() {
                Ok(NetOutcome::Progress(line)) => latest = Some(line),
                Ok(terminal) => break terminal,
                Err(_) => return latest.map(app::NetOutcome::Progress),
            }
        };
        Some(match outcome {
            // Nothing to settle: the wall clock is the result, and the UI reads
            // it back through `app::Clock` on its next pass.
            NetOutcome::Clock(o) => app::NetOutcome::Clock(match o {
                ClockOutcome::Synced => app::ClockOutcome::Synced,
                ClockOutcome::Failed(reason) => app::ClockOutcome::Failed(reason),
            }),
            NetOutcome::Update(o) => app::NetOutcome::Update(match o {
                // OTA settles no dirty journal; just mirror the outcome across
                // the app boundary.
                UpdateOutcome::Installed(v) => app::UpdateOutcome::Installed(v),
                UpdateOutcome::UpToDate(v) => app::UpdateOutcome::UpToDate(v),
                UpdateOutcome::Failed(reason) => app::UpdateOutcome::Failed(reason),
            }),
            NetOutcome::Push(o) => {
                // Settle the dirty snapshot this push took: confirmed
                // pushed (or up to date) → forget it; failed → back to pending.
                match &o {
                    PushOutcome::Pushed(_) | PushOutcome::UpToDate => {
                        self.card.push_succeeded()
                    }
                    PushOutcome::Failed(_) => self.card.push_failed(),
                }
                app::NetOutcome::Push(match o {
                    PushOutcome::Pushed(oid) => app::PushOutcome::Pushed(oid),
                    PushOutcome::UpToDate => app::PushOutcome::UpToDate,
                    PushOutcome::Failed(reason) => app::PushOutcome::Failed(reason),
                })
            }
            NetOutcome::Pull(o) => {
                // Settle the dirty snapshot this pull took (it folds unpushed
                // saves into a local commit before fetching, like push):
                // integrated → forget it (the work is committed, and a stranded
                // local commit is pushed by the next `:gs`); failed → back to
                // pending. Empty snapshot → both are no-ops.
                //
                // A discard forgets them either way: the rollback ran before
                // the fetch, so those paths match HEAD now whatever the fetch
                // did, and returning them to pending would re-offer files that
                // no longer differ from anything.
                match &o {
                    PullOutcome::Failed(_) if !self.discarding.get() => self.card.push_failed(),
                    _ => self.card.push_succeeded(),
                }
                self.discarding.set(false);
                app::NetOutcome::Pull(match o {
                    PullOutcome::Pulled(oid) => app::PullOutcome::Pulled(oid),
                    PullOutcome::Rebased(oid) => app::PullOutcome::Rebased(oid),
                    PullOutcome::UpToDate => app::PullOutcome::UpToDate,
                    PullOutcome::LocalAhead => app::PullOutcome::LocalAhead,
                    PullOutcome::Failed(reason) => app::PullOutcome::Failed(reason),
                })
            }
            // The drain above already consumed every Progress, so this can't be
            // reached — mapped rather than `unreachable!()`ed to keep the panic
            // ban intact (a wrong-but-harmless notice beats a scribe + reboot).
            NetOutcome::Progress(line) => app::NetOutcome::Progress(line),
        })
    }
}
