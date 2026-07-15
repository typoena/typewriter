//! Environment detection for the Preflight step.
//!
//! The device ships pre-flashed, so setup is SD-card-only: this checks what the
//! card prep needs — a mounted card and git. Everything here is advisory; a
//! warning informs the user, it never blocks. The real, destructive card work
//! (diskutil, rsync) lands in the SD-card step; this only reports what's there.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Missing,
}

pub struct Check {
    pub label: &'static str,
    pub status: Status,
    pub detail: String,
}

pub struct Preflight {
    pub checks: Vec<Check>,
}

impl Preflight {
    pub fn run() -> Self {
        Preflight {
            checks: vec![detect_sd_card(), detect_git()],
        }
    }

    /// Nothing is outright `Missing` (warnings are allowed to pass).
    pub fn ready(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Missing)
    }
}

fn detect_sd_card() -> Check {
    let label = "SD card";
    // Only report genuinely removable cards (via the SD step's diskutil-backed
    // detection). The Mac's own internal volumes — "Macintosh HD" and friends —
    // are deliberately never named here: surfacing the machine's own storage
    // reads as "this tool can touch my Mac's disk" and needlessly alarms people.
    let cards = crate::sdcard::detect_cards();
    if cards.is_empty() {
        return Check {
            label,
            status: Status::Warn,
            detail: "no card yet — insert your SD card (you'll pick it in the SD-card step)".into(),
        };
    }
    let names: Vec<String> = cards
        .iter()
        .map(|c| {
            if c.fat {
                c.name.clone()
            } else {
                format!("{} (not FAT32)", c.name)
            }
        })
        .collect();
    Check {
        label,
        status: Status::Ok,
        detail: format!("{} — chosen in the SD-card step", names.join(", ")),
    }
}

fn detect_git() -> Check {
    let label = "git";
    match std::process::Command::new("git").arg("--version").output() {
        Ok(o) if o.status.success() => Check {
            label,
            status: Status::Ok,
            detail: String::from_utf8_lossy(&o.stdout).trim().to_string(),
        },
        _ => Check {
            label,
            status: Status::Warn,
            detail: "not found — needed to clone your notes repo (xcode-select --install)".into(),
        },
    }
}
