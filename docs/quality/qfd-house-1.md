# House 1 — WHATs × HOWs

_Next house → [House 2](qfd-house-2.md)_

The first house of the QFD cascade (hub + reading guide:
[`qfd.md`](qfd.md)): §1's WHATs (rows) × §2's HOWs (columns), each cell
scoring how strongly a characteristic advances a requirement
(9 / 3 / 1 / blank), with the §4 roof correlations, the basement with
v0.1 targets and Σ / relative weights, and the right-hand
competitive-perception zone (source of truth + rationale:
[`qfd-perception.md`](qfd-perception.md)).

> **Single source of truth.** The `\foreach` blocks in
> [`diagrams/house-1.tikz`](diagrams/house-1.tikz) restate §1's weights and
> §2's targets: TikZ can't read the tables, so keep them in sync when either
> changes, and **recompute the basement Σ / Rel % in the diagram source**
> (see [Regenerating](#regenerating)) rather than transcribing them from
> elsewhere. The diagram is the page's own figure precisely so the two
> can't silently drift.

![House 1 — WHATs x HOWs, with roof, basement and competitive-perception zone](diagrams/house-1.tikz)

---

## 1. Customer requirements (the WHATs)

What a user values about the device, with importance weights on a 1–10
scale, grouped by theme for reading (IDs keep their House row order; the
diagram and every matrix stay flat). Source columns point at the doc the
requirement comes from.

| ID  | Requirement                                                                        | Weight | Source                                                                                                                             |
| --- | ---------------------------------------------------------------------------------- | :----: | ---------------------------------------------------------------------------------------------------------------------------------- |
|     | **The writing loop**                                                               |        |                                                                                                                                    |
| W1  | Sub-second visible response to typing                                              |   10   | [product → Write](../plan/v0.1-mvp-product.md#user-stories), [README → UX](../../README.md#ux-boundaries-set-by-the-medium)                   |
| W16 | Any file, any action, any edit point is one motion away                            |   10   | [house-vs-product.md → D1](house-vs-product.md), [v0.5 → palette](../plan/macroplan.md#v05--file-palette--multi-file--x)                                  |
| W5  | Quick boot to a writing cursor                                                     |   6    | [product → acceptance](../plan/v0.1-mvp-product.md#acceptance-criteria) (≤ 5 s)                                                            |
| W7  | Nothing on the device competes with prose                                          |   8    | [README → vision](../../README.md#vision)                                                                                             |
| W8  | The UI never moves except when I move it                                           |   7    | [README → UX](../../README.md#ux-boundaries-set-by-the-medium)                                                                        |
| W13 | Typography sets a writing-tool tone: typewriter or developer editor, never gadget |   7    | [macroplan → v1.0](../plan/macroplan.md), [README → UX](../../README.md#ux-boundaries-set-by-the-medium)                                      |
|     | **Trust: words survive power and time**                                           |        |                                                                                                                                    |
| W3  | Pulling power never corrupts the file                                              |   10   | [product → Recover](../plan/v0.1-mvp-product.md#user-stories), [acceptance](../plan/v0.1-mvp-product.md#acceptance-criteria)                       |
| W6  | Long sessions without crash / lag / drift                                          |   9    | [product → acceptance](../plan/v0.1-mvp-product.md#acceptance-criteria) (1 h soak)                                                         |
|     | **Push & scopes**                                                               |        |                                                                                                                                    |
| W2  | **Pushing** is one deliberate action away                                       |   9    | [product → Push](../plan/v0.1-mvp-product.md#user-stories), [CONTEXT → Push](../../CONTEXT.md#user-facing-actions)                      |
| W12 | Local-only file scope coexists with git scope (v0.5+)                              |   5    | [README → scopes](../../README.md#vision), [macroplan → v0.5](../plan/macroplan.md#v05--file-palette--multi-file--x)                           |
|     | **Ownership & evolution**                                                          |        |                                                                                                                                    |
| W9  | Codebase absorbs the planned roadmap without rewrite                               |   8    | [macroplan](../plan/macroplan.md)                                                                                                          |
| W10 | I can repair or fork it with hobbyist tools                                        |   5    | [README → vision](../../README.md#vision)                                                                                             |
|     | **Away from the desk**                                                             |        |                                                                                                                                    |
| W11 | Multi-day battery life (v0.8 onward)                                               |   4    | [macroplan → v0.8](../plan/macroplan.md#v010--power-battery--sleep---)                                                                       |
| W14 | I can carry the device and write away from a desk                                  |   8    | [macroplan → v0.8](../plan/macroplan.md#v010--power-battery--sleep---), [README → hardware](../../README.md#hardware)                           |
|     | **First run**                                                                      |        |                                                                                                                                    |
| W4  | Provisioning never interrupts a writing session                                    |   7    | [product → Provisioning](../plan/v0.1-mvp-product.md#provisioning-build-time-dev-only), [macroplan → v0.9](../plan/macroplan.md#v09--robustness--) |
| W15 | A first-time user reaches writing without developer tools                          |   7    | [wizard](../plan/v0.9-onboarding-wizard.md) (zero-computer path), [installer](../../installer/DESIGN.md) (one-command Mac path)               |

### Who is voting — user segments

Two segments sit behind the weights, named so §3's single-rater caveat is
structural rather than a footnote:

| ID  | Segment                                                       | Weight (1–5) |
| --- | ------------------------------------------------------------- | :----------: |
| U1  | The author, daily writer, owns the toolchain and the roadmap |      5       |
| U2  | A first-time user without developer tools                     |      2       |

Every W-weight above is asserted from U1's chair; W15 is the one row that
exists *because of* U2 (U1 already has `just` and a pre-flashed device).
U2's weight of 2 is the product's reach bet, not an observed user; when a
real one appears, re-derive the W column as Σ(segment weight × strength,
normalised to 1–10) instead of asserting it.

### The WHAT that earned its row — flow (W16)

The 2026-07-17 challenge ([`house-vs-product.md`](house-vs-product.md) D1)
argued the product's center is **flow** (everything one motion away) and
that this table was structurally blind to it: the shipped editing grammar
(palette, vim modes, search) had no row voting for it, sheltering under
W7 at best. Resolved the same day by scoring the claim instead of
asserting it: **W16** is the reach *outcome* (a requirement, not a
solution), with **H17 reach cost** (§2) as its measurable characteristic.
Deliberately *not* added: a holistic "flow" row that would touch every HOW
weakly and add noise, and W1/W3 stay at 10 because they read as flow's
preconditions, not its rivals. The re-score's headline is in §3 below and [House 2](qfd-house-2.md): H1
moved to #2 and C7 (the widget/editor layer) to #2; the derived ranking
now agrees with where the July effort went, which was D1's
revealed-preference evidence all along.

---

## 2. Engineering characteristics (the HOWs)

Measurable attributes: performance metrics of the device's functions
(below), or properties of its firmware artifact, memory layout, and build
process. See [`glossary.md`](glossary.md) for the ontology layers
(WHAT / Function / Characteristic / Metric / Target). Targets are v0.1
unless noted. Direction column shows what "better" looks like
(↑ higher, ↓ lower, → fixed).

### Functions

The device performs these. The HOW rows below measure quality attributes
of these functions, or of artifacts they produce.

| Function  | Transformation                             |
| --------- | ------------------------------------------ |
| Type      | keypress → glyph rendered + buffer mutated |
| Navigate  | intent → active file / buffer / caret repositioned |
| Save      | dirty buffer → persisted file on SD        |
| Push   | persisted file → commit on remote          |
| Recover   | degraded file state → readable file        |
| Boot      | power-on → cursor ready                    |
| Provision | uninitialized device → configured device   |

**Provision** was build-time-only in v0.1 ([ADR-005], [ADR-007]); as of the
v0.9 wizard (slices 0–5a hardware-verified 2026-07-16,
[`v0.9-onboarding-wizard.md`](../plan/v0.9-onboarding-wizard.md)) it is a runtime
function with **two peer realisations**: the on-device wizard (keyboard +
panel + the user's phone, no computer) and the macOS installer
([`../installer/DESIGN.md`](../../installer/DESIGN.md)), which prepares the same
SD card from a Mac. Both write the same artifact (`/sd/typoena.conf` + a
cloned `/sd/repo`) and both authenticate through the Typoena GitHub App
device flow. Sub-functions referenced
inside HOW names: **Render** (buffer → e-ink frame, inside Type),
**Reconnect** (network outage → restored, inside Push).

### Characteristics

Rows are grouped by theme for reading (IDs keep their House column order;
the diagram and every matrix stay flat).

| ID  | Characteristic                                 | Dir | v0.1 target              | v1.0 target         |
| --- | ---------------------------------------------- | :-: | ------------------------ | ------------------- |
|     | **Render & input**                             |     |                          |                     |
| H1  | Type latency (keypress → glyph)                |  ↓  | ≤ 400 ms §               | ≤ 300 ms §          |
| H2  | Partial-refresh region area per keystroke      |  ↓  | ≤ 1 text line (~22 px h) | same                |
| H3  | Full-refresh cadence (clears ghosting)         |  →  | 1 per 64 partials        | tuned by panel temp |
|     | **Session: start, endure, never lose a word**  |     |                          |                     |
| H4  | Boot latency (cold)                            |  ↓  | ≤ 5 s                    | ≤ 3 s †             |
| H5  | Continuous-typing endurance (no drop, no leak) |  ↑  | ≥ 1 h                    | ≥ 8 h               |
| H8  | Save durability (post-confirm power loss)      |  →  | 100 %                    | 100 %               |
|     | **Reach: everything one motion away**          |     |                          |                     |
| H17 | Reach cost (keystrokes to any file / command / edit point) | ↓ | ≤ 6 median ⊳    | same                |
|     | **Push & network**                          |     |                          |                     |
| H6  | Push reliability (network up)               |  ↑  | ≥ 95 %                   | ≥ 99 %              |
| H7  | Push latency (one file)                     |  ↓  | ≤ 30 s ‡                 | ≤ 10 s ‡            |
| H9  | Heap headroom during Push                   |  ↑  | ≥ 1 MB PSRAM free at peak ¶ | same             |
| H12 | Network reconnect time (transient outage)      |  ↓  | ≤ 30 s                   | ≤ 10 s              |
|     | **Artifact & platform**                        |     |                          |                     |
| H10 | Firmware binary size                           |  ↓  | ≤ 2 MB                   | ≤ 1.5 MB            |
| H11 | Stack budget across all tasks                  |  ↓  | ≤ 128 KB (sum) ∥         | same                |
| H13 | Idle / typing / Push current draw           |  ↓  | measured only            | sized for >2 days   |
| H15 | Build time (clean, release)                    |  ↓  | ≤ 7 min                  | ≤ 5 min             |
|     | **First run**                                  |     |                          |                     |
| H16 | Onboarding duration (blank card → writing cursor) | ↓ | ≤ 10 min (v0.9, unmeasured) | same            |

† **Boot latency, measured 2026-07-11:** cold boot is **4258 ms**, so the ≤ 5 s
v0.1 target is met. The ≤ 3 s v1.0 target is assessed **marginal-to-unreachable**:
one ~1.9 s full refresh is unavoidable at cold boot (the `0x26` "previous" bank is
garbage until the first full paint), an e-ink floor rather than a tuning knob.
The 2026-07-14 boot restructure (async splash refresh, palette file-walk moved
to a background thread) held cursor-ready at ~4.2 s even with the full git
build and the 1100-file card walk; the walk now lands mid-session instead of
blocking boot. Breakdown + levers:
[`notes/boot-time-budget.md`](../record/notes/boot-time-budget.md).

‡ **Push latency, re-measured on the real repo 2026-07-13/14:** on the
author's real notes repo (~63 k objects), a cold `:gs` is **~24 s** and a warm
clean push **~19 s**, inside the ≤ 30 s v0.1 target, but the earlier toy-repo
figures (~16 s cold / ~10 s warm, 2026-07-11) turned out not to transfer:
push cost scales with repo shape. The mix also inverted: the push leg is
**5.9 s** (TLS session resumption); the splice-commit dominates at **10.3 s**
for one depth-4 file, and the convicted residual is **FAT linear directory
scans** (~0.1 ms/entry, `objects/` fan-out ≈ 256 dirs; see
[`tradeoff-curves/sync-commit-staging.md`](../record/tradeoff-curves/sync-commit-staging.md)).
The ≤ 10 s v1.0 target is **not met on deep paths** (root-level warm ≈ 12–13 s);
the lever is pack-not-loose object writes, deferred to a perf pass. History:
[`notes/sync-latency.md`](../record/notes/sync-latency.md),
[`kaizen/real-repo-sync.md`](../record/kaizen/real-repo-sync.md).

§ **Type latency: two tiers, re-read 2026-07-14; typing tier bench-corrected
2026-07-21.** The ~630 ms figure measured 2026-07-11 is the **full-area partial**
(deletes, caret moves, mode flips, the splash→editor swap), not the additive
typing path: per-keystroke typing rides the **windowed-Y partial** (~10 rows).
The floor+slope model projected **~100–130 ms** here, but the 2026-07-21
custom-LUT bench refuted it — refresh time is **area-independent** (waveform
BUSY, not the band-write), so the default windowed partial is **~495 ms** (misses
≤ 400 ms) and the custom `0x32` fast LUT (FR `0x08`, `fast_partial` opt-in)
reaches **~265 ms** (meets it), pending a longevity + cold soak before it can
default on
([`tradeoff-curves/epd-refresh-latency.md`](../record/tradeoff-curves/epd-refresh-latency.md)).
This also undercuts H2's latency framing, tracked as
[D2](house-vs-product.md#d2--refresh-area-is-not-a-latency-lever).
The v0.1 target history stands: relaxed from ≤ 200 ms to ≤ 400 ms (the original
was tighter than [ADR-003]'s own accepted "~200–300 ms" e-ink cost); v1.0 reset
from ≤ 150 ms to ≤ 300 ms. The open items are now the **erase/caret tier**
(~630 ms full-area partial per event) and graduating the fast LUT so additive
typing meets ≤ 400 ms on the **default** path, not just the opt-in one.

¶ **Heap during Push: measured and re-plumbed 2026-07-13.** The ≥ 1 MB
PSRAM bar is now **met** (min-ever 4.5 MB free on the first real-repo push,
run 9), but only after capping libgit2's mmap working set (mwindow
64 KB/1.5 MB, was 256 KB/4 MB; whole-file `.idx`/midx maps sit **outside** that
budget) and the odb cache at 1 MB. The learning that renamed this row: PSRAM
was the wrong sole pool to watch: **internal DRAM** is now the scarcer
resource (min-ever ~2.1 KB during TLS send; the mbedTLS
`EXTERNAL_MEM_ALLOC` move and interning the palette file list to one PSRAM
blob were both forced by internal-DRAM exhaustion). Watched via the
`log_push_heap` telemetry in `git_sync`.

∥ **Stack budget revised ≤ 80 KB to ≤ 128 KB (2026-07-16).** The old target was
priced for the pre-libgit2 five-thread guess (76 KB). Shipped reality: git
thread **96 KB** (libgit2 needs the headroom, [ADR-004] outcome) + USB pumps
4 + 8 KB + background file-walk 16 KB = **124 KB explicit**; UI/render run on
the main task and Wi-Fi is owned by the git thread, so there are no separate
ui/render/wifi stacks. Comfortable in 512 KB SRAM either way; the target
moved to follow the architecture rather than the reverse.

⊳ **Reach cost (H17), defined 2026-07-17: the W16/flow characteristic**
([`house-vs-product.md`](house-vs-product.md) D1). Keystrokes from intent
to target: any of ~1100 files = Cmd-P + 2-char query + Enter (**4**; MRU
recents under 2 chars), any command = the `>` / `:` grammar, any edit
point = modal motions with counts. The ≤ 6 bar is a session *median* and
is **unmeasured**; first measurement owed ([§6](qfd.md#6-critical-performance-budget)). H17 had a fight before it
had a name: the 35 s palette walk (2026-07-13, fixed to 4.3 s via dirent
file_type) was a reach-cost regression, invisible to the house as then
drawn.

---

## 3. House of Quality — WHATs × HOWs

This section reads the House (the diagram is at the top of this page): §1's
WHATs (rows) × §2's HOWs (columns), each cell scoring how strongly a
characteristic advances a requirement (9 / 3 / 1 / blank). The roof carries the §4 HOW-vs-HOW correlations; the basement carries the
v0.1 targets (from §2), the weighted-vote sums `Σ = Σ(W weight × cell strength)`,
and rounded relative weights. The right-hand zone scores five products against
the WHATs (0–5): the four competitors are **guessed, not measured**, while the
Typoena column is the **shipped, measured device** (rebased 2026-07-16; see
[Perception scores](qfd-perception.md#perception-scores-guessed)). The Σ totals quoted in the
priority list below come from the basement.

### Reading the house

- **Importance (left column)** is the raw 1–10 weight from §1, not a normalised
  %, so adding stays cheap when a WHAT shifts. Sum of weights is 120; treat each
  unit as ~0.83 % if you want a percentage view.
- **Roof** carries the §4 symbols translated into classical QFD glyphs:
  `++` strong reinforcement (`◎`), `+` mild reinforcement (`○`), `−` mild
  conflict (`×`), `−−` strong conflict (`⊗`).
- **Basement rows** are: v0.1 target → column sum (`Σ` of `weight × strength`) →
  relative weight as integer % of total (1804). Rounded relative weights sum
  to ~100 (99 with 16 columns' rounding).
- **H7, H10, H15** (Push latency, binary size, build time) sit at the bottom
  of the basement, knowingly-paid costs per [§7](qfd.md#7-tradeoffs-and-their-why-linked-to-adrs), not signals to optimise harder.

### Top engineering priorities (from importance)

1. **H9: heap during push** (193). libgit2 pack + rope + TLS all
   share the same arena; [ADR-001] and [ADR-004] trade binary size for ecosystem
   so this became the watched metric, and the watch paid off: the 2026-07-13
   real-repo push campaign found libgit2's mmap working set as the consumer,
   capped it (mwindow 64 KB/1.5 MB, odb 1 MB), and shifted the live worry to
   internal DRAM (see §2 ¶). The umbrella typography WHAT (W13)
   keeps a fixed-size glyph-cache load on top of that arena pressure.
2. **H1: Type latency** (178). The single most user-visible number;
   [ADR-002] and [ADR-003] are co-conspirators. #5 until the 2026-07-17
   W16 re-score: reach's medium vote (every one-motion jump is only as
   instant as its repaint) adds 30 and pushes it one point past H2.
3. **H2: partial-refresh region area** (177). Bound how many pixels the
   panel has to flip per keypress; [ADR-003] is the hardware-side answer.
4. **H12: network reconnect time** (160). Mobile use is the chief driver
   (W14 + W2 + W4 + W6, now joined by W15's clone leg); TLS session
   resumption (2026-07-14, second vendored delta in
   `esp_mbedtls_stream.c`) is the shipped answer: it cut the rejected-push
   reconcile fetch from ~30 s to ~5 s. Previously below the top six on a
   stationary v0.1 reading; W14 promotes it.
5. **H8: save durability** (156). Atomic-rename + fsync; FAT's weakness
   is acknowledged in [ADR-007] and mitigated, not designed around. H8's
   voter base spans W3 (power-loss correctness), W6 (long sessions),
   W12 (file scopes), and W14 (carrying = unclean shutdowns): the
   fourth voter is what lifts H8 into the top five by arithmetic alone.
6. **H3: full-refresh cadence** (144). The ghosting/flash tradeoff; lives
   in the render layer.

H17 (reach cost, 117) enters at **#9 on day one, above H5 endurance**:
a brand-new characteristic out-voting a soak target reads right for a
product whose center is flow, and is exactly the statement W16 was added
to make. Its two voters are W16 (strong) and W2 (medium: `:gs` is the
reach grammar applied to Push).

H13 (current draw, 137) sits at #7, close to the top-six cutoff because
W14 promotes the "wall-power for v0.1, measure first" stance from
acknowledged tradeoff to watched metric. The v0.1 "measured only" target
(§2) is still right; what changes is that bench multimeter readings (§6)
gain a second audience: sizing the v0.8 cell against a real portability
target, not just informing ADR-008's deferral.

H6 (Push reliability, 134) sits just below the top six. Its ADR ownership
is [ADR-004], whose spike-7 kill-switch **fired** (2026-07-06, gix has no
HTTPS push; the shipped transport is libgit2), plus [ADR-005] token auth.
The matrix simply reads W14's mobile-use voter as a louder signal for
reconnect (H12) than for the Push transport itself.

H16 (onboarding duration, 93) sits in the lower half rather than at the
bottom: W15 is its only strong voter, but W16's "the product itself is one
command away" adds a medium vote (63 to 93 at the 2026-07-17 re-score). The
house still correctly reads it as a first-run, once-per-device
characteristic: important to the product's reach (it is what
typoena.dev's "just power it on" sells), not to the daily writing loop.
Its budget row lives in [§6](qfd.md#6-critical-performance-budget) because it is still unmeasured.

**Why H8 ranks where it does.** Pre-W14, HoQ totals rewarded characteristics
that touch many WHATs over characteristics that absolutely matter for one WHAT.
W3 ("Pulling power never corrupts the file", weight 10) was H8's
strongest single voter, but H8 still sat at #6 because its base was
narrow. W14's "carrying = bumps = unclean shutdowns" widens H8's voter
base and lifts it into the top five by arithmetic — #5, not #3: H12's
160 outranks H8's 156. §6's "table-stakes correctness"
override is no longer the load-bearing argument for H8's prominence;
its acceptance-criteria override for H4/H5 still is. See [§6](qfd.md#6-critical-performance-budget).

The bottom three (H7 Push latency, H15 build time, H10 binary size) are real
costs but ones we knowingly took on ([ADR-001]) and are not in the critical
path of user experience. The tightened H15 v0.1 target (≤ 7 min) reflects
user preference for faster iteration, not matrix-derived priority; if it
pushes back against [ADR-001]'s "+5–10 min" pricing, the target moves
before the runtime decision does.


### Perception scores

The right-hand zone of the diagram: five products scored 0–5 against
the WHATs. The scores, per-cell rationale, measured benchmarks, and
caveats live in [`qfd-perception.md`](qfd-perception.md) — re-score
there first, then mirror the diagram above, same day.

### Regenerating

The matrix cells (`\node[qfdrel/{S,M,W}]`), roof symbols (`C-i-j` slots),
and basement Σ + Rel% live in
[`diagrams/house-1.tikz`](diagrams/house-1.tikz): re-score there, then update
the §3 priority list and §4 conflict list in the narrative above to match the
new picture.

When §1 or §2 changes:

1. Importance column → §1 weight column.
2. HOW titles + v0.1 targets → §2 target column.
3. Recompute basement Σ for any HOW whose column changed: per-cell
   contribution = `(W weight) × (cell strength: 9 / 3 / 1 / 0)`.
4. Recompute relative weight: each Σ ÷ total Σ × 100, rounded to integer
   percent.
5. Carry the change down: [House 2](qfd-house-2.md)'s component Σ row
   multiplies these basement Σ values by the HOW→component cells, so it
   moves whenever they do.

Perception scores are **not** derived from §1/§2: they live in
[`qfd-perception.md`](qfd-perception.md). Update them when (a) a competitor ships a relevant change,
(b) measurement replaces a guess, or (c) a WHAT is added/removed in §1.
Each score keeps its one-line rationale in the [perception table](qfd-perception.md).

The diagram source carries placement comments naming each WHAT, HOW and cell,
so it reads as text where a renderer is unavailable. The perception-scores
table in [`qfd-perception.md`](qfd-perception.md) is the readable form of the
right-hand zone.

---

## 4. Roof — HOW-vs-HOW tradeoffs

The roof shows where pushing one characteristic pushes another the wrong way.
ASCII glyphs (with classical QFD equivalents): **`++`** strong
reinforcement (`◎`), **`+`** mild reinforcement (`○`), **`−`** mild
conflict (`×`), **`−−`** strong conflict (`⊗`). The 16×16 roof matrix is
drawn on House 1 at the top of this page; the cells that actually shape the
design are called out below.

### Conflicts that actually shape the design

- **H1 latency ↔ H3 refresh cadence** (mild). More partial refreshes per
  second pile up ghosting faster, demanding earlier full refreshes:
  visible flashes that hurt H8 perception and H1 burst behaviour. The
  [ADR-003] (T3) strip aspect is the structural answer: a small framebuffer makes
  _both_ cheaper, not one at the expense of the other. The runtime answer
  is render §H3: schedule full refreshes at a typing pause (v0.1: idle ≥ 1 s;
  now a ≥ 2 s gate plus boot-cleanup / 30 s deep-idle / file-switch triggers that
  clear ghosting without ever flashing mid-typing — see
  [§6](qfd.md#6-critical-performance-budget) H3). The
  rows-vs-latency cost model behind this tradeoff (full / full-area-partial /
  windowed-Y) is in
  [`tradeoff-curves/epd-refresh-latency.md`](../record/tradeoff-curves/epd-refresh-latency.md).
- **H9 heap ↔ H10 binary size** (strong). std + libgit2 + mbedtls inflate
  both. We chose to spend on these ([ADR-001]/T1, [ADR-004]/T4) because 16 MB flash
  and 8 MB PSRAM make them affordable. Spike 7's kill-switch fired for a
  different reason than feared (gix had no HTTPS push, not a heap failure),
  and the heap fight then happened on libgit2's side: the 2026-07-13 push
  campaign traced full-PSRAM exhaustion to its mmap working set and settled
  it with hard caps (mwindow 64 KB/1.5 MB, odb cache 1 MB; §2 ¶).
- **H9 heap ↔ H5 soak** (strong). A long writing session grows the rope
  and the glyph cache; Pushing on top can OOM. Mitigations shipped: 256 KB
  file cap (v0.1 tech doc), the persistent two-frame draw in `main.rs`
  (repaints never allocate: added after a mid-push `HalfPageUp` OOM-aborted
  the UI thread, run 4), and the palette file list interned to one PSRAM
  blob (was 182 KB of internal DRAM).
- **H6 Push reliability ↔ H12 network reconnect** (reinforcing). Both come
  from the same network stack; the TLS session-resumption vendored delta
  (2026-07-14) proved the correlation: added for reconnect cost, it also
  removed the idle keep-alive push failure's sting (run 8's `SSL Generic
  error` during the 31 s marking gap; durable fix, reconnect-on-stale in
  the http layer, still open).
- **H12 reconnect ↔ H16 onboarding** (reinforcing). The wizard's clone leg
  rides the same TLS + Wi-Fi bring-up as Push; every second shaved off
  connect/reconnect shortens first-run too (ls-refs fast path, session
  resumption).
- **H10 binary ↔ H15 build time** (strong). std builds are slow. Accepted
  in [ADR-001] (T1): refactor leverage is the long-term payoff, not the
  per-build seconds.
- **H4 boot ↔ H10 binary** (mild). Larger binary = slower flash load.
  Affordable at our size class but worth watching as features land.
- **H11 stacks ↔ H13 current draw** (mild, future). Idle threads draw
  little but never zero; a future light-sleep policy (v0.8) wants them
  parked. W14's portability outcome raises the value of that policy
  from "battery hygiene" to "the thing that lets the device leave the
  desk."
- **W13 typography ↔ H9 heap + H10 binary** (mild, future). Achieving a
  writing-tool tone needs room for glyph caches and font assets. Not
  load-bearing in v0.1 (one mono font), but the v1.0 tone goal is why H9
  and H10 keep slack rather than being squeezed to the minimum.
- **Tightened H15 ↔ [ADR-001]** (mild). Pulling v0.1 build time from
  ≤ 10 min to ≤ 7 min eats into [ADR-001]'s accepted "+5–10 min" cost.
  Worth aiming at via cargo profile / vendor LTO / crate-graph trims;
  worth giving up before reversing [ADR-001].

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
