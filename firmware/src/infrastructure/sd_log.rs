//! Card-side log mirror — a copy of the serial log on the SD card, so a failure
//! that happens untethered still leaves a trace.
//!
//! [`init`] takes the `log` crate's single logger slot and tees every record:
//! first to the esp-idf UART logger (same format, same
//! `CONFIG_LOG_MAXIMUM_LEVEL` filtering as before), then — for the records that
//! pass the card gate — into a RAM buffer a background thread appends to
//! [`LOG_PATH`]. The target is the CARD ROOT, next to
//! [`panic_scribe::EMERGENCY_PATH`](super::panic_scribe::EMERGENCY_PATH), so a
//! log is never swept into a sync commit.
//!
//! ## Why a RAM buffer and not an open file
//!
//! The mount has one shared pool of 16 FatFS descriptors (`MAX_FILES_GIT`, see
//! [`super::storage_sd`]), and a downloading `:gl` already peaks around 10 of
//! them. Three shapes were possible:
//!
//! - **hold the file open** — 1 descriptor, fast, but it spends 1/16 of that
//!   pool for the whole power session. Exhaustion is the leading suspect for the
//!   "can't open README" this sink exists to diagnose, so a hold-open sink would
//!   make the bug it measures more likely. Refused.
//! - **open-append-close per record** — 0 resident descriptors, but tens of ms
//!   of FatFS latency on the UI task for every `log::` call, i.e. a stall per
//!   keystroke. Refused.
//! - **RAM buffer, flushed on a timer from a background thread** — 0 resident
//!   descriptors, one for a few ms per flush, and the UI task only ever pays a
//!   mutex-guarded `write!` into a `String`. Chosen.
//!
//! The flusher holds the buffer lock ONLY for the swap; the SD write runs
//! unlocked, so a `log::` call on the UI task can never wait behind FatFS. It is
//! pinned to Core1 at priority 1 — an unpinned `std::thread` runs at priority 5
//! against the main task's 1 and starves the UI (already hit once here, on the
//! palette file walk; see [`super::file_index`]).
//!
//! ## What reaches the card
//!
//! WARN and ERROR from any target, plus anything on the [`DIAG`] target (the
//! per-sync diagnostics that are info-level context, not warnings). A healthy
//! typing session therefore writes **nothing at all**: `info!` fires roughly
//! once per render batch, and mirroring that would be per-keystroke SD traffic.
//! The flusher wakes, finds an empty buffer, and sleeps.
//!
//! Degradation is silent by contract: no card, a full card, a failed write — the
//! records are dropped and UART logging continues. Nothing here panics, blocks
//! the writer, or propagates an error to the editor.

use std::fs;
use std::io::Write as _;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use core::fmt::Write as _;

use esp_idf_svc::hal::cpu::Core;
use esp_idf_svc::hal::task::thread::ThreadSpawnConfiguration;
use esp_idf_svc::log::{EspIdfLogFilter, EspLogger};
use esp_idf_svc::sys;
use log::{Level, Log, Metadata, Record};

/// Card-root log target — outside `/sd/repo`, so it is never staged by a sync.
pub const LOG_PATH: &str = "/sd/typoena.log";
/// The previous generation, kept so the window around a failure survives one
/// rotation.
pub const LOG_PREV: &str = "/sd/typoena.log.1";

/// Log target whose records reach the card at ANY level. The per-sync
/// diagnostics (`odb inventory`) are info-level context rather than warnings:
/// promoting them to `warn!` would misuse the level and pollute the serial warn
/// stream, so they carry this target instead.
pub const DIAG: &str = "typoena-diag";

/// Bound on ONE generation. Two generations, so the card never holds more than
/// 256 KiB of log whatever happens.
const MAX_BYTES: u64 = 128 * 1024;

/// Bound on the RAM buffer between flushes. Once it is reached records are
/// dropped and counted rather than buffered — the editor's heap is not the log's
/// to spend. The check is before the write, so the buffer can overshoot by the
/// one record that crossed the line and no more.
const BUF_MAX: usize = 16 * 1024;

/// Flush period. Long enough that a healthy session's SD traffic is zero and a
/// busy one pays one open+append per two seconds; short enough that a crash
/// loses at most that window (the panic hook flushes what is left, see
/// [`flush_blocking`]).
const FLUSH_MS: u64 = 2000;

/// Pending records, newline-terminated. Seeded in [`init`] with a capacity past
/// the 16 KB SPIRAM-malloc threshold so it lands in PSRAM rather than the tight
/// internal DRAM — the same trick the palette's path blob uses.
static BUF: Mutex<String> = Mutex::new(String::new());

/// Records the buffer had no room for since the last flush.
static DROPPED: AtomicU32 = AtomicU32::new(0);

/// The UART half of the tee. Hand-built rather than
/// `EspLogger::initialize_default()` because esp-idf-svc's own integrated
/// logger static is private, and mirroring needs to own `log::set_logger`.
/// Same type, same filter, so the serial format and the
/// `CONFIG_LOG_MAXIMUM_LEVEL` gating are unchanged.
static UART: EspLogger = EspLogger::new(EspIdfLogFilter::new());

static TEE: Tee = Tee;

struct Tee;

impl Log for Tee {
    fn enabled(&self, metadata: &Metadata) -> bool {
        UART.enabled(metadata) || wants_card(metadata.level(), metadata.target())
    }

    fn log(&self, record: &Record) {
        // UART first and unconditionally: it re-checks its own filter, and it is
        // the sink that must never be affected by anything below.
        UART.log(record);

        if !wants_card(record.level(), record.target()) {
            return;
        }
        let marker = match record.level() {
            Level::Error => 'E',
            Level::Warn => 'W',
            Level::Info => 'I',
            Level::Debug => 'D',
            Level::Trace => 'V',
        };
        let ts = unsafe { sys::esp_log_timestamp() };
        let mut buf = lock();
        if buf.len() >= BUF_MAX {
            DROPPED.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let _ = writeln!(*buf, "{marker} ({ts}) {}: {}", record.target(), record.args());
    }

    /// Push whatever is buffered to the card. Does real SD I/O on the calling
    /// thread; nothing in the firmware calls it on the hot path.
    fn flush(&self) {
        flush_blocking();
    }
}

fn wants_card(level: Level, target: &str) -> bool {
    level <= Level::Warn || target == DIAG
}

/// Absorb mutex poisoning: a panic while a record was being appended must not
/// take logging down with it.
fn lock() -> std::sync::MutexGuard<'static, String> {
    BUF.lock().unwrap_or_else(|e| e.into_inner())
}

/// Install the tee as the process logger. Call once, before any other
/// esp-idf-svc call that logs; a second call is a silent no-op (esp-idf's own
/// initializer unwraps internally and would abort the boot, so this never
/// delegates to it). Records made before [`start`] accumulate in RAM and land in
/// the first flush, so the boot window — EPD bring-up, the mount, the wizard
/// gate — is on the card too.
pub fn init() {
    *lock() = String::with_capacity(BUF_MAX + 1024);
    // What `initialize_default` does beyond installing the logger: seeds
    // `log::set_max_level` from CONFIG_LOG_MAXIMUM_LEVEL. Unconditional, because
    // the `log` crate's default max level is Off — a second `init` that lost the
    // logger slot must not leave the winner muted.
    UART.filter().initialize();
    let _ = log::set_logger(&TEE);
}

/// Start the flusher, once the card is mounted. Best-effort: if the thread
/// cannot be spawned the buffer simply stays in RAM, bounded, and UART logging
/// is unaffected.
///
/// This call is the whole card sink's only switch. Not calling it costs no
/// thread and no SD write, leaves the diagnostics on UART, and caps the buffer
/// at [`BUF_MAX`] — so a bench session that suspects the sink of costing the
/// writer latency can rule it out by dropping this one line.
pub fn start() {
    // Background-task scheduling for the spawn below, mirroring the palette
    // walk's: Core1 keeps it off the UI core, priority 1 matches the main task.
    // `..Default::default()` keeps the esp-idf-version-specific fields at their
    // defaults.
    let cfg = ThreadSpawnConfiguration {
        name: None,
        stack_size: 8 * 1024,
        priority: 1,
        inherit: false,
        pin_to_core: Some(Core::Core1),
        ..Default::default()
    };
    if let Err(e) = cfg.set() {
        log::warn!("sd log thread cfg (Core1, prio 1) FAILED ({e}); spawning at pthread default");
    }

    let spawned = std::thread::Builder::new()
        .name("sdlog".into())
        .stack_size(8 * 1024)
        .spawn(flusher);
    if let Err(e) = spawned {
        log::warn!("sd log thread spawn FAILED ({e}); serial only this session");
    }

    // Restore the pthread default so later spawns from this (UI) thread aren't
    // silently pinned to Core1 / deprioritised.
    if let Err(e) = ThreadSpawnConfiguration::default().set() {
        log::warn!("restoring default thread cfg FAILED ({e})");
    }
}

fn flusher() {
    let mut size = fs::metadata(LOG_PATH).map(|m| m.len()).unwrap_or(0);
    // The buffer's twin: swapped in so the live buffer keeps its PSRAM capacity
    // instead of being re-grown from zero after every flush.
    let mut chunk = String::with_capacity(BUF_MAX + 1024);
    let mut complained = false;
    loop {
        std::thread::sleep(Duration::from_millis(FLUSH_MS));
        chunk.clear();
        {
            let mut buf = lock();
            if buf.is_empty() {
                continue;
            }
            std::mem::swap(&mut *buf, &mut chunk);
        }
        let dropped = DROPPED.swap(0, Ordering::Relaxed);
        if dropped > 0 {
            let _ = writeln!(chunk, "W (0) {DIAG}: sd log: {dropped} record(s) dropped, buffer full");
        }
        // Appending is deliberately NOT mutually exclusive with buffering: the
        // lock is already released, `append` reaches no Rust `log::` call, and
        // muting here would silently lose the records emitted during the write —
        // on a slow card, exactly the window the interesting ones fall in.
        let outcome = append(&chunk, &mut size);
        if let Err(e) = outcome {
            // Once per session: the card is the thing that just failed, so
            // repeating this every two seconds would only fill the serial log.
            if !complained {
                complained = true;
                log::warn!("sd log: append to {LOG_PATH} FAILED ({e}); serial only");
            }
        }
    }
}

/// Append `chunk`, rotating first when it would push the file past
/// [`MAX_BYTES`]. `size` is the caller's running byte count of [`LOG_PATH`],
/// updated in place so a healthy flush costs no `stat`.
///
/// No `fsync`: FatFS commits the sector and the directory entry at `f_close`,
/// which the `File` drop below performs, and a diagnostic log is not worth an
/// explicit sync per flush.
fn append(chunk: &str, size: &mut u64) -> std::io::Result<()> {
    if size.saturating_add(chunk.len() as u64) > MAX_BYTES {
        // FatFS f_rename refuses an existing destination (the same constraint
        // the atomic save is built around), so drop the old generation first.
        // If the rotation itself fails, truncate rather than let the file grow.
        let _ = fs::remove_file(LOG_PREV);
        if fs::rename(LOG_PATH, LOG_PREV).is_err() {
            let _ = fs::remove_file(LOG_PATH);
        }
        *size = 0;
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(LOG_PATH)?;
    f.write_all(chunk.as_bytes())?;
    *size = size.saturating_add(chunk.len() as u64);
    Ok(())
}

/// Last-chance flush, for the panic hook: the warnings that precede a crash are
/// exactly the ones worth keeping, and the reboot would otherwise take them.
///
/// HAZARD: `try_lock`, never `lock`. A panic taken while a record was being
/// appended holds the buffer mutex, and blocking on it here would hang the
/// device instead of letting esp-idf's abort handler reboot it.
pub fn flush_blocking() {
    let Ok(mut buf) = BUF.try_lock() else {
        return;
    };
    if buf.is_empty() {
        return;
    }
    let chunk = std::mem::take(&mut *buf);
    drop(buf);
    let mut size = fs::metadata(LOG_PATH).map(|m| m.len()).unwrap_or(0);
    let _ = append(&chunk, &mut size);
}
