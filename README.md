# Typoena

A distraction-free, hackable, DIY writing machine. ESP32-S3 + e-ink + a real
mechanical keyboard. You write Markdown, you commit, you push. Nothing else
runs on it.

> **Status: v0.7 shipped, hardware on bench.** v0.1 (MVP — boots, edits,
> pushes) shipped 2026-07-11; v0.2 through v0.7 (vim navigation and editing,
> file palette + multi-buffer, Markdown affordances, `/` search, `:gp` push /
> `:gl` pull) followed within the week. Live per-item status: the Macroplan,
> first link below.

Design & context:

- [Macroplan](docs/macroplan.md) — the week-by-week delivery plan, live
  per-item status, and per-version scope
- [QFD](docs/qfd.md) — how every decision is weighted against the
  user-facing requirements (Goal → Function → How → Component)
- [ADRs](docs/adr.md) — the decision log, rejected alternatives included
- [Doc index](docs/README.md) — everything else: per-version specs, notes,
  tradeoff curves
- [Bill of materials](docs/bom.md) — build it yourself, full part list
- [Hardware notes](docs/hardware.md) — part rationale and bench status
- [Postmortems](docs/postmortems/README.md) — failure write-ups
- [Kaizen](docs/kaizen/README.md) — improvement loops

Technical pages, one per subsystem:

- [Firmware](firmware/README.md) — build, flash, provisioning, editor setup,
  bring-up spike log
- [Enclosure](hardware/case/README.md) — parametric, 3D-printable
  typewriter-body case (OpenSCAD)
- [Installer](installer/DESIGN.md) — macOS setup tool: flash + provision
  without a dev environment

---

## Vision

A single-purpose appliance that boots into a text editor with a Vim keymap,
edits Markdown files, and (optionally) pushes them to a git remote (GitHub
first) over Wi-Fi. No browser, no notifications, no apps. Open lid → write →
push (or don't) → close lid.

Two file scopes coexist on the SD card — formal definitions in
[`CONTEXT.md`](CONTEXT.md):

- **Tracked** — lives in the git working copy, gets **Pushed** when the
  user runs `:gp`.
- **Local** — never leaves the device. Permanently-private: journal entries,
  scratch, things that aren't anyone else's business. There is no "promote
  to Tracked" gesture — scope is fixed at file creation.

Same editor, same keymap; the difference is just whether `:gp` (push to
the remote) is offered.

---

## Hardware

**ESP32-S3-N16R8** (16 MB flash, 8 MB PSRAM) · **GDEY0579T93** 5.79″ e-ink
strip (792×272, ~2.9:1 — biases the UX toward "current line + recent context",
the writing posture we want) · **Nuphy wired USB keyboard** with the S3 as USB
host · **microSD over SPI** · **USB-C wall power** for the MVP, battery in
v0.8.

---

## Software stack

**Rust on `esp-idf-rs` (std)**, a custom modal editor and thin widget layer on
`embedded-graphics`, a custom dual-SSD1683 e-ink driver, libgit2 (via `git2`)
for `:gp`/`:gl` sync over mbedTLS. The layer-by-layer table — each choice with
its ADR, measured costs, and the annotated repo layout — is in
[`docs/stack.md`](docs/stack.md).

---

## UX boundaries set by the medium

E-ink is a brutal honesty filter on UI choices. Hard constraints we design
around, not against:

- **No cursor blink.** Kills the panel and the battery.
- **Typing latency target: ≤ 200 ms** from keypress to glyph on screen, using
  partial refresh on the affected line only.
- **Full refresh every ~20 partials** to clear ghosting. User-visible flash —
  schedule it on pauses (>1 s of no input).
- **No smooth scrolling.** Page-style jumps only.
- **No animations.** Anywhere.
- **Render only changed lines**, not the viewport.

---

## Open questions / risks (tracked, not yet resolved)

- [ ] Heap fragmentation over a long writing session with the PSRAM allocator.
- [ ] Real-world e-ink ghosting with the current partial-refresh cadence.
- [~] Use-after-free freeing the in-flight USB transfer on keyboard unplug —
  fixed in code, pending an on-device hot-plug run to confirm
  ([`MEMORY_AUDIT.md`](MEMORY_AUDIT.md) finding #1).

Retired risks ([gix push](docs/postmortems/2026-07-05-spike7-gix-https-push.md),
[SD CMD59 rejection](docs/postmortems/2026-07-05-spike3-sd-cmd59.md), TinyUSB HID
stability, TLS heap, libgit2-on-xtensa) and how they died:
[`docs/spikes.md`](docs/spikes.md) and
[`docs/postmortems/`](docs/postmortems/README.md).

These get resolved by writing code, not by deciding harder.
