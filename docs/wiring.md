# Typoena — wiring reference

> Every electrical connection in the build, one table per subsystem. Companion
> to [bom.md](bom.md) (_what_ each part is — this file is _how they connect_).
> Pin numbers are ESP32-S3-DevKitC-1 v1.0 GPIOs, verified against the firmware
> (`firmware/src/main.rs`, `firmware/src/infrastructure/storage_sd.rs`) and the
> QC pin map in [`firmware/docs/bench-qc.md`](../firmware/docs/bench-qc.md).
> Power-path design rationale + behavior contract:
> [v0.10 — Power switch + power path](v0.10-battery-and-sleep.md#power-switch--power-path--decided-2026-07-26).

## ESP32-S3 pin map

| GPIO      | Net                    | Notes                                                  |
| --------- | ---------------------- | ------------------------------------------------------ |
| 4         | EPD BUSY               | Input, pull-down — active-high (high = busy)           |
| 5         | EPD RST                | Output                                                 |
| 6         | EPD DC                 | Output                                                 |
| 7         | EPD CS                 | Output                                                 |
| 11        | EPD MOSI               | SPI2                                                   |
| 12        | EPD SCK                | SPI2                                                   |
| 10        | SD CS                  | SPI3, internal pull-up                                 |
| 13        | SD MISO                | SPI3, internal pull-up                                 |
| 14        | SD SCK                 | SPI3, internal pull-up                                 |
| 15        | SD MOSI                | SPI3, internal pull-up                                 |
| 19        | USB D−                 | Native PHY → keyboard USB-C breakout                   |
| 20        | USB D+                 | Native PHY → keyboard USB-C breakout                   |
| 21        | `CHRG` (optional)      | HW-373 charge status, open-drain, low = charging       |
| 0         | BOOT button            | Devkit strapping pin; QC operator confirm              |
| 48        | WS2812 LED             | On-devkit RGB (board rev v1.0 — not 38)                |
| ⟨TBD⟩     | `PWR_HOLD`             | Output → 2N7002 gate (see power signals below)         |
| ⟨TBD⟩     | `PWR_SENSE`            | Input, internal pull-up ← button via 10 kΩ             |
| ⟨TBD⟩     | Battery sense (v0.10)  | Must be **ADC1** (GPIO 1/8/9) — Wi-Fi disables ADC2    |

Free for later: 1, 2, 8, 9 (ADC1), 16/17/18, 38, 39, 40, 41, 42, 47.
Off-limits: 26–37 (flash + octal PSRAM), 43/44 (console UART), 0/3/45/46
(strapping).

## E-paper — DESPI-C579 header → devkit (PCB 1)

Panel FPC clips into the DESPI-C579; its 8-pin header wires to the devkit.

| DESPI-C579 pin | Devkit        |
| -------------- | ------------- |
| VCC            | 3V3           |
| GND            | GND           |
| SDI            | GPIO 11       |
| SCK            | GPIO 12       |
| CS             | GPIO 7        |
| D/C            | GPIO 6        |
| RES            | GPIO 5        |
| BUSY           | GPIO 4        |

## microSD — breakout on PCB 2 → devkit

Dedicated SPI3 host (own bus, no sharing with the EPD — ADR-012); firmware
enables internal pull-ups on all four lines.

| µSD breakout pin | Devkit    |
| ---------------- | --------- |
| VCC              | 3V3       |
| GND              | GND       |
| SCK / CLK        | GPIO 14   |
| MOSI / CMD       | GPIO 15   |
| MISO / DAT0      | GPIO 13   |
| CS / DAT3        | GPIO 10   |

## USB-C keyboard host — breakout on PCB 2 → devkit

The S3's native PHY is the host; the hard requirement is **sourcing 5 V onto
VBUS** (the S3 has no CC pins). Details + bench evidence:
[`bench-qc.md`](../firmware/docs/bench-qc.md).

| Breakout pin | Connects to                                          |
| ------------ | ---------------------------------------------------- |
| VBUS         | 5 V rail (sourced **out** to the keyboard)           |
| D+           | GPIO 20 — both orientation pads bridged (A6 & B6)    |
| D−           | GPIO 19 — both orientation pads bridged (A7 & B7)    |
| GND          | GND                                                  |
| CC1/CC2      | Optional 56 kΩ Rp → VBUS for stricter keyboards      |

## USB-C charge-in — breakout on PCB 2

| Breakout pin | Connects to                                        |
| ------------ | -------------------------------------------------- |
| VBUS         | HW-373 IN+ — also the "mains 5 V" power-path net   |
| GND          | HW-373 IN−                                         |
| D+/D−        | Unused                                             |

## Power path

Circuit diagram, behavior contract, and placement invariants live in the
[v0.10 section](v0.10-battery-and-sleep.md#power-switch--power-path--decided-2026-07-26).
Not covered there: the MT3608 output (trimmed ~5.0 V, ≤ 5.5 V) feeds the
devkit's **5V pin** and the keyboard VBUS; all grounds are common.

### Power / control signals

| Signal        | GPIO  | Wiring                                                                |
| ------------- | ----- | --------------------------------------------------------------------- |
| `PWR_HOLD`    | ⟨TBD⟩ | → 2N7002 gate (100 kΩ pulldown)                                       |
| `PWR_SENSE`   | ⟨TBD⟩ | ← button through 10 kΩ series, internal pull-up                       |
| `CHRG`        | 21    | ← HW-373 charge-status pad, one wire (optional — pad access unconfirmed) |
| Battery sense | ⟨TBD⟩ | Planned (v0.10): 100k/100k divider + 100 nF into ADC1 — or MAX17048   |

## Off-board connectors

| Connector    | Family                | Wiring                                                                                       |
| ------------ | --------------------- | -------------------------------------------------------------------------------------------- |
| Battery      | JST-PH 2.0 mm 2-pin   | Pack plug → PCB 2 pigtail. :alert-triangle: polarity not standardized — verify red = B+ first |
| Power button | JST-XH 2.54 mm 2-pin  | Panel-mount button → latch-gate net + GND; gate signal only (µA)                             |
| Panel FPC    | 24-pin FPC            | Folds back through the case's internal clearance slot into the DESPI-C579                    |

## PCB 1 ↔ PCB 2 harness

PCB 1 (devkit + DESPI-C579 + MT3608, back-left) ↔ PCB 2 (µSD + 2× USB-C +
HW-373, back-right), rainbow ribbon + F-F Dupont (~22 mm vertical stack — see
the case README). Nets crossing the boards:

- SD SPI: GPIO 14/15/13/10 + 3V3 + GND
- Keyboard USB: GPIO 19/20 + 5 V (VBUS) + GND
- Power: load node → MT3608 input (the soft-latch feed), plus `PWR_HOLD`,
  `PWR_SENSE`, and optional `CHRG`
- ⟨TBD⟩ which board hosts the discrete power-path parts (FETs, diodes, latch
  network) — placement not yet decided
