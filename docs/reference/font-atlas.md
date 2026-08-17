# Font atlas and framebuffer conventions

The contract between `display/tools/fontgen.py`, the generated
`display/src/fonts/*.raw`, and the framebuffer in `display/src/lib.rs`.

## Cell and atlas geometry

| Constant | Value | Why |
|---|---|---|
| `CW`, `CH` | 10, 20 | Cell size; must match `editor::{CW, CH}` |
| `COLS` | 16 | Glyphs per atlas row — 160 px / 10 |
| `BASELINE` | 15 | Matches embedded-graphics `FONT_10X20` |
| `INK_THRESHOLD` | 128 | Gray below this is ink; pure 1-bit, no antialiasing |

Each `.raw` is 4800 bytes: 160×240, 1 bit per pixel, MSB first, a set bit means
ink.

## Glyph slot order

The atlas holds the 192 codepoints of embedded-graphics' `ISO_8859_15` mapping,
in its order (decoded from the range string in embedded-graphics 0.8's
`mono_font/mapping.rs`):

| Slots | Codepoints |
|---|---|
| 0..95 | `0x20`–`0x7F` — ASCII plus DEL |
| 96..99 | `0xA0`–`0xA3` |
| 100..104 | `0x20AC`, `0xA5`, `0x160`, `0xA7`, `0x161` — € ¥ Š § š |
| 105..115 | `0xA9`–`0xB3` |
| 116 | `0x17D` — Ž |
| 117..119 | `0xB5`–`0xB7` |
| 120 | `0x17E` — ž |
| 121..123 | `0xB9`–`0xBB` |
| 124..126 | `0x152`, `0x153`, `0x178` — Œ œ Ÿ |
| 127..191 | `0xBF`–`0xFF` |

`0x7F`, `0xA0` and `0xAD` are blanked: Pillow draws them as a `.notdef` box.

Characters outside this set are drawn from the hand-built bitmaps in
`display/src/glyphs.rs` (`extra_glyph`), not the atlas.

## Ink polarity

The framebuffer follows the SSD16xx convention: **bit 1 is white paper, bit 0 is
black ink.** So `Frame`'s pixel writer inverts against the usual reading of
`BinaryColor` — `On` (ink) clears the bit, `Off` (paper) sets it.
