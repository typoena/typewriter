# Typoena — PCBs

One KiCad 10 project, [`mainboard/`](mainboard/README.md): a single-board PCBA,
130 × 45 mm, 4 layers, carrying the BQ25896 charger and power path, the
ESP32-S3-WROOM-1, the panel power stage, the µSD and both USB-C ports.

| | |
| --- | --- |
| Schematic | hierarchical, 4 sheets — 97 components, ERC 0 |
| PCB | skeleton only — outline, stackup, mounting holes, net classes. Footprints not yet imported |
| Pinout, board shape, net classes | [`mainboard/README.md`](mainboard/README.md) |
| Component values and their datasheet sources | [`mainboard/DESIGN-NOTES.md`](mainboard/DESIGN-NOTES.md) |

## Building headlessly

`kicad-cli` 10 is on `PATH` on the desktop:

```sh
kicad-cli sch erc --severity-all -o /tmp/erc.rpt \
  hardware/pcb/mainboard/typoena-mainboard.kicad_sch
```

The container instead runs KiCad from an extracted AppImage, which needs its
own environment:

```sh
K=~/.local/share/com.jean.desktop/tools/kicad9
bash -c "source $K/kicad-env.sh && \$KICAD_CLI sch erc -o /tmp/erc.rpt --severity-all \
  hardware/pcb/mainboard/typoena-mainboard.kicad_sch"
```

Do not `source` that env into a shell you run other tools in — its
`LD_LIBRARY_PATH` breaks the system `curl`.
