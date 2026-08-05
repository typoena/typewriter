# Typoena — PCBs

KiCad 9 projects replacing the two perfboards described in
[`docs/bom.md`](../../docs/bom.md) ("Boards & interconnect"). Board split, sizes
and the case envelope are unchanged, so
[`hardware/case/typoena-case.scad`](../case/typoena-case.scad) needs no rework.

| Board | Size | Contents | Status |
| ----- | ---- | -------- | ------ |
| [`mainboard/`](mainboard) (PCB 1) | 50 × 70 mm | ESP32-S3-WROOM-1 + 3V3 LDO + power path + DESPI-C579 / MT3608 headers | **routed** — ERC 0, DRC 0, 0 unconnected pads; Gerbers in [`mainboard/gerbers/`](mainboard/gerbers) |
| PCB 2 | 20 × 80 mm | µSD + keyboard USB-C + charge USB-C + HW-373 | not started |

PCB 1 is 2-layer, SMD on the front only, both layers poured with ground.
Autorouted (Freerouting), then the ground stitching was pruned by hand — see
the antenna note below. **Not yet reviewed on a bench or ordered.**

## What changed versus the perfboard build

The devkit is gone — PCB 1 carries a bare **ESP32-S3-WROOM-1-N16R8**. That
moves three things the DevKitC-1 used to provide onto our board:

- **3V3 rail** — `U2` AMS1117-3.3, fed from the MT3608's 5 V output.
- **Strapping + buttons** — `SW1` RESET (EN), `SW2` BOOT (IO0), EN RC delay.
- **Programming** — `J2`, a 6-pin UART header (3V3 / GND / TXD0 / RXD0 / EN /
  IO0). The native USB pair is committed to the keyboard host port, so there is
  no USB-serial-JTAG path: **first flash is UART, then OTA**.

## Decisions this board forced

`docs/wiring.md` left three GPIOs and one placement question open. The layout
can't be drawn without answering them, so:

| Was `⟨TBD⟩` | Now | Why |
| --- | --- | --- |
| `PWR_HOLD` | **IO40** | Free, not strapping, not PSRAM. Boots as input so the 100 k pulldown releases the latch on a crash, per the v0.10 contract |
| `PWR_SENSE` | **IO41** | Same, and adjacent to IO40 for a compact latch block |
| Battery sense | **IO1** | Must be ADC1 — Wi-Fi disables ADC2 |
| Which board hosts the power path | **PCB 1** | PCB 2 (20 × 80 mm) is already full with HW-373 + 2× USB-C + µSD. Dropping the devkit freed the room on PCB 1, and it keeps the back-feed-sensitive `PWR_SENSE` net on the same board as the MCU |

The symbol exposes IO35–37, but those are **octal PSRAM** on the N16R8
(`firmware/sdkconfig.defaults:40`) — left unconnected on purpose, along with the
other spares.

`J5` (button, JST-XH) sits on PCB 1 because the latch gate is here; the button
itself is panel-mount, so its **panel position next to the µSD is unchanged**.

## The antenna dominates the layout

The KiCad `ESP32-S3-WROOM-1` footprint carries its own keep-clear zone —
**48 × 21 mm**, drawn on `F.CrtYd` and as a real keepout. On a 70 mm-tall board
that rules out the top ~29 mm for everything except the module, which is why
every other part sits in the lower half and the ground pours stop short of the
top edge.

Two consequences, both deliberate:

- The module sits flush with the top edge rather than overhanging it. Overhang
  is what the datasheet prefers, but the case is fixed at 50 × 70 mm and
  overhang would need case rework. The copper keepout is still honoured, so the
  cost is some range, not function.
- `add_gnd_stitching_vias` does **not** know about keepouts — it happily dropped
  16 vias straight under the antenna. They were removed afterwards. Re-run that
  tool and you must prune again (anything with `y < 59.6 mm` here).

## Open items

- **PCB 2 and both layouts.** Only the PCB 1 schematic exists.
- **LDO efficiency.** 3V3 comes via battery → MT3608 boost → LDO, ≈ 56 % end to
  end. This is what the devkit already did, so it is not a regression, but a
  3.3 V buck straight off the load node would be materially better for runtime.
  Feeding the LDO from the load node instead is *not* a drop-in fix — an
  AMS1117 drops out below ≈ 4.4 V, well inside the cell's range.
- **AMS1117 thermals.** ≈ 0.85 W at a 500 mA Wi-Fi peak. Needs a real copper
  pour at layout time.
- **WS2812 status LED dropped.** The devkit's on-board LED was GPIO 48;
  `firmware/src/bin/qc.rs:167` drives it. Either fit one on IO48 or drop that QC
  step — currently IO48 is a spare.

## Building headlessly

The container runs KiCad 9 from an extracted AppImage; `kicad-cli` needs its
environment. From the repo root:

```sh
K=~/.local/share/com.jean.desktop/tools/kicad9
bash -c "source $K/kicad-env.sh && \$KICAD_CLI sch erc -o /tmp/erc.rpt --severity-all \
  hardware/pcb/mainboard/typoena-mainboard.kicad_sch"
```

Do not `source` that env into a shell you run other tools in — its
`LD_LIBRARY_PATH` breaks the system `curl`.
