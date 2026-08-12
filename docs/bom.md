# Typoena — project BOM

> Every physical part in the build, one table per subsystem. Purchase links for
> the power-path parts live in
> [v0.10 — Power switch + power path](v0.10-battery-and-sleep.md#power-switch--power-path--decided-2026-07-26)
> (status-checked 2026-07-26); this file is the reference-of-record for _what_
> each part is, and [wiring.md](wiring.md) for _how the parts connect_ (pin
> map, per-subsystem nets, inter-board harness). Rows marked `⟨ref?⟩` need the
> exact reference confirmed by Julien.

## Core electronics

| Part                | Reference / pick                                                                                   | Notes                                                                                                             |
| ------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| MCU dev board       | **ESP32-S3-DevKitC-1 v1.0** (module **ESP32-S3-WROOM-1-N16R8**: 16 MB flash, 8 MB octal PSRAM)     | Board rev v1.0 confirmed (RGB LED on GPIO 48, not 38). Pinout: `firmware/docs/esp32-s3-devkitc-1-v1.0-pinout.jpg` |
| E-paper panel       | **Good Display GDEY0579T93** — 5.79″ B/W, 792 × 272, dual SSD1683                                  | Glass 150.92 × 56.94 × 1.0 mm, active 139.00 × 47.74 mm; FPC exits left                                           |
| E-paper adapter     | **Good Display DESPI-C579**                                                                        | Panel FPC → SPI breakout; usually sold as a kit with the panel                                                    |
| Keyboard            | User-supplied 60 % USB keyboard — bench unit is a **NuPhy (VID:PID 19f5:3255)** ⟨exact model ref?⟩ | Any boot-protocol HID keyboard works; VBUS-only USB-C host port (no CC)                                           |
| µSD card            | Genuine **SDHC ≤ 32 GB** (bench: 32 GB)                                                            | Hard requirement: large/counterfeit SDXC rejects CMD59 (CRC) in SPI mode — firmware refuses it                    |
| µSD slot / breakout | microSD→pin SPI adapter board, soldered to PCB 2 ⟨ref?⟩                                            |                                                                                                                   |
| USB-C breakout ×2   | 4-pin USB-C breakout (VBUS/D+/D−/GND), one keyboard-host + one charge-in ⟨ref?⟩                    | Keyboard port sources 5 V straight onto VBUS; optional 56 kΩ Rp (CC→VBUS) for stricter keyboards                  |

## Power

Design + behavior contract + purchase links:
[v0.10 power-path section](v0.10-battery-and-sleep.md#power-switch--power-path--decided-2026-07-26).

| Part                           | Reference / pick                                                                                                             | Notes                                                                                                                                                                           |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Charger board                  | **HW-373** (TP4056 + DW01A/FS8205A protection, USB-C)                                                                        | On PCB 2. Bare TP4056 is not a power-path controller — hence the P-FET/Schottky circuit                                                                                         |
| Boost converter                | **MT3608** module, trimmed to ~5 V — bought: [AZDelivery](https://www.amazon.fr/dp/B089QHXF8K)                               | On PCB 1. No fixed-output module in this current class — set the trim pot to 5.0 V with a meter before wiring the load, ≤ 5.5 V max (devkit LDO stress); input peaks ~2 A during refresh + Wi-Fi |
| Battery                        | **EEMB 103395** — 1S LiPo 3.7 V 3700 mAh, 10.3 × 33 × 95 mm, JST-PH 2.0 plug ([Amazon](https://www.amazon.fr/dp/B08215B4KK)) | Bought, not yet installed — measure the real cell (incl. lead exit) and fill the scad's `bat_*` `<< MEASURE >>`                                                                 |
| Power switch                   | **QitinDasen** Ø12 mm latching push button, red LED, IP66, pre-wired — bought ×3 (2026-08-01)                                | Gate signal only, never in the load path; flip on = boot, flip off = clean shutdown + sleeping-Typo off card; LED stays **unwired** — contract + rationale in the [v0.10 power-path section](v0.10-battery-and-sleep.md#power-switch--power-path--decided-2026-07-26) |
| P-MOSFET ×2                    | **AO3401A** (SOT-23, P-ch 30 V 4 A)                                                                                          | #1 load-sharing switchover, #2 soft-power latch in the load path                                                                                                                |
| N-MOSFET                       | **2N7002** (SOT-23)                                                                                                          | `PWR_HOLD` GPIO holds the latch gate low; 100k pulldown so a crashed ESP32 releases it                                                                                          |
| Signal diode                   | **1N4148**                                                                                                                   | Button → latch gate pull; keeps `PWR_SENSE` readable                                                                                                                            |
| Schottky diode                 | **SS34** (3 A 40 V; SOD-123F or the easier-to-solder SMA)                                                                    | Mains → load node feed                                                                                                                                                          |
| Resistors / cap                | 100 kΩ ×3, 10 kΩ ×1, ≥ 100 nF ×1                                                                                             | Gate pull-up/pulldowns, `PWR_SENSE` series, press-stretch cap — any stock                                                                                                       |
| Battery connector              | **JST-PH 2.0 mm** 2-pin — bought (2026-08-01): 20× male plugs + 20× pre-wired female pigtails, 10 cm red/black silicone      | :alert-triangle: polarity not standardized — verify red = B+ before first plug-in                                                                                               |
| Button connector               | **JST-XH 2.54 mm** 2-pin, pre-wired pair                                                                                     | Different family from the battery on purpose — can't cross-mate                                                                                                                 |
| Reed switch (lid close, v0.10) | Not yet picked ⟨ref?⟩                                                                                                        | Deep-sleep wake source; blocked on the hinged-lid decision                                                                                                                      |

## Boards & interconnect

| Part        | Reference / pick                                   | Notes                                                                   |
| ----------- | -------------------------------------------------- | ----------------------------------------------------------------------- |
| PCB 1       | Perfboard/protoboard **50 × 70 mm** ⟨ref?⟩         | Devkit + DESPI-C579 + MT3608; Ø2 corner holes                           |
| PCB 2       | Perfboard/protoboard **20 × 80 mm** ⟨ref?⟩         | µSD + 2× USB-C + HW-373; connectors overhang 8 mm                       |
| Jumper wire | F-F Dupont rainbow jumpers + rainbow ribbon ⟨ref?⟩ | Vertical rows make PCB 1 ~22 mm tall — drives the case's rear placement |
| FFC coupler | **sourcing map** 24-pin ↔ 24-pin FFC/FPC extension adapter, 0.5 mm pitch — bought ×2 (2026-08-01) | Joins the panel FPC to the extension cable below, for slack routing panel → DESPI-C579 |
| FFC cable   | AWM 20624 flat flex, 24P, 0.5 mm pitch, 100 mm, **Type B** (opposite-side contacts) — bought ×2   | Pairs with the coupler; Type B flips contact side — check orientation at the DESPI-C579 end |

## Enclosure

Model: `hardware/case/typoena-case.scad`; assembly + print notes:
[hardware/case/README.md](../hardware/case/README.md).

| Part                          | Reference / pick                                          | Notes                                                         |
| ----------------------------- | --------------------------------------------------------- | ------------------------------------------------------------- |
| Filament, body                | PLA/PETG, matte **sage `#B6CEB4`** ⟨ref?⟩                 |                                                               |
| Filament, bracket + baseplate | PLA/PETG, cream or brass ⟨ref?⟩                           | Two-tone via filament swap                                    |
| Threaded inserts              | **ruthex RX-6-32×3.8** brass heat-set, `GE-6-32X38-001` (bag of 50) | ×7 into the BODY: bracket bosses (×4) + baseplate posts (×3). Ø5.4 flange / Ø4.7 body; the print gives them a Ø4.8 bore with ≥1.8 mm of wall per the datasheet. Pressed in with a soldering iron before assembly — see *Fasteners* in [`hardware/case/README.md`](../hardware/case/README.md) |
| Screws, insert joints         | **#6-32 × 7 mm thread**, flat pan head 1.0 × Ø6.5 ⟨qty 7⟩ | Measured off the part, not a catalogue number — the model budgets bore depth at 8 mm of thread, so **caliper a screw if the batch changes**. The head sits in a lamage in the baseplate's underside, flush with the desk |
| Screws, PCBs                  | **M2 self-tapping** ⟨length/qty ref?⟩                     | PCB standoffs (×8) only. Deliberately NOT the insert family: the standoffs are 3 mm tall and an insert needs 4.8 |
| Foam gasket                   | **A4 Technologie MOU-BL-5-A** — closed-cell PE foam, 48 kg/m³, 5 × 200 × 300 mm, €2.40 ([a4.fr](https://www.a4.fr/mousse-polyethylene-45kg-m3-5-x-200-x-300-mm-bleue.html)) | Non-adhesive. Spreads the bracket's clamp load on the 1 mm glass, and its squash (`foam_t` 5.0 → `foam_c` 3.5, 30 %) IS the clamp preload — see the CONTRACT on `foam_c`. Density matters: 48 kg/m³ ≈ Plastazote LD45; cosplay EVA (85–125) is too firm at this squash. Cut as a frame, not a pad — `module foam()` is the template |
| Battery fixing                | Foam / VHB tape ⟨ref?⟩                                    | Plus the baseplate cage nibs                                  |
| Feet (optional)               | Printed (modelled) vs stick-on rubber bumpers — undecided | Open TODO in the case README                                  |
