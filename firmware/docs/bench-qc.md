# Bench QC firmware — spec

A bring-up / go-no-go fixture firmware for the hand-soldered Typoena carrier PCB.
It exercises every connection the ESP32-S3 can reach and reports **OK / NOK** per
subsystem, so a freshly-assembled board is validated (or its bad joints located)
in one flash.

Written for the PCB migration: a **DevKitC-1 (WROOM-1 N16R8) mounted on a carrier
PCB**, with EPD, microSD, a **USB-C host port** for the keyboard, and a **LiPo +
charge IC** hand-soldered around it.

## What this is / is NOT

A firmware self-test only senses what the S3 can drive or read back. That is a
strong signal for **functional, per-subsystem OK/NOK** — a shorted SCK or a cold
CS joint makes the SD refuse to mount and the EPD BUSY handshake hang; a swapped
MOSI/MISO kills the round-trip. Those are unambiguous.

It is **not** an ICT / bed-of-nails replacement:

- No rail-voltage measurement (no ADC divider is planned on 5V/3V3) → **rails stay
  a 30-second multimeter check.** The cell is the exception: the charger's own ADC
  reports VBAT, SYS and VBUS over I2C, so check #7 reads real voltages.
- Opens on a truly floating spare pin are only inferable via its internal pull.

## :alert-triangle: Hardware prerequisites (validate before trusting two of the tests)

These are wiring facts the firmware cannot compensate for. Confirm them at the
board, ideally with a multimeter _before first power-on_.

1. **USB-C host (keyboard) — sourced VBUS is the hard requirement, not CC Rp.** The
   S3 OTG exposes only **D+/D−**; it has **no CC pins**, so CC is never handled by
   the ESP32. Empirically, across every bring-up test the gating factor was
   **VBUS**, not CC: the USB-A breakout worked (VBUS hard-wired to the rail), a
   4-pin USB-C breakout worked (VBUS sourced from the board's 5V), and the devkit's
   native USB-C port stayed **dark** because its VBUS is an _input_, never sourced
   outward. The tested keyboard enumerates on VBUS alone (tolerates a floating CC).
   So on the PCB the non-negotiable is: **source 5V onto the C receptacle's VBUS**,
   and route D+/D− (both orientations, A6/B6 & A7/B7) to GPIO20/19. External
   **Rp (≈56 kΩ ×2 to 5V) is optional** here — add it only to support arbitrary,
   spec-strict USB-C keyboards; the current one does not need it.
   → Test #5 passes as long as VBUS is sourced (with the tested keyboard).
2. **The charger answers on I2C before anything else can be trusted.** A
   **BQ25896** at `0x6B` on SDA 17 / SCL 18 owns the whole power path: it selects
   between the USB-C input and the cell, limits input current, and feeds SYS,
   which the 3V3 buck-boost runs off. No discrete FET, no diode, no separate
   gauge. If it does not answer, check #7 is a NOK and the numbers every other
   power question depends on are simply absent.

   Two rails are **switched by firmware and off at reset**: the µSD's 3V3
   (`SD_PWR_EN`, IO40) and the keyboard's 5 V (`KBD_5V_EN`, IO41), both
   active-high through an N-FET with a 100 kΩ pulldown. The fixture raises them
   first — without that, checks #4 and #5 test dead hardware and report faults
   that are not there.

## Target pin map (reused from the devkit, unchanged)

Whole-build wiring reference-of-record: [hardware/wiring.md](../../hardware/wiring.md).

| Function                    | Bus / pin                                               |
| --------------------------- | ------------------------------------------------------- |
| EPD (SSD1683 / GDEY0579T93) | SPI2 — SCK 12, MOSI 11, CS 7, DC 6, RST 5, BUSY 4       |
| microSD                     | SPI3 — SCK 14, MOSI 15, MISO 13, CS 10                  |
| USB-C keyboard              | native PHY — D− 19, D+ 20, VBUS→5V, CC1/CC2 Rp          |
| Status LED (WS2812)         | GPIO 48 (devkit RGB)                                    |
| Operator confirm            | **BOOT button, GPIO 0** (input, pull-up, pressed = low) |
| Charger (BQ25896 @ 0x6B)    | I2C — SDA 17, SCL 18; `PMIC_INT` 16 (unused)            |
| Power button (`PWR_SENSE`)  | GPIO 21, active-low, internal pull-up                   |
| Button LED                  | GPIO 38, active-high                                    |
| Switched rails              | `SD_PWR_EN` 40, `KBD_5V_EN` 41 — both active-high       |

Free for later: GPIO 1, 2, 8, 9 (ADC1), 39, 42, 47.
Off-limits: 26–37 (flash + octal PSRAM), 43/44 (console UART), 0/3/45/46 (strapping).

## Architecture

- New binary `firmware/src/bin/qc.rs`; recipe **`just qc`** (build + flash +
  monitor). No `git` feature — keeps the build light and fast.
- Reuses the proven drivers: `Epd`, `SdStorage`, `usb_kbd`, `NetService`.
- **Run to completion, never abort on first fault.** Each check is isolated (its
  `Result` is captured); a NOK is recorded and the suite continues, so one flash
  yields the whole fault matrix instead of fix-one-reflash-repeat.
- **Verdict model** per line: `OK` / `NOK` / `SKIP` (a dependency failed) /
  `CONFIRM?` (visual, resolved by the operator).

### Output — three layers

1. **Serial (UART0) — authoritative.** Full checklist + per-test diagnostics +
   timings. Always attached on the bench.
2. **WS2812 (GPIO 48) — coarse aggregate, survives a dead panel.** Amber = running,
   blue = waiting on operator, **green = all OK**, **red = ≥1 NOK**.
3. **EPD panel — checklist mirror,** painted only _after_ test #2 passes (the panel
   is itself under test).

### Operator input (no dedicated buttons on the board)

Visual items (EPD pattern, LED colors) are confirmed with the **BOOT button**:
short press = OK, long press (>1 s) = NOK. A 30 s timeout leaves the item at
`CONFIRM?` (non-gating) logged as "not confirmed", so an unattended run still
completes and the aggregate reflects the auto-checks.

## Test suite

Ordered; each row lists the auto criteria and what a NOK points at.

| #   | Check                 | Auto criteria                                                                                                                                                                                                                                                                                                                           | NOK → likely fault                                                                   |
| --- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 1   | **LED**               | _(deferred)_ drive R→G→B via RMT; then `CONFIRM?` — esp-idf-hal 0.46 replaced the RMT API, so this reports `SKIP` for now (serial + panel carry results)                                                                                                                                                                                | LED net / GPIO 48                                                                    |
| 2   | **EPD handshake**     | reset → BUSY toggles within timeout; `init()` ok; full refresh completes within the ~1.9 s BUSY budget                                                                                                                                                                                                                                  | CS/DC/RST/BUSY/SCK/MOSI open or bridged; BUSY stuck high = RST or BUSY               |
| 3   | **EPD pattern**       | checkerboard + seam test (x=396) + text; `CONFIRM?`                                                                                                                                                                                                                                                                                     | pixel noise = CS; missing band = MOSI/SCK; bad seam = dual-controller                |
| 4   | **SD**                | mount (`format_if_mount_failed=false`); CMD59/CRC accepted; log negotiated kHz; write→read a blob byte-identical; MISO idle-high (internal pull-up is enough; log if low)                                                                                                                                                               | swap/open on 13/14/15/10; MISO low = pull-up                                         |
| 5   | **USB-C keyboard**    | install host lib; enumerate (log VID:PID, expect 19f5:3255); claim boot iface; SET_PROTOCOL(boot)+SET_IDLE(0); poll EP 0x81. Prompt "press a key" → decode. Then "flip the connector, press again" → re-enumerate                                                                                                                       | no enum = VBUS / **CC Rp** / D+/D−; one orientation only = D pairs or CC not bridged |
| 6   | **Wi-Fi**             | scan → ≥1 AP found (log best RSSI); if creds present, associate + SNTP                                                                                                                                                                                                                                                                  | antenna / RF                                                                         |
| 7   | **Charger / battery** | BQ25896 answers at `0x6B`; one ADC sweep → VBAT / SYS / VBUS / charge current + charge state. VBAT under 2.5 V = no cell on the connector. Then the power button: `PWR_SENSE` idle-high, goes low on a press, and the button LED is `CONFIRM?`                                                                                          | no answer = SDA 17 / SCL 18 or the 3V3 pull-ups; no press = J2 or the 10 k series     |
| 8   | **GPIO short/open**   | for each pin marked _isolated_ in the expected-net table: drive it high, read all other isolated pins (input pull-down) → any unexpected follower = a bridge; then float + internal pull → read level (open only inferable via the pull). Bus pins skipped (covered functionally). Log "coupling-tested" vs "pull-tested only" honestly | solder bridge between adjacent nets                                                  |

## Expected-net table (fill from the schematic)

The short/open scan needs each GPIO classified. Bus pins are skipped (their
functional test covers them); only _isolated_ pins are meaningfully scanned.

| GPIO                               | Class            | Note                                                    |
| ---------------------------------- | ---------------- | ------------------------------------------------------- |
| 4,5,6,7,11,12                      | bus (EPD)        | skip — covered by #2/#3                                 |
| 10,13,14,15                        | bus (SD)         | skip — covered by #4                                    |
| 19,20                              | bus (USB)        | skip — covered by #5                                    |
| 0                                  | button           | BOOT, reads pull, low on press                          |
| 17,18                              | bus (I2C)        | skip — covered by #7                                    |
| 21,38,40,41                        | power            | skip — covered by #7 and the rails bring-up             |
| 48                                 | LED              | skip — covered by #1                                    |
| 1,2,8,9,16,39,42,47                | isolated / spare | scan candidates — **confirm which are actually routed** |

## Build integration

**Implemented** at `firmware/src/bin/qc.rs` (light build — no `git`/`full`
feature, so no libgit2). Compiles clean for xtensa. Recipes:

```
just qc         # build + flash + monitor
just build-qc   # compile only (the offline build check)
just monitor-qc # serial monitor with decoded backtraces
```

Status: builds clean; **not yet hardware-verified** (the carrier PCB is still in
soldering). The verdict/report harness lives inline in `qc.rs`.

## Out of scope / limits

- Rail voltages (5V/3V3) — multimeter. The cell's is not: check #7 reads VBAT,
  SYS and VBUS from the charger's ADC.
- Short/open scan: opens on genuinely floating pins are not reliably detectable.

## Rough effort

~1 day. Checks #2/#4/#5/#6 are lifts of proven spikes; new work is the verdict/
report harness, the LED + panel mirror, the BOOT-button confirm loop, the charger
I2C read, and the short/open scanner.

## Open points / to clarify

- **USB-C keyboard runs with no CC resistor.** On the breadboard the keyboard is
  plugged straight into a 4-pin USB-C breakout with **no Rp/Rd on CC anywhere**, and
  it works. It enumerates because the keyboard tolerates a floating CC and only needs
  VBUS — but that is **not USB-C spec-compliant**, so a different or stricter keyboard
  may not come up on this board. Cheap USB-A→C adapters bake in a **56 kΩ Rp (CC→VBUS)**
  for exactly this; adding that one resistor (or a DNP footprint for it) on the PCB
  is cheap insurance if keyboard-independence ever matters.
  - **Confirmed (breadboard):** the keyboard enumerates _and_ decodes keystrokes on
    the CC-less breakout — so on the PCB, sourcing VBUS is sufficient for this
    keyboard. The 56 kΩ Rp stays optional insurance for other keyboards.
- **Charge current.** The firmware programs 1856 mA (~0.5 C for the 3700 mAh
  cell), a generic LiPo figure — the EEMB 103395's own sheet has not been read.
  Check the cell's temperature during a full charge before trusting it.
- **Cell internal resistance.** The state-of-charge estimate backs out the IR
  drop with a 100 mΩ guess (`app::CELL_MILLIOHMS`). Measure it — VBAT with and
  without charge current, at a known charge — and correct the constant.
- **Standby draw.** The design budgets ~84 µA in deep sleep. Measure it with the
  machine switched off: anything far above means one of the shed rails did not
  actually drop, or the panel missed its `0x10`/`0x03` deep sleep.
- **Expected-net table.** Fill the isolated/spare GPIO rows from the actual schematic
  before relying on the short/open scan (#8).
