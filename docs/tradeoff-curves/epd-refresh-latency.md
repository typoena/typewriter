# E-ink refresh latency vs rows driven

> **Model (corrected 2026-07-16):** on this GDEY0579T93 (SSD1683 dual-controller)
> panel, refresh time is set by **which waveform LUT runs** — full-clear
> (~1870 ms) or partial (~540 ms floor) — plus a small SPI-transfer term that
> scales with the rows written (~0.34 ms/row at 4 MHz). Rows driven do **not**
> shorten the waveform itself: the gate-scan spike refuted MUX-proportional
> timing
> ([postmortem](../postmortems/2026-07-16-gate-scan-spike-refuted.md)), killing
> this doc's original `90 ms + 2 ms · rows` model. This is the cost model behind
> per-keystroke typing, the boot splash→editor swap
> ([`../notes/boot-time-budget.md`](../notes/boot-time-budget.md)), and the
> scroll/gutter spikes.
>
> Tradeoff-curves index: [`README.md`](README.md). Docs index:
> [`../README.md`](../README.md). Driver:
> [`../../firmware/src/epd.rs`](../../firmware/src/epd.rs). Bench origin:
> Spikes 5 + 8 ([`../spikes.md`](../spikes.md)); measured points from the
> 2026-07-16
> [bank-toggle](../postmortems/2026-07-16-partial-refresh-bank-toggle.md) and
> [gate-scan](../postmortems/2026-07-16-gate-scan-spike-refuted.md) sessions.

## The model

A refresh is three serial costs:

```
set RAM window  →  clock the pixels out over SPI  →  run the update waveform
   (fixed)          (scales with bytes = rows:         (fixed per LUT tier —
                     3 band writes at 4 MHz)            does NOT scale with rows)
```

- **The LUT sets the tier — and the floor.** A _partial_ update (`0x22`←`0xFF`)
  plays a short phase schedule that only nudges pixels that changed; a _full_
  update (`0x22`←`0xD7`, the fast-full LUT) plays ~3× as many frames to fully
  clear-and-set — slower, but it erases ghosting and re-establishes a known
  image. BUSY ends when the phase schedule ends, **however many gates were
  driven** — refresh duration lives in the LUT, not the scan.
- **Row count only trims the SPI transfer.** Each partial writes the band three
  times (`0x24` before the waveform, `0x26`+`0x24` resync after — see the
  [bank-toggle postmortem](../postmortems/2026-07-16-partial-refresh-bank-toggle.md)),
  so a full-area partial moves ~41 KB at 4 MHz (~86 ms) where a one-line band
  moves ~3 KB (~6 ms). That transfer is the *entire* difference between
  windowed and full-area.
- **Column width (X) is free to keep full.** The panel is a master (`0x00`) +
  slave (`0x80`) pair with the framebuffer split at the seam; every refresh
  drives _both_ controllers full width so the seam/mirror math stays intact
  (`update_part` / `write_frame_bank` in
  [`epd.rs`](../../firmware/src/epd.rs)).

Fitted through the two measured partial points:

```
t_partial(rows) ≈ 536 ms + 0.34 ms · rows
                  └ LUT waveform ┘  └ SPI: 3 band writes at 4 MHz ┘
```

Full refresh sits in its own flat tier (~1870 ms, full-clear LUT) and cannot be
windowed — the clear waveform needs the whole panel.

## The points

All measured on-device via the per-refresh log
(`{mode} refresh #N … {ms} ms` in [`main.rs`](../../firmware/src/main.rs)):

```mermaid
xychart-beta
    title "Partial refresh: measured (bars) vs refuted 90 + 2·rows model (line)"
    x-axis ["20 rows (windowed)", "272 rows (full-area)", "272 rows (boot 1st)"]
    y-axis "latency (ms)" 0 --> 700
    bar [543, 629, 680]
    line [130, 634, 634]
```

Bars are the measured partials; the line is what the old linear model
predicted. At 272 rows the two agree within 5 ms — that's where the model was
calibrated, which is how it survived until the first windowed bench. The
20-row point (543 ms measured vs ~130 ms predicted) is the refutation.

| Point                       | Rows |     Latency | LUT        | Source                                                                                       |
| --------------------------- | ---: | ----------: | ---------- | -------------------------------------------------------------------------------------------- |
| Windowed one-line band      |   20 |  **543 ms** | partial    | [bank-toggle session](../postmortems/2026-07-16-partial-refresh-bank-toggle.md)              |
| Full-area partial           |  272 |  **629 ms** | partial    | same session (630 ms in Spike 5)                                                             |
| Full-area partial, at boot  |  272 |  **680 ms** | partial    | boot log (splash→editor swap)                                                                |
| Full refresh                |  272 | **1870 ms** | full-clear | Spikes 5 + 8                                                                                 |
| ~~Spike: 20-gate MUX scan~~ |   20 |      571 ms | partial    | [gate-scan spike](../postmortems/2026-07-16-gate-scan-spike-refuted.md) — reverted, hazard   |
| ~~Spike: 272-gate trim~~    |  272 |      690 ms | partial    | same spike — even the "free" 272-vs-300 trim bought nothing                                  |

## Two things that bound it

**Ghosting caps the partial streak.** Partial updates leave faint residue, so a
full refresh every 64 updates resets clarity and panel state. You can't "always
partial" — the ~1870 ms tier is a periodic tax paid for longevity, not a mode you
can retire.

**The first cold-boot image must be a full refresh.** After power-on the `0x26`
"previous" bank holds garbage, and a partial refresh _diffs against it_ — so the
very first clean paint has to be the full tier. This is why boot pays exactly one
unavoidable ~1.9 s full refresh, and why the splash (which rides it) is nearly
free while the _editor's_ first frame can be a cheap partial on top. Full
derivation: [`../notes/boot-time-budget.md`](../notes/boot-time-budget.md).

## What it decides

- **Per-keystroke typing → ~545 ms per batch, whatever the window.** Additive
  Insert edits still take the windowed band (it's the cheapest mode and skips
  ~80 ms of SPI), but the flat waveform means typing feel is set by the LUT
  floor, not by how little changed. Type-ahead absorbs keystrokes during the
  refresh, so this is per *batch*, not per key.
- **The clean-erase policy is nearly free.** Deletes, caret moves, mode flips,
  and the snackbar take the full-area partial to avoid windowed erase ghosts —
  under the old model a ~500 ms penalty, under the real one ~86 ms of SPI.
- **Boot splash→editor → full-area partial (~680 ms), not a second full refresh
  (~1870 ms).** The splash already seeded the baseline, so the editor rides in
  on a partial — the ~1.2 s cold-boot win recorded in the boot-time budget.
- **Splash + periodic → full refresh.** The unavoidable first image and the
  every-64 de-ghost.

## Levers on the ~540 ms floor

Candidates, ranked by expected payoff against risk. Results tracked in the log
below — this table is the standing menu; the log records what each flash showed.

| Lever                                | Touches waveform? | Moves *perceived* per-stroke latency? | Risk                                                          | Status                                    |
| ------------------------------------ | ----------------- | ------------- | ------------------------------------------------------------ | ----------------------------------------- |
| Custom partial LUT via `0x32`        | **yes — authored** | **Yes — the only lever that does.** ~100–200 ms reported on similar panels | ghosting, DC balance/longevity, temperature, no reference waveform — [postmortem](../postmortems/2026-07-16-gate-scan-spike-refuted.md) | parked ("touching the LUT")               |
| SPI clock 4 → 20 MHz                 | no                | full-area only — measured **−122 ms** (~693→~571 ms, now ≈ windowed); typing flat | signal integrity — 20 MHz clean in test but at panel ceiling on jumpers | **shipped 2026-07-17** — see log            |
| ~~Async partial + deferred bank resync~~ | no — pure firmware | no — frees the editor loop during BUSY, not the eye | bank-toggle ordering (this panel is treacherous — [postmortem](../postmortems/2026-07-16-partial-refresh-bank-toggle.md)) | **closed 2026-07-17** — not worth it post-20 MHz (see below) |
| ~~Temperature-select (`0x1A` sweep)~~ | no | no — flat at every value | — | **closed 2026-07-17** — not a lever (see log) |
| ~~Gate-scan restriction (`0x01`/`0x0F`)~~ | no | — | **refuted + hazard**: MUX-independent timing, mirrors the panel (OTP gate config, write-only) | closed — never write these registers      |

### Async partial — CLOSED 2026-07-17, not worth building

Pitched as a responsiveness win; working the timing through, it isn't — writing
down why so we don't chase it a third time.

Per-stroke *perceived* latency is `write 0x24 band` + `BUSY waveform` — the point
where the ink has physically formed. The `0x26` + `0x24` resync writes async
would defer run **after the ink is already on the panel**, so they never added to
what the user sees — only to how long the editor loop is held. Async frees the
*loop*, not the *eye*.

Three things closed it, the first two decisively after the SPI bump:

1. **The deferred work is now nearly free.** The resync was ~86 ms full-area at
   4 MHz — worth chasing. At 20 MHz the *entire* full-area SPI excess is ~6 ms,
   so there's almost nothing left to move off the path.
2. **The overlap benefit is already handled.** Freeing the loop during BUSY only
   pays if other work can overlap it. Git push (`:gp`) is already on its own
   96 KB thread (non-blocking); SD saves are inline but run in the effect-drain
   step *before* the refresh, not inside the BUSY wait, and idle-saves fire only
   on a typing pause. So the loop rarely has blocking work to hide behind BUSY.
3. **The risk is real.** Async lives in the bank ping-pong that already caused
   the [bank-toggle flapping bug](../postmortems/2026-07-16-partial-refresh-bank-toggle.md)
   — an intermittent, speed-dependent failure class. Poor trade for a few ms.

No observed background-sync stutter to justify it (the decision gate). If that
symptom ever appears, revisit — build it toggleable with a persistent resync
buffer and the resync-before-next-`0x24` invariant enforced in `wait_ready`.

**What's left after this pass:** the only lever on the ~543 ms itself is the
custom `0x32` LUT — parked for its real costs (longevity, ghosting, no reference
waveform), the *sole* per-stroke lever, revisit only if typing latency becomes a
top user complaint. Everything else (temperature, gate-scan, async, SPI beyond
20 MHz) is closed.

### Experiment log — temperature-select sweep

**What it tests.** The partial's `0x22 ← 0xFF` reloads temperature + LUT from the
`0x1A` register on every refresh. `init()` leaves that register at `[0x64, 0x00]`
(~100), so the 543 ms baseline *already* runs at temp 100 — the spike sweeps the
register above and below that to find out whether the partial OTP LUT's schedule
is temperature-indexed the way the fast-full LUT is. Higher = faster would open
the lever; flat across the sweep proves the floor is fixed and closes it.

**How to run.** Set `PARTIAL_TEMP` in [`epd.rs`](../../firmware/src/epd.rs),
flash, type, read `windowed refresh #N … {ms} ms` from the serial log. Note
ghosting over a full ~64-partial streak (shorter drive shadows sooner), not just
the first refresh. `[0x64, 0x00]` reproduces the baseline as a control.

| `0x1A` value  | Windowed (20-row) ms | Full-area (272) ms | Ghosting over 64-streak | Verdict | Date |
| ------------- | -------------------: | -----------------: | ----------------------- | ------- | ---- |
| `[0x64,0x00]` (baseline, init default) | 543 | 629 | clean (current shipping) | control | 2026-07-16 |
| `[0x7F,0x00]` (hotter) | 562–571 | 693 | not evaluated (no gain to justify) | **no gain** — flat vs baseline | 2026-07-17 |
| `[0x19,0x00]` (cold ~25) | 562–572 | 689–698 | not evaluated | **no gain** — flat vs baseline | 2026-07-17 |

**Verdict: CLOSED — temperature is not a lever.** Hot, cold, and default all land
at ~565 ms windowed / ~690 ms full-area. The partial waveform's BUSY time is
temperature-independent on this panel: either `0x18 ← 0x80` (internal sensor)
overrides the `0x1A` register during load-temperature, or the OTP partial LUT
carries a single fixed phase schedule. Restored `PARTIAL_TEMP = None`.

The deeper takeaway: **the ~543 ms is ink-formation physics set by the partial
LUT, and nothing that selects *among* factory LUTs can move it.** Only authoring
a shorter waveform (`0x32`) touches this number — see the reassessment below.

### Experiment log — SPI clock sweep

**What it tests.** The EPD bus clock (`SpiBusDriver` baudrate in
[`main.rs`](../../firmware/src/main.rs)) sets only the pixel clock-out rate, not
the waveform BUSY time. Raising it trims the pre-kick band write and the resync
writes — a perceived-latency term on the full-area path (~43 ms of write at
4 MHz), a small one on the windowed path (~6 ms). The risk is signal integrity
on the panel wiring at higher clocks: watch for garbled or missing bands.

**How to run.** Set the baudrate, flash, type, read `full-area refresh #N … ms`
(the full-area path shows the largest SPI term) and eyeball the panel for
glitches. Expected full-area floor if SPI vanished entirely ≈ 543 ms waveform +
minimal write.

| EPD SPI clock | Windowed (20-row) ms | Full-area (272) ms | Panel integrity | Verdict | Date |
| ------------- | -------------------: | -----------------: | --------------- | ------- | ---- |
| 4 MHz (canonical baseline) | 543 | 629 | clean | control | 2026-07-16 |
| 4 MHz (same-session ref) | ~565 | ~693 | clean | apples-to-apples ref for the sweep | 2026-07-17 |
| 10 MHz | ~563 | 623–628 | clean | −68 ms full-area vs same-session 4 MHz; windowed flat | 2026-07-17 |
| **20 MHz** | ~565 | 569–574 | clean (short test) | **kept — full-area now ≈ windowed**; −122 ms full-area vs same-session 4 MHz, beat the ~20 ms estimate | 2026-07-17 |

```mermaid
xychart-beta
    title "Refresh latency vs EPD SPI clock — full-area converges on the waveform floor"
    x-axis ["4 MHz", "10 MHz", "20 MHz"]
    y-axis "latency (ms)" 520 --> 720
    line [693, 625, 571]
    line [565, 563, 565]
```

Upper (descending) line = full-area partial (272 rows); lower flat line ≈ 565 ms
= windowed one-line partial (the typing path). Both sit just above the ~543 ms
canonical waveform floor. The curve is the whole story: raising the clock only
drains the SPI term, so the full-area line falls toward the windowed line and
stops — past 20 MHz there's nothing left to drain. **Read the shape, not the
slope:** the x-axis is categorical (mermaid limitation), so 4/10/20 MHz are drawn
evenly spaced though the real gaps are 2.5× then 2×; the diminishing return is
even sharper than the picture suggests.

**Result: 20 MHz kept.** Full-area collapsed to ~571 ms — within ~6 ms of the
windowed typing path (~565 ms), so a full-panel repaint is now essentially
*waveform-bound*: the SPI cost of driving all 272 rows instead of 20 has all but
disappeared. The 10→20 MHz step beat the linear-SPI prediction (~54 ms vs the
~20 ms estimated) — the fixed-delay floor was a smaller share of the residual
than assumed. Panel clean through the test; **caveat: 20 MHz is at the SSD1683
ceiling on jumper wiring**, so intermittent corruption could surface in longer
use. Low blast radius if it does — the RAM buffer is source of truth and a bad
paint self-heals on the next refresh (the `force_full` recovery path) — but if
any ghosting/garbling appears, drop to 10 MHz, which is safely in-spec and still
holds most of the win. **Stop here:** only ~6 ms of SPI excess is left, and
above 20 MHz exceeds panel spec for no meaningful gain.

**Result: 10 MHz kept.** Full-area (erase/caret/scroll/mode-switch) dropped
~693 → ~625 ms, windowed typing stayed ~563 ms — the clock moves the SPI term
and nothing else, exactly as modelled. Panel clean, no glitches.

> **Session variance, read the same-session rows.** The canonical 4 MHz numbers
> (543/629, 2026-07-16) run ~20 ms windowed / ~60 ms full-area faster than the
> 2026-07-17 4 MHz measurements of the *same* firmware — panel warmup / ambient
> drift between sessions. So the SPI win is measured against the same-session
> 4 MHz ref (~693 full-area), not the canonical baseline. Cross-session
> absolute-ms comparisons in this doc carry ±~10 %.

At the shipped 20 MHz the full-area path is ~571 ms ≈ the windowed ~565 ms, so
the SPI cost of a full-panel repaint is spent — what remains (~28 ms over the
~543 ms canonical waveform) is the fixed overhead both paths share (the
`FreeRtos::delay_ms(2)` calls in `set_ram_area`, command framing) plus
session-drift. The delay floor turned out to be a *smaller* share than the
10 MHz residual suggested — the 10→20 MHz step cut full-area SPI excess from
~62 ms to ~6 ms, well past linear. Those fixed `set_ram_area` delays (~12–16 ms
per full-area refresh) are the only remaining non-waveform lever, and they'd
help *both* paths equally — a separate micro-optimization if ever worth it.

**Power is not a factor in the clock choice.** Dynamic power scales with
frequency (`P ≈ C·V²·f`) but a faster clock finishes in proportionally less
time, so the *energy* to shift the same pixel bytes is ~constant across clock —
higher SPI ≠ more drain. And SPI is a rounding error next to a refresh's real
energy cost, the panel DC-DC pump driving ±15 V through the ~543 ms waveform,
which is set by the LUT and wholly independent of SPI clock. The energy levers
are *fewer/shorter refreshes* (custom LUT frames, lower full-refresh cadence),
not the bus rate; device-level draw is dominated by Wi-Fi/TLS during `:gp` and
the CPU/PSRAM regardless.

