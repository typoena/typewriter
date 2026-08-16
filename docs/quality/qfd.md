# Quality Function Deployment

Translates what the device must _be_ (user-facing requirements) into what it
must _achieve_ (engineering characteristics) and what we must _build_
(components), cascading through the four classical Houses of Quality:
requirements × characteristics, characteristics × components, components ×
processes, processes × controls. Surfaces the few targets that dominate the
design and the conflicts between them. Every decision cell points back to
[`adr.md`](../adr.md). Strength weights everywhere: **9** strong, **3** medium,
**1** weak, blank none.

Scope: the shipped device (v0.1 delivered 2026-07-11, v0.5–v0.7 delivered
2026-07-12/14, v0.9 onboarding in flight; see
[`v0.1-mvp-product.md`](../plan/v0.1-mvp-product.md),
[`v0.5-palette-and-multi-file.md`](../plan/macroplan.md#v05--file-palette--multi-file--x),
[`v0.7-search-and-git.md`](../plan/macroplan.md#v07--search--better-git--x),
[`v0.9-onboarding-wizard.md`](../plan/v0.9-onboarding-wizard.md)), **plus the
companion products** that now deliver the getting-started outcome: the macOS
installer ([`../installer/DESIGN.md`](../../installer/DESIGN.md)), the
typoena.dev site with its `install.sh` one-liner, and the Typoena GitHub App
(device-flow auth shared by installer and on-device wizard). The remaining
v0.8–v1.0 trajectory ([README](../../README.md), [macroplan](../plan/macroplan.md)) is
kept in mind so we don't paint into a corner. Terminology
(e.g. **Tracked**, **Local**, **Save**, **Push**) follows the project
glossary at [`../CONTEXT.md`](../../CONTEXT.md); the WHAT / Function /
Characteristic / Metric / Target ontology is defined in
[`glossary.md`](glossary.md).

## The pages

Section numbers (§1–§8) are global across the pages; each house diagram
lives on the same page as the tables it mirrors.

- [`qfd-house-1.md`](qfd-house-1.md) — **House 1, WHATs × HOWs**: §1
  requirements + segments, §2 characteristics + measured footnotes, §3
  reading + top priorities, §4 roof conflicts
- [`qfd-perception.md`](qfd-perception.md) — **competitive perception**:
  five products scored 0–5 per WHAT, measured benchmarks, caveats
- [`qfd-house-2.md`](qfd-house-2.md) — **House 2, HOWs × components**: §5
  cascade tree, component catalogue + derived ranking, shared-pool budget
  matrix
- [`qfd-houses-3-4.md`](qfd-houses-3-4.md) — **Houses 3 & 4** under the
  pipeline reading: processes P1–P9 × controls Q1–Q8
- [`house-vs-product.md`](house-vs-product.md) — standing challenges: when
  the houses and the builder disagree about what the product *is*, the
  dispute is argued there first, not silently re-scored

## What matters now (as of 2026-07-17)

**Top engineering priorities** ([§3](qfd-house-1.md#3-house-of-quality--whats--hows),
by basement Σ): H9 heap during Push (193) · H1 type latency (178) ·
H2 refresh area per keystroke (177) · H12 network reconnect (160) ·
H8 save durability (156).

**Component ranking** ([House 2](qfd-house-2.md), derived): C5 e-ink
panel #1 · C7 widget/editor layer #2 (the headline of the 2026-07-17
W16/flow re-score) · C12 libgit2 #3 · C2 std runtime #4.

**Open gaps** (detail and fallbacks in [§6](#6-critical-performance-budget)):

- **H1 erase/caret tier ~630 ms vs ≤ 400 ms** — still unmet. Additive
  typing was bench-clocked 2026-07-21: refresh time is area-*independent*
  (waveform BUSY, not the band-write), so the default windowed factory
  partial is ~495 ms and the earlier ~100–130 ms guess was wrong. The
  custom `0x32` fast-partial LUT (FR `0x08`, `fast_partial` opt-in) reaches
  ~265 ms (meets ≤ 400 ms), pending a longevity + cold soak before it can
  default on:
  [`tradeoff-curves/epd-refresh-latency.md`](../record/tradeoff-curves/epd-refresh-latency.md).
- **H8 power-pull test still owed** (v0.9 gate); dirty journal + boot
  recovery are shipped, the physical test is not run.
- **H17 reach cost and H16 onboarding duration are unmeasured** — the
  two budget rows that have never been clocked (≤ 6 keystrokes median;
  ≤ 10 min blank-card-to-cursor).
- **H7's v1.0 ≤ 10 s Push target is not honest on deep paths**
  (~12–13 s root-level warm): FAT loose-object residual, lever =
  pack-not-loose writes, deferred to a perf pass.

**Live tensions with triggers** ([§7](#conflicts-left-explicitly-unresolved-by-v01)):
keep-alive race (durable fix owed before v1.0 claims ≥ 99 %), token
plaintext at rest (the open [ADR-011]), onboarding reach (SoftAP
companion deferred), FAT rename window ([ADR-007]), typography paths
(v1.0 pass), battery ([ADR-008] — bench current numbers start v0.8 cell
sizing).

## How to keep these documents honest

- When a new ADR lands, add its components to [House 2](qfd-house-2.md)
  and re-score any characteristic row whose dominant component changed.
  **The same applies when an existing ADR gains an Outcome** (a
  kill-switch fires, a decision reverses): cascade it here the same day:
  these pages scored the dead gitoxide option for ten days after the
  swap.
- When a spike returns numbers, update [§6](#6-critical-performance-budget)'s "Target" or
  "Watched on" columns: §6 is the page that _should_ feel out of date if
  measured reality drifts from estimates.
- The companion surfaces (installer, typoena.dev, GitHub App, wizard) are
  in the house as W15 / H16 / C17–C20 but keep their design records in
  [`../installer/DESIGN.md`](../../installer/DESIGN.md) and
  [`v0.9-onboarding-wizard.md`](../plan/v0.9-onboarding-wizard.md); when those
  ship changes, re-check those rows rather than re-deriving them here.
- The WHATs (§1) change rarely; the HOWs (§2) change with each release.
  When either changes, re-score the matrix and recompute the basement Σ
  in the [House 1](qfd-house-1.md) diagram; then check §3's priority
  list and §4's conflict list still match the new picture — and update
  this hub's "What matters now" if the headlines moved.
- The [House 2](qfd-house-2.md) component Σ/Rank row is **derived**
  (basement Σ × cell strength): recompute it whenever the basement or a
  §5 cell changes, and keep unbuilt components (today C11, C15)
  parenthesised and out of the rank: scored fiction outranks real
  components, as the 2026-07-16 pass showed.
- The shared-pool budget matrix on [House 2](qfd-house-2.md) is the
  source of truth for the pool-mediated roof cells: when a component
  starts allocating from internal DRAM, PSRAM, or the DMA reserve (or a
  telemetry min-ever moves), update the table first, then draw (or
  retire) the roof cell it justifies. The roof was scored from the call
  graph once and missed three crashes; don't score it that way twice.
- Each house diagram mirrors the tables on its own page (House 1 the
  §1/§2 catalogues and [`qfd-perception.md`](qfd-perception.md)'s zone,
  House 2 the §5 matrix, Houses 3–4 the P/Q catalogues): re-score the
  table first, then the drawing, same day. **A diagram stays embedded in the
  page that owns its tables** — the pre-2026-07-11 split is how they drifted
  apart last time. The drawings live in
  [`diagrams/`](diagrams) as `.tikz` files, one per house, each embedded as
  an image. Their first 219 lines are a byte-identical preamble, so a style
  change must be pasted into all four.
- [Houses 3–4](qfd-houses-3-4.md) re-score when the *pipeline* changes
  shape: a new process step (CI, a second-platform installer,
  auto-update) or a new control (a test rig, release automation) gets a
  column and a fresh derivation the day it ships. Their cells are a
  2026-07-16 single-rater first cut; treat the
  P4-has-no-automated-control and Q6-is-the-only-install-path-control
  flags as live until answered.
- A [§6](#6-critical-performance-budget) row is not done when its target is met: the "If
  we miss it" cell must always name a live fallback, and a
  [§7](#7-tradeoffs-and-their-why-linked-to-adrs) tension must always carry a **Trigger to
  revisit**: otherwise it is a decision being avoided, not deferred.
- When the houses and the builder disagree about what the product *is*,
  the dispute goes to [`house-vs-product.md`](house-vs-product.md) first:
  argued with evidence and a trigger, not resolved by a same-day
  re-weight. Weights, rows, or cells change only after the entry there
  says why; and the next House-1 re-score must settle any OPEN entry
  that is waiting on it (none open today: D1/flow resolved 2026-07-17 by
  the W16/H17 re-score).

---

# 6. Critical performance budget

A curated rank, drawing from [§3 importance](qfd-house-1.md#3-house-of-quality--whats--hows) and [§4 conflicts](qfd-house-1.md#4-roof--how-vs-how-tradeoffs), with one
deliberate override: acceptance-criteria critical paths (H4 boot,
H5 soak) move up regardless of weighted-vote spread. (Pre-W14 this list
also lifted H8 durability over its narrow voter base; W14 has widened
that base, so H8's top-five spot is now arithmetic; see [§3](qfd-house-1.md#3-house-of-quality--whats--hows).) These started as
the numbers spikes 2–7 had to validate; most are now measured on the
shipped device. The Verdict column carries the result, and every row
names its fallback in "If we miss it": a target without a fallback is a
wish, not a budget. The fallback column also covers regression on
already-met targets.

| Rank | Characteristic         | Target                           | Watched on                          | Verdict                                                                                                                                                              | If we miss it                                                                                                                                                            |
| ---- | ---------------------- | -------------------------------- | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | H2 region area         | ≤ 1 line per keypress            | on-device refresh log               | ✓ windowed-Y drives only the touched line's band — but its *latency* framing is disputed (2026-07-21 bench: refresh time is area-independent), see [D2](house-vs-product.md#d2--refresh-area-is-not-a-latency-lever)                                                                                                                     | Larger font / coarser refresh region: the fallback that was never needed, kept named                                                                                    |
| 2    | H9 heap (Push)      | ≥ 1 MB PSRAM free at push peak   | `log_push_heap` telemetry           | ✓ run 9: min-ever 4.5 MB after mwindow 64 KB/1.5 MB + odb 1 MB caps; **new watch = internal DRAM** (min-ever ~2.1 KB during TLS send); [§2 ¶](qfd-house-1.md#2-engineering-characteristics-the-hows)                           | Re-tighten the mwindow/odb caps; move remaining internal-DRAM allocs to PSRAM (the `EXTERNAL_MEM_ALLOC` pattern); last resort = gate repo shape, as onboarding's 30 MB gate already does |
| 3    | H8 durability          | 100 % (post-confirm power loss)  | dirty journal + boot recovery       | Journal (`/sd/.typoena-dirty`) + `*.tmp` boot-recovery + stranded-commit replay shipped; the physical power-pull test is still owed (v0.9)                             | A failed pull test blocks v0.9 sign-off: fsync the directory handle after rename, then redesign the journal if that is not enough                                        |
| 4    | H1 Type latency        | ≤ 400 ms (revised from ≤ 200 ms) | refresh log; bench 2026-07-21 ([tradeoff](../record/tradeoff-curves/epd-refresh-latency.md)) | Bench killed the ~100–130 ms projection — refresh time is area-**independent** (waveform BUSY, not the band-write), so the default windowed factory partial is **~495 ms ✗**. Custom `0x32` LUT (FR `0x08`, `fast_partial` opt-in) → **~265 ms ✓**, pending longevity + cold soak before default-on; **erase/caret tier ~630 ms ✗**                                                                                                   | Windowed erase is dead as a lever (time is area-independent); the proven path is the same custom fast waveform on the erase/caret tier, or graduate `fast_partial` to default after its soak — else re-price [ADR-003] and move the target openly, never quietly                                     |
| 5    | H6 Push reliability | ≥ 95 % (network up)              | daily `:gs` use                     | Rejected-push → reconcile → replay → push cycle verified on device 2026-07-14; residual risk = stale keep-alive on long marking gaps (avoided via repack, not fixed)  | Reconnect-on-stale in the http layer: the named durable fix, owed before v1.0 claims ≥ 99 %                                                                             |
| 6    | H3 cadence             | full every ~64 partials          | `FULL_REFRESH_EVERY = 64`           | ✓ holding; every full is deferred to a typing pause (≥ 2 s, `CURSOR_DEBOUNCE_MS`) or hidden behind a file-load, so a flash never lands mid-typing (protects H1/H2). Longevity cadence is 64 partials (32 when `fast_partial`); three extra triggers now clear ghosting *below* that budget — one-shot boot-splash cleanup, a 30 s deep-idle launder, and a file-switch piggyback at ≥ half budget                                                                                                                             | If ghosting returns: lower `FULL_REFRESH_EVERY`, temperature-tune per panel                                                                                              |
| 7    | H4 Boot latency        | ≤ 5 s (cold, to cursor)          | 4258 ms 2026-07-11 ✓                | Held ~4.2 s through the 2026-07-14 restructure (async splash, background walk); [boot-time-budget](../record/notes/boot-time-budget.md)                                        | For v1.0's ≤ 3 s: memtest off (−0.74 s); the ~630 ms editor-bring-up **partial** is now a boot-path lever too — the custom `0x32` fast LUT ([tradeoff](../record/tradeoff-curves/epd-refresh-latency.md)) does full-area in ~300 ms at the bench, ~330 ms off boot if it validates cold; the ~1.9 s cold full refresh remains the e-ink floor the fast partial can't touch                                |
| 8    | H5 soak                | 1 h no leak / no drop            | 1 h real-use soak ✓ 2026-07-11      | Attested                                                                                                                                                              | Bisect the heap-touching change (the run-4 per-draw-alloc OOM was exactly this class) and re-soak before shipping it                                                     |
| 9    | H17 reach cost         | ≤ 6 keystrokes median (file / command / edit point) | **unmeasured**: count a real session | 4-keystroke file reach by construction (Cmd-P + 2-char query + Enter; MRU recents under 2 chars); the grammar is host-tested but a session median has never been counted | MRU depth + `PALETTE_MIN_QUERY` tuning, pinned files; if the *grammar itself* is what costs motions, that is a design question for [house-vs-product.md](house-vs-product.md), not a tuning knob |
| 10   | H16 onboarding         | ≤ 10 min (blank card → cursor)   | **unmeasured**: time a fresh run   | Wizard slices 0–5a verified on hardware but never wall-clocked                                                                                                        | Shallow-clone tuning, device-flow poll cadence; structurally, the deferred SoftAP companion (a phone keyboard beats the device keyboard for entry speed)                 |

The two not-in-MVP rows but already-shaped-by-design:

| — | H13 current | Measured only in v0.1 | bench multimeter | Cell sizing for v0.8 is data-driven, not spec-sheet; the Wi-Fi/auto-sync energy half is modelled in [wifi-auto-sync](../record/tradeoff-curves/wifi-auto-sync.md) | If measurements say > 2-day life is unreachable: revisit [ADR-008]'s cell class or W11's weight, on numbers, not hope |
| — | H11 stacks | Sum ≤ 128 KB (was ≤ 80 KB) | measured: 124 KB explicit (git 96 + walk 16 + USB 4+8) | Target followed the shipped architecture; [§2 ∥](qfd-house-1.md#2-engineering-characteristics-the-hows) | Re-price before adding any thread; if a new one breaks the sum, shrink or merge an existing stack first |

---

# 7. Tradeoffs and their why, linked to ADRs

Plain-language summary of what we accepted in exchange for what.
T-IDs are referenced from the [§5 cascade tree](qfd-house-2.md#the-cascade--what--function--how--components) and the tension list
below.

| ID  | Tradeoff                                        | Got                                                                                                  | Paid                                                                                                                                                  | ADR       |
| --- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| T1  | std (esp-idf-rs) over no_std (esp-hal)          | Heap, threads, VFS, mbedtls, room for a full git stack (proved out by libgit2)                       | +1 MB binary, +5–10 min builds                                                                                                                        | [ADR-001] |
| T2  | Custom widget layer over Ratatui                | Dirty-rects aligned to e-ink regions; 200 KB binary back                                             | 500 LoC we own and maintain                                                                                                                           | [ADR-002] |
| T3  | e-ink medium over FSTN / memory LCD / OLED      | Paper aesthetic; 0 W idle persistence; medium enforces writing posture                               | ~200–300 ms typing latency; periodic full-refresh flash (scroll worst-case)                                                                           | [ADR-003] |
| T4  | `libgit2` (`git2`) over `gitoxide`: the [ADR-004] kill-switch, fired 2026-07-06 | Working HTTPS push on-device; mature pack/transport code riding ESP-IDF's mbedTLS                    | FFI + a C build (esp-idf CMake component); two vendored C deltas to maintain (`esp_mbedtls_stream.c` double-free fix + TLS session resumption); an mmap profile that needed hard caps (mwindow, odb) | [ADR-004] |
| T5  | HTTPS + GitHub token over SSH                   | Simplest auth the device transport supports; App device-flow tokens (`ghu_`) ride the same header as a PAT, so wizard/installer sign-in changed nothing in the git path | Long-lived secret on device, now **plaintext in `/sd/typoena.conf`** (both provisioning paths write it; physical custody of the card is the control); encrypted-at-rest is the open [ADR-011]      | [ADR-005], [ADR-011] |
| T6  | `std::thread` over `embassy` or `tokio`         | Boring, debuggable, real stack traces; no exec to tune                                               | ~76 KB total stack across 5 tasks                                                                                                                     | [ADR-006] |
| T7  | FAT-on-SD + LittleFS-on-flash split             | Desktop can read SD; config survives SD reformat                                                     | Two filesystems to manage; FAT's power-loss weakness mitigated by atomic-rename                                                                       | [ADR-007] |
| T8  | Wall power for v0.1, battery deferred           | Measure real draw before sizing the cell                                                             | Tethered MVP; not the final aesthetic                                                                                                                 | [ADR-008] |
| T9  | USB host (TinyUSB) over BLE-HID                 | No radio contention with Wi-Fi during push; keyboard powered from the device                         | One more USB connector on enclosure                                                                                                                   | [ADR-009] |
| T10 | Atomic Push (`:gs`) + auto-timestamp commit message | One action, one outcome; matches the user's existing `gct` workflow; no modal prompt to slow H1 latency | Commit history is timestamp noise; the device authors replay commits the user never sees; reversal would break muscle memory                          | [ADR-010] |
| T11 | Splice commit over full index write             | Real-repo Push exists at all: ~19–24 s vs 611 s / OOM on the index path; dirty-path journal makes it power-pull-safe | Desktop-side edits to the card are never committed by the device; hand-edits on a computer must be pushed from that computer                         | [sync-commit-staging](../record/tradeoff-curves/sync-commit-staging.md) |
| T12 | Media stays in git, never on the card           | Killed `:gl`'s last OOM path; pull/apply touches text only; the repo stays whole for remote readers | Stale card media; phantom `git status` noise if the card is mounted on a computer; never hand-commit from the card                                   | (2026-07-14, `is_media_path`) |
| T13 | Shallow clone + ~30 MB repo gate at onboarding  | First-run clone fits device memory and minutes-scale patience                                        | Repos over the gate are refused at the repo-pick step (libgit2 has no partial clone, so tip media would download even if never written)               | [wizard](../plan/v0.9-onboarding-wizard.md) |
| T14 | Installer provisions the card, never flashes    | No USB-flash toolchain in the user path; devices ship pre-flashed; installer stays a small TUI      | Field firmware updates cannot lean on the installer: auto-update becomes a device-side problem (open, macroplan v1.x)                                | [installer/DESIGN.md](../../installer/DESIGN.md) |
| T15 | `curl … \| sh` one-liner over app-store/dmg     | Zero-friction start from typoena.dev; checksum-verified download; quarantine handled                | Pipe-to-shell trust ask; macOS-only today; the site and the GitHub release become launch-path infrastructure to keep up                               | (site repo `install.sh`) |

### Conflicts left explicitly unresolved by v0.1

These are the live tensions we are watching, not deciding harder. Each
carries the trigger that would force the decision: a tension without a
trigger is a decision being avoided, not deferred.

- **FAT loose-object cost vs H7's v1.0 target** (falls out of T11). The
  convicted residual of Push latency is FAT's linear directory scans
  (~0.4 s per loose write against the 256-dir `objects/` fan-out), bounded
  at ≤ ~2 s per commit and **accepted** for now; the lever is pack-not-loose
  writes. Until then the ≤ 10 s v1.0 H7 target is not honest for deep
  paths. **Trigger to revisit:** a v1.0 planning pass that keeps the ≤ 10 s
  target, or warm root-level `:gs` regressing past ~15 s.
- **Keep-alive race vs H6.** Run 8's push died on a connection idled out
  during a long marking gap; repack shrank the gap so run 9 succeeded:
  the race is *avoided*, not fixed. Durable fix = reconnect-on-stale in the
  http layer. **Trigger to revisit:** any recurrence of the run-8 signature
  (`SSL Generic error` mid-push), or before v1.0 claims ≥ 99 %.
- **Token at rest ([ADR-011], open, the Paid side of T5).** Both
  provisioning paths write the GitHub token plaintext to
  `/sd/typoena.conf`; physical custody of the card is the only control.
  Encrypted-at-rest (C15's eFuse key, C11) stays designed-but-unbuilt.
  **Trigger to revisit:** the device or card starts leaving the home, a
  second user's token lands on a card, or a token broader than the App's
  `contents:write` scope is ever provisioned.
- **Onboarding reach vs simplicity** (T13, T15). The wizard types Wi-Fi
  passwords on the device and the installer is macOS-only; the SoftAP
  companion webapp (phone-driven hand-off) was chosen over BLE 2026-07-16
  and **deferred**. **Trigger to revisit:** a real first-time user blocked
  by either path: no Mac for the installer, or defeated by on-device
  password entry.
- **[ADR-007] vs H8** (T7). Power loss between FAT rename and dir flush
  yields the previous saved version. We document this as expected behavior.
  **Trigger to revisit:** soak or power-pull testing showing it trigger on
  a routine save: then it is a bug, not a documented behavior.
- **W13 typography paths.** v0.1 ships one mono font; v1.0's
  writing-tool-tone outcome admits two paths (mono = developer comfort,
  serif = typewriter feel). Not yet decided whether to ship both or one.
  Cost preview per added font: +H9 glyph-cache footprint, +H10 binary for
  embedded assets. **Trigger to revisit:** the v1.0 design pass opening, or
  a serif asset being proposed for any earlier release, whichever first.
- **[ADR-008] vs W11+W14** (T8). Wall power in v0.1 is now an explicit
  disappointment of two WHATs, not one (battery W11 + portability W14).
  The disappointment is bounded by [ADR-008]'s commitment to measure
  current draw on real hardware before sizing v0.8's cell: spec the
  cell against measured numbers, not against the spec sheet. The [§3](qfd-house-1.md#3-house-of-quality--whats--hows)
  promotion of H13 (current draw) from #11 to #7 is the matrix
  registering this. **Trigger to revisit:** bench multimeter numbers
  landing (H13's "measured only" fulfilled): that starts v0.8 cell
  sizing.

[ADR-001]: adr.md#adr-001-language-and-runtime--rust-on-esp-idf-rs-std
[ADR-002]: adr.md#adr-002-ui-strategy--custom-widgets-on-embedded-graphics-not-ratatui
[ADR-003]: adr.md#adr-003-display-medium--e-ink-gdey0579t93-panel
[ADR-004]: adr.md#adr-004-git-implementation--gitoxide-gix
[ADR-005]: adr.md#adr-005-auth--https--github-personal-access-token
[ADR-006]: adr.md#adr-006-concurrency--stdthread--channels-no-async-runtime
[ADR-007]: adr.md#adr-007-storage-split--fat-on-sd-for-working-copy-littlefs-on-flash-for-config
[ADR-008]: adr.md#adr-008-mvp-power--wall-powered-battery-deferred-to-v08
[ADR-009]: adr.md#adr-009-keyboard-transport--usb-host-tinyusb
[ADR-010]: adr.md#adr-010-push-ux--atomic-ctrl-g-auto-timestamp-commit-message-no-user-prompt
[ADR-011]: adr.md#adr-011-credential-provisioning--how-the-pat-reaches-the-device-and-is-protected-at-rest
[ADR-012]: adr.md#adr-012-sd-on-its-own-spi3-host-not-shared-with-the-epd-on-spi2
