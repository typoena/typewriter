# Typoena

A single-purpose writing appliance: e-ink + mechanical keyboard + ESP32-S3. The
user opens the lid, writes Markdown, and (when they choose) pushes to a git
remote. This glossary fixes the language of that workflow, and of the screen
the writer looks at while doing it.

**Related docs:**
[`README.md`](README.md) — project overview, hardware, macro roadmap.
[`docs/adr.md`](docs/adr.md) — load-bearing decisions; **ADR-010** is the
formal record of the **Push** UX defined below.
[`docs/qfd.md`](docs/qfd.md) — requirements ↔ functions ↔ components, ranked
by user-facing weight. References the terms in this file as canonical.
[`docs/v0.1-mvp-product.md`](docs/v0.1-mvp-product.md) — the v0.1 product
surface, expressed in this vocabulary.
[`docs/v0.1-mvp-technical.md`](docs/v0.1-mvp-technical.md) — how v0.1 is
built.
[`docs/macroplan.md`](docs/macroplan.md) — per-version scope, where new terms
enter this glossary as versions land (the v0.5 multi-file **Buffer** terms
are in).

## Language

### File scopes

**Tracked**:
A file that lives in the device's git working copy and can be pushed to the
remote. Lives under `/sd/repo/`.
_Avoid_: synced, public, remote, committable.

**Local**:
A file that exists only on the device and can never be pushed. A
permanently-private scope, not a draft staging area — files are born Local and
stay Local for their lifetime. Lives under `/sd/local/`.
_Avoid_: draft, private, untracked, scratch (these all imply impermanence or
promotability, which is not the model).

### Editing model

**Buffer**:
A **File** loaded into memory for editing, with its own caret, scroll position,
and undo history. Opening a file makes it the **active buffer** — the one the
**Writing column** shows. Up to three buffers stay **resident** at once (the
active one plus two parked in the background); switching back to a resident
buffer restores its caret and undo without re-reading the card. A fourth open
**evicts** the least-recently-used resident buffer — saved first if it has
unsaved edits, so nothing is lost.
_Avoid_: tab, window, document (a buffer is not a UI chrome element); "the file"
when you mean the in-memory copy rather than the bytes on the card.

**Open**:
Bringing a **File** into the **active buffer**, via `Cmd-P` (the file
palette); new files are created with `:enew <file>` or the palette's
`> new file` (`:e` was retired in v0.6). Scope is read from where the file
lives (`/sd/repo` → **Tracked**, `/sd/local` → **Local**), never chosen at
open time.
_Avoid_: load (implementation talk for the disk read behind an Open).

### User-facing actions

**Save**:
The act of durably writing the current buffer to the SD card. Triggered by
`:w` (and by the idle auto-save when `save_on_idle` is on). Applies to both
**Tracked** and **Local** files.
_Avoid_: write, flush, persist (use them only in implementation talk).

**Push**:
The atomic act of pushing the current state of the entire **Tracked** working
copy to the git remote. Workspace-scoped, not buffer-scoped: a **Push**
ships every dirty **Tracked** file on the device, not just the one the user is
viewing. Triggered by `:gs`. Internally: splice the journaled dirty paths
onto HEAD's tree → commit with a timestamped message → push → on a rejected
push, fetch and replay the commit onto the remote tip (no merge) → push
again. Unavailable in **Local**.
_Avoid_: push, commit, sync, upload, git-push (these leak transport details
into user-facing language).

> **Commit** is deliberately _not_ a user-facing term. The device authors all
> commit messages itself (a timestamped message); the user never sees a commit
> prompt. A **Push** is the only user-observable unit of "shipping work";
> internal commits are an implementation detail of that.

### First run

**Onboarding**:
The one-time path from a device with no `/sd/typoena.conf` to a configured
device showing a writing cursor on the user's own notes repo. Two peer paths
produce the same card: the **Wizard** (on the device itself) and the
**Installer** (on a Mac). Both sign in through the same GitHub App device
flow and write the same two artifacts: `/sd/typoena.conf` and a cloned
`/sd/repo`.
_Avoid_: provisioning (the engineering function name in `docs/qfd.md`, not
user-facing language); setup (collides with `:setup`, the in-session reset
menu); first-boot flow (that names the trigger, not the outcome).

**Wizard**:
The on-device onboarding flow — Wi-Fi scan-pick, QR code sign-in with
GitHub, repo pick, clone — driven with only the device's keyboard, its
panel, and the user's phone. No computer involved. Triggered when
`/sd/typoena.conf` is absent; revisited later via `:setup`.
_Avoid_: captive portal, SoftAP (a deferred companion idea, not the wizard).

**Installer**:
The macOS one-command tool (`curl … | sh` from typoena.dev) that prepares an
SD card from a Mac: clones the notes repo onto the card, seeds defaults,
writes `typoena.conf`, ejects. It never flashes firmware — devices ship
pre-flashed.
_Avoid_: flasher, setup app, `install.sh` (that is the download script that
fetches the Installer, not the Installer itself).

### Screen regions

**Writing column**:
The left region of the panel showing the text being edited — the _only_ region
that repaints per keystroke. A 63-col region split into a **line-number gutter**
(absolute numbers, 2–4 cols wide, sized to the buffer's line count) and the text
column it steals from (~60 cols for a file ≤ 99 lines). Full panel height;
straddles the driver's `x = 396` seam invisibly.
_Avoid_: edit area, text area, main pane (superseded — they named the old
full-width text region before the side panel carved out its right edge).

**Side panel**:
The right region (~160 px / ~17 cols at its `FONT_9X15` metadata font, full
height) holding all metadata:
filename + dirty dot, word count, elapsed time, clock, Wi-Fi,
keyboard-disconnect flag, push state, and the mode indicator at its
bottom-left. Sits entirely in the master half
(right of the `x = 396` seam). Every field is static, event-driven, or
throttled — never per-keystroke.
_Avoid_: header, status line, status bar (retired — the old top header band and
bottom status band are both collapsed into this one right-hand region); sidebar.
Do not write bare **panel**: it collides with the **transient panel** (the
modal full-screen help/config view that swaps in over the editor — the palette,
the rest card, the `:about` splash). Always qualify: _side panel_ vs
_transient panel_.

### Refresh cycle

How the e-paper repaints. Three layers: the two driver **waveforms** (physics),
the render engine's **scope** choice for a partial, and the **triggers** that
schedule a full refresh. Terms below are one-to-one — each names exactly one
thing. The engine logs the chosen paint as one of `FULL` / `windowed` /
`windowed-fast` / `area`.

**Full refresh**:
The whole-panel Mode-1 flash (`0xF7`). Develops _every_ pixel and clears
accumulated ghosting; ~1–1.5 s. The only paint that launders the panel. Driver:
`display_frame` → `update_full`. Logged `FULL`.
_Avoid_: attaching "full" to any partial — see **area**, retired for exactly that.

**Partial refresh**:
The fast differential waveform (`0xFF`, ~0.5–0.65 s): only pixels that differ
from the on-screen image transition, so it leaves faint ghosting a later **full
refresh** clears. Never launders. Driver: `update_part`. Has two **scopes**:

- **Windowed**:
  A partial over only the rows that changed since the last frame — the
  per-keystroke typing path. Logged `windowed` (or `windowed-fast` with the
  experimental custom-LUT waveform, `Prefs::fast_partial`).

- **Area**:
  A partial over the whole panel height (all rows), for an edit that erases or
  moves ink — delete, scroll, mode switch, theme flip — where a **windowed**
  partial would leave ghost fragments of the vacated ink. Logged `area`.
  _Avoid_: **full-area** (retired 2026-07-25 — it carried "full" but is a
  _partial_, colliding with **full refresh**).

**Idle full refresh**:
A **full refresh** the render engine defers to a typing pause rather than firing
mid-keystroke (the flash must never land mid-sentence). Mechanism:
`Panel::longevity_full`. Fires on one of three **triggers**, named in the log as
`idle FULL refresh (<trigger>)`:

- **Boot-splash cleanup** — one-shot, launders the boot wordmark ghost at the
  first pause.
- **Longevity** — the periodic budget trigger: after `FULL_REFRESH_EVERY`
  partials, re-launder accumulated charge.
- **Deep-idle** — any accumulated ghosting after a genuine break
  (`DEEP_IDLE_MS`, 10 s).

_Avoid_: using **deep-idle** and **idle full refresh** interchangeably —
deep-idle is _one_ of the three triggers, not the path itself.
_Note_: `longevity` names both the budget **trigger** and (loosely) the code
method `longevity_full` that serves all three triggers; in the domain, `longevity`
is the trigger — the method name is an internal overlap, not a second meaning.

A **full refresh** can also be forced _outside_ the idle path — a card transition
(Rest / `:about`), a buffer switch past half the budget, or failed-paint
recovery. Those are not **idle full refreshes**; they ride a transition the user
already expects, so the flash is unsurprising there.

## Relationships

- A **File** belongs to exactly one scope (**Tracked** or **Local**), fixed at
  creation. There is no operation that moves a file between scopes.
- **Save** applies to any **File**; **Push** applies only to **Tracked**.
- A single **Push** is atomic from the user's view: a Wi-Fi failure or
  remote divergence surfaces as a single retry-able outcome, not as a multi-
  step progression the user has to reason about.

## Example dialogue

> **Dev:** "If I'm in a **Local** file and I run `:gs`, what happens?"
> **Domain expert:** "Nothing — **Push** is unavailable in **Local**. The
> side panel says so. There is no path from **Local** to the remote."
> **Dev:** "So if I want to push something that started as a journal entry,
> I have to copy-paste it into a **Tracked** file?"
> **Domain expert:** "Yes, deliberately. Promotion is a manual gesture, not a
> built-in operation."
> **Dev:** "And if the remote has changed since I last pulled — does
> **Push** fail?"
> **Domain expert:** "It fetches, replays the device's commit onto the remote
> tip — no merge commit — and pushes again. From the user's view it's one
> action with one outcome — success or retry."

## Principles

- **The device is a writing tool, not a sync engine.** Every git operation is
  the direct, in-session consequence of a `:gs` (or `:gl` pull) the user ran. The
  device does not auto-push, auto-pull, retry-on-boot, or otherwise
  reconcile remote state in the background. If a previous **Push** ended
  mid-flight and left a local commit unpushed, the next user-initiated
  **Push** picks it up; until then, the device is silent about it.
- **Push is sync, not history.** The user's mental model is a Google Doc
  that happens to be backed by git: the point is "I want to read this on my
  phone later," not "I want a curated commit log." Commits are a transport
  detail the device authors itself. Branches are out of scope for the same
  reason — the device tracks one linear stream of work on whichever branch
  the remote was cloned on, and never switches.
- **Durability before delivery.** A **Push**'s user-meaningful moment is
  when the local commit lands (a few seconds — the splice), not when the push
  completes (the rest of a ~12–24 s `:gs` on the real notes repo). The side
  panel surfaces the commit-landed state as soon as
  it exists; the remaining push time is the transport of an already-safe
  thing. Long-form rationale:
  [`docs/notes/ctrl-g-perceived-latency.md`](docs/notes/ctrl-g-perceived-latency.md).
- **No state the user didn't ask for.** No banners about pending work, no
  prompts about divergence, no "did you mean to push" warnings. The side
  panel reflects the _current_ action's outcome, nothing else.

## Flagged ambiguities

- "Local" was initially ambiguous between (a) a draft pen that gets promoted,
  (b) a permanently-private scope, (c) a second git repo, (d) `.gitignore`'d
  files inside the working copy. Resolved: (b). Each **File**'s scope is fixed
  at creation; there is no promotion operation.
- "Commit" was used loosely across early docs as if it were a user-facing
  action. Resolved: it is not. The user has **Save** and **Push**. Commits
  are an internal unit inside **Push**, never authored or named by the
  user.
