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
    let vols: Vec<String> = std::fs::read_dir("/Volumes")
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    // Slice-level detection only reports what's mounted; true removable/FAT
    // identification (diskutil) — and the ambiguity refusal — land with the
    // SD-card step.
    match vols.len() {
        0 => Check {
            label,
            status: Status::Missing,
            detail: "no volumes under /Volumes — insert a card".into(),
        },
        _ => Check {
            label,
            status: Status::Warn,
            detail: format!(
                "{} volume(s): {} — the card is chosen in the SD-card step",
                vols.len(),
                vols.join(", ")
            ),
        },
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
