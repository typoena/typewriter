# SSD1683 command reference

The command and data bytes `firmware/src/drivers/screen_epd.rs` writes to the
GDEY0579T93. The driver spells them as bare hex; this table is what each one
means.

The panel is a dual-controller device. Every command is OR-ed with a target
offset: `0x00` = master (right half), `0x80` = slave (left half). A command
written without an offset reaches the master only.

For what a waveform *is* and how the LUT bytes pack, see
`docs/record/notes/epd-waveforms.md` and
`docs/record/notes/epd-waveform-lut-deep-dive.md`. For pins, see ADR-012 and
`hardware/wiring.md`.

## Commands

| Cmd | Name | Used by |
|---|---|---|
| `0x03` | Gate driving voltage (VGH) | fast-partial recipe |
| `0x04` | Source driving voltage (VSH1, VSH2, VSL) | fast-partial recipe |
| `0x11` | Data entry mode | `set_ram_area` |
| `0x12` | SWRESET | `init` |
| `0x18` | Temperature sensor control | `init` |
| `0x1A` | Write temperature register | `init`, full refresh, partial |
| `0x20` | Master activation — runs whatever `0x22` armed | every refresh |
| `0x21` | Display update control 1 | full refresh, partial |
| `0x22` | Display update control 2 — arms the update mode | every refresh |
| `0x24` | RAM bank: **current** frame | frame and band writes |
| `0x26` | RAM bank: **previous** frame | frame and band writes |
| `0x2C` | VCOM | fast-partial recipe |
| `0x32` | LUT register — waveform phases plus FR/XON | fast-partial recipe |
| `0x37` | Display option | fast-partial recipe |
| `0x3C` | Border waveform control | partial, fast partial |
| `0x3F` | EOPT — LUT end option | fast-partial recipe |
| `0x44` | RAM X window, start/end | `set_ram_area` |
| `0x45` | RAM Y window, start/end | `set_ram_area` |
| `0x4E` | RAM X address counter | `set_ram_area` |
| `0x4F` | RAM Y address counter | `set_ram_area` |

`0x37` is not optional: the fast-partial recipe does not work without it. Log:
`docs/record/tradeoff-curves/epd-refresh-latency.md`.

## Data bytes

| Cmd | Value | Meaning |
|---|---|---|
| `0x18` | `0x80` | Use the internal temperature sensor |
| `0x21` | `0x40, 0x10` | Bypass the RED/previous bank as 0, cascade |
| `0x21` | `0x00, 0x10` | RED bank normal, cascade |
| `0x22` | `0xB1` | Enable clock, load temp, load LUT (B/W), disable clock |
| `0x22` | `0x91` | Load temp, load LUT (B/W), disable clock |
| `0x22` | `0xD7` | Fast full update |
| `0x22` | `0xFF` | Factory partial update, including load-temp and load-LUT |
| `0x22` | `0xCF` | Display with the LUT already in `0x32`, then power down |
| `0x22` | `0xCC` | `0xCF` without the disable-analog/disable-clock bits |
| `0x3C` | `0x80` | The border level this driver keeps (the vendor recipe uses `0xC0`) |
| `0x11` | `0x00`–`0x03` | X/Y increment or decrement, per `set_ram_area`'s match |

`0x22 ← 0xFF` reloads the LUT from OTP, so it overwrites a resident custom
recipe. `0xCF` and `0xCC` deliberately do not.

## Sequences

**Init** — `0x12` SWRESET, `0x18 ← 0x80`, `0x22 ← 0xB1` + `0x20`,
`0x1A ← 0x64, 0x00`, `0x22 ← 0x91` + `0x20`. The two master activations are
what load the temperature value and the OTP LUT.

**Full refresh** (~2200 ms BUSY) — window both halves, `0x21 ← 0x40, 0x10`,
`0x1A ← 0x64, 0x00`, `0x22 ← 0xD7`, `0x20`.

**Factory partial** — window both halves, `0x3C ← 0x80`, `0x21 ← 0x00, 0x10`,
`0x22 ← 0xFF`, `0x20`. Well under the full 2.2 s.

**Fast partial** — window both halves, then per controller: `0x32` (227-byte
phase table), `0x3F` EOPT, `0x03` VGH, `0x04` VSH1/VSH2/VSL, `0x2C` VCOM,
`0x37`. Then `0x3C ← 0x80`, `0x21 ← 0x00, 0x10`, `0x22 ← 0xCF`, `0x20`. The
whole recipe goes to both controllers — each half has its own waveform SRAM and
charge pump, so a master-only write leaves the halves ghosting differently.

**Evict the custom recipe** — `0x22 ← 0x91`, `0x20`. Reloads the OTP waveform
and leaves the RAM banks alone (~15 ms).

## Fast-partial LUT layout

`FAST_PARTIAL_LUT` is 233 bytes: `[0..227)` is the `0x32` phase table as 7-byte
rows, FR/XON at `[224..227)`, and the last 6 bytes fan out to EOPT, VGH,
VSH1/VSH2/VSL and VCOM.

The phase table holds four drive groups. Each group is three rows — main drive,
follow-up drive, tail — separated by zeroed rows, with six zeroed rows ahead of
the first group. Every group's tail row is zeroed: it measures as a ~2% near-noop
and is kept only because it is harmless. The FR byte is the lever that moves
refresh time.
