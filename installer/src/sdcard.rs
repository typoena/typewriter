//! SD-card provisioning: pick the card, clone the notes repo onto it, seed the
//! git-tracked prefs, write `typoena.conf`, and eject. Ports the safety
//! behaviours of the `just init`/`load` recipes; the repo copy is a fresh clone
//! from the remote (no rsync / .gitignore excludes / repack — see DESIGN.md).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// A candidate removable volume.
pub struct Card {
    pub volume: PathBuf,
    pub name: String,
    pub fs: String,
    pub fat: bool,
}

/// Read-only summary of an already-provisioned card, shown on the wipe-confirm
/// screen so the user sees exactly what they're about to erase.
pub struct CardInspect {
    pub origin: Option<String>,
    pub head: Option<String>,
    pub dirty: usize,
}

pub struct Plan {
    pub remote: String,
    pub pat: String,
    pub card_volume: PathBuf,
    pub conf_body: String,
    /// Erase an existing `repo/` + dirty journal before cloning.
    pub wipe: bool,
}

impl Plan {
    fn repo_dir(&self) -> PathBuf {
        self.card_volume.join("repo")
    }
    fn conf_path(&self) -> PathBuf {
        self.card_volume.join("typoena.conf")
    }
}

pub enum SdEvent {
    Log(String),
    Done(Result<(), String>),
}

/// Detect removable/SD volumes under /Volumes (via diskutil). Mirrors the
/// justfile `_card` heuristics; the internal boot disk never matches.
pub fn detect_cards() -> Vec<Card> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/Volumes") else {
        return out;
    };
    for entry in rd.flatten() {
        let vol = entry.path();
        if !vol.is_dir() {
            continue;
        }
        let info = match Command::new("diskutil").arg("info").arg(&vol).output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            _ => continue,
        };
        if !is_removable(&info) {
            continue;
        }
        let fs = field(&info, "File System Personality").unwrap_or_default();
        let up = fs.to_uppercase();
        out.push(Card {
            name: vol
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            fat: up.contains("FAT") || up.contains("MS-DOS"),
            fs,
            volume: vol,
        });
    }
    out
}

fn is_removable(info: &str) -> bool {
    // Test the VALUE of each key, not the whole line: the label "Removable
    // Media" itself contains "Removable", so a line-substring test matches every
    // disk — including the internal boot volume (found on real hardware, 07-14).
    let val = |k| field(info, k).unwrap_or_default();
    val("Protocol").contains("Secure Digital")
        || val("Removable Media").contains("Removable")
        || val("Ejectable") == "Yes"
        || val("Device Location") == "External"
}

fn field(info: &str, key: &str) -> Option<String> {
    info.lines().find_map(|l| {
        let rest = l.trim().strip_prefix(key)?.trim_start();
        let val = rest.strip_prefix(':')?.trim();
        (!val.is_empty()).then(|| val.to_string())
    })
}

/// True if the card already carries a working copy at `repo/`.
pub fn card_has_repo(vol: &Path) -> bool {
    vol.join("repo").exists()
}

/// Read-only inspection of an existing card (origin, HEAD, unpublished-edit count).
pub fn inspect_card(vol: &Path) -> CardInspect {
    let repo = vol.join("repo");
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!v.is_empty()).then_some(v)
    };
    CardInspect {
        origin: git(&["remote", "get-url", "origin"]),
        head: git(&["rev-parse", "--short", "HEAD"]),
        dirty: std::fs::read_to_string(vol.join(".typoena-dirty"))
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
    }
}

/// Run the full provision on a worker thread, streaming progress to `tx`.
pub fn run_provision(plan: Plan, tx: Sender<SdEvent>) {
    let mut log = |m: String| {
        let _ = tx.send(SdEvent::Log(m));
    };
    let res = provision(&plan, &mut log).map_err(|e| format!("{e:#}"));
    let _ = tx.send(SdEvent::Done(res));
}

fn provision(plan: &Plan, log: &mut dyn FnMut(String)) -> anyhow::Result<()> {
    if plan.wipe {
        wipe_card(&plan.card_volume, log)?;
    }
    clone(&plan.remote, &plan.repo_dir(), &plan.pat, log)?;
    log("seeding .typoena.toml (if absent)…".into());
    seed_prefs(&plan.repo_dir())?;
    log(format!("writing {}", plan.conf_path().display()));
    std::fs::write(plan.conf_path(), &plan.conf_body).context("writing typoena.conf")?;
    log("stripping AppleDouble ._ files…".into());
    dot_clean(&plan.card_volume);
    log("ejecting…".into());
    match eject(&plan.card_volume) {
        Ok(()) => log("card ejected — remove it and insert into Typoena.".into()),
        Err(e) => log(format!(
            "⚠ could not eject ({e}); eject from Finder before removing."
        )),
    }
    Ok(())
}

/// Erase an existing working copy before a re-provision. Only ever removes
/// `repo/` and the `.typoena-dirty` journal — never the volume itself, `ca.pem`,
/// or `/local`. The path guard rejects a bogus (root/empty) volume.
fn wipe_card(vol: &Path, log: &mut dyn FnMut(String)) -> anyhow::Result<()> {
    if !vol.is_dir() || vol.parent().is_none() {
        bail!(
            "refusing to wipe: '{}' is not a mounted volume",
            vol.display()
        );
    }
    let repo = vol.join("repo");
    log(format!("wiping {} …", repo.display()));
    if repo.exists() {
        std::fs::remove_dir_all(&repo).with_context(|| format!("removing {}", repo.display()))?;
    }
    let _ = std::fs::remove_file(vol.join(".typoena-dirty"));
    Ok(())
}

/// Clone `remote` into `dest` with the system git. The PAT (if any) rides in an
/// HTTP Authorization header, so it never lands in the cloned repo's origin URL
/// — origin stays the clean HTTPS URL the device authenticates against.
fn clone(remote: &str, dest: &Path, pat: &str, log: &mut dyn FnMut(String)) -> anyhow::Result<()> {
    if dest.exists() {
        bail!(
            "{} already exists — wipe the card first, or use a fresh one",
            dest.display()
        );
    }
    log(format!("cloning {remote} → {}", dest.display()));
    let mut cmd = Command::new("git");
    if !pat.is_empty() {
        let token = STANDARD.encode(format!("x-access-token:{pat}"));
        cmd.arg("-c")
            .arg(format!("http.extraHeader=Authorization: Basic {token}"));
    }
    cmd.arg("clone")
        .arg("--progress")
        .arg(remote)
        .arg(dest)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().context("spawning git (is it installed?)")?;
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            let line = line.trim_end();
            if !line.is_empty() {
                log(line.to_string());
            }
        }
    }
    let status = child.wait().context("waiting for git clone")?;
    if !status.success() {
        bail!("git clone failed (exit {:?})", status.code());
    }
    Ok(())
}

const PREFS_TEMPLATE: &str = "\
# Typoena editor preferences — hand-editable, git-tracked.
save_on_idle = true
format_on_save = true
line_numbers = true
theme = \"light\"
auto_sync = \"10m\"
";

/// Seed a starter `.typoena.toml` only if the cloned repo doesn't already carry
/// one (a repo with a synced prefs file keeps its own). Mirrors `_seed-configs`.
fn seed_prefs(repo_dir: &Path) -> anyhow::Result<()> {
    let p = repo_dir.join(".typoena.toml");
    if p.exists() {
        return Ok(());
    }
    std::fs::write(&p, PREFS_TEMPLATE).with_context(|| format!("seeding {}", p.display()))?;
    Ok(())
}

fn dot_clean(vol: &Path) {
    // Best-effort: strip the AppleDouble `._` companions macOS writes on FAT,
    // which otherwise corrupt the pack scan (`._pack-*.idx`). Failure never blocks.
    let _ = Command::new("dot_clean").arg("-m").arg(vol).status();
}

fn eject(vol: &Path) -> anyhow::Result<()> {
    let _ = Command::new("sync").status();
    let status = Command::new("diskutil")
        .arg("eject")
        .arg(vol)
        .status()
        .context("running diskutil eject")?;
    if !status.success() {
        bail!("diskutil eject exited {:?}", status.code());
    }
    Ok(())
}

/// Headless verification: (optionally wipe) + clone + seed + write a sample conf
/// into `dest`, with no card selection and no eject. Backs `--dry-run-sd`.
pub fn dry_run(remote: &str, dest: &Path, wipe: bool) -> anyhow::Result<()> {
    let plan = Plan {
        remote: remote.to_string(),
        pat: String::new(),
        card_volume: dest.to_path_buf(),
        conf_body: "# sample typoena.conf (dry run)\nTW_WIFI_SSID=example\n".to_string(),
        wipe,
    };
    let mut log = |m: String| println!("  {m}");
    if plan.wipe {
        wipe_card(&plan.card_volume, &mut log)?;
    }
    clone(&plan.remote, &plan.repo_dir(), &plan.pat, &mut log)?;
    log("seeding .typoena.toml (if absent)…".into());
    seed_prefs(&plan.repo_dir())?;
    log(format!("writing {}", plan.conf_path().display()));
    std::fs::write(plan.conf_path(), &plan.conf_body).context("writing typoena.conf")?;
    log("dry run complete (no card write, no eject).".into());
    Ok(())
}
