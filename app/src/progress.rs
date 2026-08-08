//! The in-flight phases a long network operation reports to the panel.
//!
//! One home for the wording *and* for how it reads. The same phase is reached
//! from four backends — the wizard's clone, `:gs`, `:gl`, `:update` — so the
//! label lives here rather than at each call site, and each backend hands its
//! sink a [`Phase`] instead of a string. That leaves exactly one place that turns
//! a phase into a line ([`Phase::label`]), which is the seam any change to how
//! in-flight state *reads* goes through. It sits in `app` (not beside the
//! transports, which are firmware) for the same reason the outcome types do: this
//! is UI vocabulary, and it is testable off the xtensa target.
//!
//! **No animation lives here, deliberately.** A panel line costs a whole-panel
//! area partial (~630 ms of e-paper drive, ~265 ms with `fast_partial`) and one
//! of the 64-partial ghosting budget, so a timer-driven spinner would drive the
//! panel most of a sync and pull the black full refresh forward to pay for dots.
//! Phases are emitted on real state change and time-gated by their callers
//! instead. If a static phase ever does read as frozen on device, this is where
//! the fix belongs — once, not at thirteen call sites.

/// A phase of a network operation, in flight. Counted variants carry raw
/// libgit2/HTTP numbers; [`label`](Phase::label) decides how they read.
pub enum Phase {
    /// Radio bring-up, one variant per step: each is its own multi-second wait on
    /// a cold operation (~3.65 s / ~2.1 s / ~0.3 s measured), and a warm one skips
    /// all three — so they are separate phases rather than one "connecting".
    JoiningWifi,
    SettingClock,
    VerifyingTls,
    /// The ref advertisement — "has origin anything new?". Both no-download pull
    /// shapes (up to date, local ahead) end here, so on most pulls this is the
    /// only phase the writer ever sees.
    ContactingOrigin,
    /// libgit2's pack build, ahead of the first byte sent.
    Packing { current: usize, total: usize },
    /// Pack bytes going out to origin.
    Sending { current: usize, total: usize },
    /// Objects coming in — the pull's fetch and the clone's shallow fetch.
    Downloading { current: usize, total: usize },
    /// Origin moved under our push: a fetch, a replay, and a second handshake, so
    /// the wait visibly grows past the counts already shown.
    Retrying,
    /// Replanting local commits onto origin. Covers the tree-apply behind it —
    /// one line for what the writer experiences as a single phase.
    Rebasing,
    /// Working-copy writes: the pull's tail, the clone's bulk.
    WritingFiles,
    /// OTA image bytes into the inactive slot (download and flash are one
    /// interleaved stream, hence one phase). `kb` is what has landed so far.
    InstallingImage { kb: usize },
}

impl Phase {
    /// The panel line for this phase. Kept lowercase and short — it renders in
    /// the side-panel notice, which a keystroke dismisses.
    pub fn label(&self) -> String {
        match self {
            Self::JoiningWifi => "joining wi-fi".to_string(),
            Self::SettingClock => "setting the clock".to_string(),
            Self::VerifyingTls => "verifying tls".to_string(),
            Self::ContactingOrigin => "contacting origin".to_string(),
            Self::Packing { current, total } => count("packing", *current, *total),
            Self::Sending { current, total } => count("sending", *current, *total),
            Self::Downloading { current, total } => count("downloading", *current, *total),
            Self::Retrying => "origin moved - retrying".to_string(),
            Self::Rebasing => "rebasing".to_string(),
            Self::WritingFiles => "writing files".to_string(),
            Self::InstallingImage { kb } => format!("installing {kb} KB"),
        }
    }
}

/// `verb current/total`, degrading as the total becomes known. libgit2 reports
/// `total` = 0 through libgit2's whole AddingObjects stage and before a fetch has
/// parsed the advertisement, and a bare "3/0" on the panel reads as a bug — so
/// count up until there is a denominator worth showing.
fn count(verb: &str, current: usize, total: usize) -> String {
    if total > 0 {
        format!("{verb} {current}/{total}")
    } else if current > 0 {
        format!("{verb} {current}")
    } else {
        verb.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counted_phase_degrades_with_what_libgit2_knows() {
        // The three shapes one callback walks through in order: nothing counted
        // yet, AddingObjects (total unknown), then a real denominator.
        assert_eq!(Phase::Packing { current: 0, total: 0 }.label(), "packing");
        assert_eq!(Phase::Packing { current: 3, total: 0 }.label(), "packing 3");
        assert_eq!(Phase::Sending { current: 3, total: 7 }.label(), "sending 3/7");
    }

    #[test]
    fn every_phase_has_a_short_line_that_starts_lowercase() {
        // The notice is one side-panel line, and the terminal ones it sits among
        // ("synced abc1234", "up to date") all open lowercase — a Capitalised or
        // over-long phase would read as the odd one out mid-sync. Only the
        // opening is checked: units keep their case ("1234 KB").
        for p in [
            Phase::JoiningWifi,
            Phase::SettingClock,
            Phase::VerifyingTls,
            Phase::ContactingOrigin,
            Phase::Packing { current: 1, total: 2 },
            Phase::Sending { current: 1, total: 2 },
            Phase::Downloading { current: 1, total: 2 },
            Phase::Retrying,
            Phase::Rebasing,
            Phase::WritingFiles,
            Phase::InstallingImage { kb: 1234 },
        ] {
            let line = p.label();
            assert!(!line.is_empty(), "a phase with no line would blank the notice");
            assert!(line.len() <= 24, "{line:?} is too long for the notice");
            assert!(
                line.starts_with(|c: char| c.is_lowercase()),
                "{line:?} should open lowercase like the terminal notices",
            );
        }
    }
}
