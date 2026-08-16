# Typoena installer — design

A self-contained macOS CLI (ratatui TUI) that prepares an SD card so a
**pre-flashed** Typoena is ready to use the moment the card goes in. The public
entry point is the one-liner on typoena.dev:

    curl -fsSL https://typoena.dev/install.sh | sh

`install.sh` downloads this prebuilt binary; the binary does the rest. The user
needs **no repo checkout and no Rust toolchain** — just the card.

## Decisions

- **Self-contained end-user tool.** No `just`, no typewriter checkout. The
  binary bundles what it needs (config templates, snippet catalog). The proven
  `just` bash (`firmware/justfile`) is the *reference spec* for the safety
  behaviours, ported to Rust — not shelled out to.
- **The installer never flashes.** Devices ship **pre-flashed from
  manufacturing**; setup is SD-card-only. Firmware field updates
  (auto-update) are a **device/roadmap** concern, not the installer's — see
  [`docs/plan/macroplan.md`](../docs/plan/macroplan.md) (v1.x note).
- **The card's repo is a fresh `git clone` from the remote** (HTTPS + PAT),
  written straight onto the card. There is no local source clone to mirror, so
  none of the rsync machinery applies: **no `--ff-only` refresh, no `.gitignore`
  excludes, no repack** — a fresh clone already contains only tracked files and
  is a single pack, and its origin is already the HTTPS URL we cloned from.
- **Lives in `typewriter/installer/`**, tracked in the firmware repo so it
  versions in lockstep with the config templates + snippet catalog it ships.

## Phase pipeline (the wizard)

1. **Preflight** — a mounted card + git present. Advisory; warnings don't block.
2. **Configure** — collect Wi-Fi SSID/pass, GitHub user, GitHub token, git
   remote, and commit identity (the remote field sits **after** user + token so
   a bare repo name can complete against the now-known username). Pre-fill via
   the derive ladder (below); the token comes from **Sign in with GitHub** (^G,
   device flow) or a pasted PAT. The remote accepts shorthand — a bare `notes`
   (→ `https://github.com/<gh_user>/notes.git`), `you/notes`, `you/notes.git`,
   `github.com/you/notes`, and SSH pastes (`git@host:…`, `ssh://…`) all expand
   to the canonical HTTPS clone URL (`expand_remote_url_with_user`, live
   "→ will use …" hint under the field); the conf and the clone only ever see
   the expanded form, since the device's libgit2 is HTTPS-only.
3. **SD card** — pick the card (refuse on ambiguity), `git clone` the remote
   onto `/repo`, seed `.typoena.toml` + snippets if absent, write
   `typoena.conf`, strip `._*`, eject.
4. **Done** — "put the card in your Typoena and power on."

## Safety behaviours to keep (from `firmware/justfile` — do NOT regress)

- **Card-ambiguity refusal** — never guess when >1 removable volume; a wrong
  guess lets a write hit the wrong disk. Refuse and ask.
- **`.typoena-dirty` guard** — refuse to overwrite a card that carries
  unpushed device edits; offer backup-and-discard.
- **AppleDouble `._*`** — `dot_clean` before eject; `._pack-*.idx` corrupts the
  pack scan (Mac git *and* device libgit2).
- **Token never derived** — it comes from the ^G sign-in or is typed, never
  scraped from this Mac; plaintext on FAT means physical custody is the
  control. Device-flow tokens carry only the Typoena app's `contents:write`
  on repos the user granted; a pasted PAT should be fine-grained and scoped
  the same way. The clone uses the token so private notes repos work.

_Dropped vs. the old `just load` (now moot with clone-from-remote): rsync
mirror, `--ff-only` source refresh, `.gitignore` exclude list, `git repack -ad`._

## Config derive ladder (Configure step)

Each value: explicit input → derived from this Mac → prompt.
`author` ← `git config user.{name,email}` · `gh_user` ← `gh api user` ·
`ssid` ← live SSID if it can be read, else the top preferred network as a
**flagged guess** · `wifi_pass` ← Keychain (on ^K, may prompt macOS) ·
`remote` ← typed · `token` ← ^G Sign in with GitHub (device flow) or a typed
PAT (never derived).

**Wi-Fi SSID is best-effort, not authoritative.** On macOS 15+ (incl. Tahoe
26) `networksetup -getairportnetwork` reports "not associated" even when
connected, and `ipconfig`/`system_profiler` return `<redacted>` unless the
process holds Location Services permission — which a `curl | sh` binary won't.
So `config.rs` tries the real current SSID (dynamic Wi-Fi device via
`-listallhardwareports`, then `getairportnetwork`, then `ipconfig getsummary`),
and when all are blocked, falls back to the top of
`-listpreferredwirelessnetworks` as a guess. A guess sets `wifi_ssid_guessed`,
and the Configure step flags the field so the user confirms rather than trusts
it.

## Architecture / crates

- `ratatui` + its crossterm backend — TUI.
- `git2` — clone the remote onto the card, confirm origin. _[SD slice]_
- Config templates + snippet catalog embedded via `include_str!`
  (self-contained).

## Open items

- **Non-macOS** — Linux/Windows later; the work is macOS-first.
- **Clone target** — cloning ~hundreds of MB directly onto FAT via a reader;
  measure, and fall back to clone-to-temp-then-copy if it's too slow.
- **Re-provision** — the destructive case is covered: an existing card goes
  through an explicit **wipe-and-reclone** (`y`-confirmed screen showing origin +
  HEAD + unpushed-edit count; removes only `repo/` + the dirty journal, then
  clones fresh). Missing: a config-only rewrite that rotates the token or
  switches Wi-Fi *without* recloning, and backing up `.typoena-dirty` edits
  before wiping instead of only warning.

## The wizard, screen by screen

**App shell + Preflight.** Branded wizard with a `--check` headless mode; card
and git detection. Preflight reports only genuinely removable cards (via
`diskutil`) and never names `Macintosh HD` — a machine's own disk showing as
"available" alarms users.

**Configure.** Form + derive ladder, masked secrets, Keychain fill,
required-field validation.

**SD card.** Pick the card (boot disk excluded) → `git clone` onto it (single
pack, clean HTTPS origin) → seed `.typoena.toml` → write `typoena.conf` → strip
`._*` → eject. The clone runs on a worker thread streaming progress: `git clone
--progress` is split on `\r`/`\n` (line-buffered reading swallows the in-place
ticks) and parsed into a gauge, and the scrolling log keeps only the phase-final
`done.` lines.

**Navigation.** `Tab` / `Shift-Tab` move forward and back through fields *and*
steps, spilling at the ends; vim `h/j/k/l` on the non-form steps; arrows work
too. The sidebar shows `✓` done / `▸` current / dim pending, a `move` box, and a
live gate hint (`fill required` / `write card first` / `→ <next>`), so
"when and where can I go" is always visible.

**Brand header.** A single block caret types `typoena`, then the site's tagline,
paced against a wall-clock `Instant` rather than the render tick so the cadence
is frame-rate-independent. Solid while writing, blinks for 10 s once both lines
land, then settles.

## Sign in with GitHub (device flow)

`^G` on Configure asks GitHub for a one-time code (client_id
`Iv23liwgnE86ITDpBdnn`, the org-owned "Typoena" GitHub App — public by design,
no client secret in the device flow), shows it big, auto-opens
github.com/login/device, and polls in the background
(`authorization_pending` / `slow_down` honored, Esc cancels via an atomic flag)
until the `ghu_` token lands in the token field. The panel is modal on Configure
(plain Esc/n/q cancel; `^N`/`^P` are inert so a reflexive step-jump can't kill
the flow). HTTP is system `curl` — no HTTP crate; GitHub's OAuth endpoints
answer form-encoded, parsed by a tiny percent-decoding parser. `ghu_` tokens
speak the same `x-access-token:<token>` basic auth as a PAT, so the clone path
and the firmware are unchanged; a manual PAT paste remains the fallback. If the
app has token expiry on, the success message flags the token's lifetime.

**Authorization ≠ installation.** The token proves identity, but it only reaches
repos the app is *installed* on — a clone of an uninstalled repo 403s
(`Write access to repository not granted`), and the device flow never installs
anything, so every first-time `^G` user would hit it. Two guards:

- the token is probed against `GET /repos/{owner}/{repo}` the moment sign-in
  completes. No access → a yellow flag on Configure with the fix; `^O` opens the
  install page and a background watcher re-probes (5 s, ~2 min) so the flag
  turns green the moment the repo is granted. (`^I` would be the mnemonic, but
  Ctrl-I *is* Tab on a terminal.) Verdicts remember the remote they judged, so
  editing the field retires a stale flag. Advisory only: non-GitHub hosts and
  network failures stay silent.
- the SD step detects that 403's stderr signature and fails with a hint pointing
  at `github.com/apps/typoena/installations/new`, plus Enter-to-retry — no new
  sign-in needed, since token access is evaluated live.

## Release

Public GitHub releases on `typoena/typewriter`, one per `installer-v*` tag: a
universal macOS binary (lipo arm64+x86_64, stripped) with a `.sha256` sidecar.
`typoena.dev/install.sh` (in the [[typoena-site]] repo) is a Darwin guard → curl
of binary + sidecar from `releases/latest/download` → `shasum -c` verify →
`exec … </dev/tty`. Cutting a tag is the whole deploy.
