//! The esp wall-clock + timezone driver, behind [`app::Clock`].
//!
//! `EspClock` gives the run loop today's date (for dated `:inbox` notes) and the
//! idle-yield tick. `apply_timezone` is boot glue — it sets libc's `TZ` so
//! `localtime_r` reads the local calendar day. Both compile into every build
//! (the light editor still dates notes and yields the CPU).

use esp_idf_svc::hal::delay::FreeRtos;

/// [`app::Clock`] over the esp wall clock and the FreeRtos tick.
pub struct EspClock;

impl app::Clock for EspClock {
    fn today(&self) -> Option<editor::Date> {
        today_date()
    }
    fn idle_yield(&self) {
        FreeRtos::delay_ms(8);
    }
}

/// Today's date from the wall clock, or `None` when the clock is not yet
/// trustworthy. The editor boot path never runs SNTP, so the clock sits at the
/// epoch until a `:gl`/`:gp` sync sets it this power cycle (no battery-backed
/// RTC) — a year before 2020 means "unset". Honours the timezone applied at boot.
fn today_date() -> Option<editor::Date> {
    let mut now: esp_idf_svc::sys::time_t = 0;
    let mut t: esp_idf_svc::sys::tm = unsafe { core::mem::zeroed() };
    // SAFETY: `now`/`t` are valid, owned locals; `time` fills `now`, `localtime_r`
    // fills `t` from it (the reentrant form writes into our `t`, no shared state).
    unsafe {
        esp_idf_svc::sys::time(&mut now);
        esp_idf_svc::sys::localtime_r(&now, &mut t);
    }
    let year = t.tm_year + 1900;
    if year < 2020 {
        return None; // clock unset (still at the epoch) — no sync yet this boot
    }
    Some(editor::Date {
        year,
        month: (t.tm_mon + 1) as u32, // tm_mon is 0-11
        day: t.tm_mday as u32,
    })
}

/// Apply a POSIX `TZ` string to libc so `localtime_r` reads the local calendar
/// day (see `Prefs::timezone`). newlib carries no zoneinfo database, so `tz` must
/// be the POSIX form (`CET-1CEST,M3.5.0,M10.5.0/3`), never an IANA name
/// (`Europe/Paris`) — the latter silently stays UTC. Best-effort: an interior NUL
/// or a failed `setenv` just leaves the previous zone (UTC) in place.
pub fn apply_timezone(tz: &str) {
    let Ok(c_tz) = std::ffi::CString::new(tz) else {
        log::warn!("timezone {tz:?} has an interior NUL; left at UTC");
        return;
    };
    // SAFETY: both pointers are valid C strings for the call; `tzset` reads the
    // `TZ` env var we just set. `1` = overwrite any existing value.
    unsafe {
        esp_idf_svc::sys::setenv(c"TZ".as_ptr(), c_tz.as_ptr(), 1);
        esp_idf_svc::sys::tzset();
    }
    log::info!("timezone applied: TZ={tz}");
}
