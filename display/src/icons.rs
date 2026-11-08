//! Small 1-bit UI icons, rasterized from [Tabler Icons](https://tabler.io/icons)
//! (outline set, 24 px viewBox rendered 1:1 by `rsvg-convert`, alpha ≥ 50 %)
//! and cropped to their ink. Drawn with [`crate::typo::blit_sprite`] at scale 1.

use crate::typo::Sprite;

/// Tabler `battery-1` … `battery-4`, indexed by bar count − 1. 18×12, so one
/// fits a FONT_9X15 panel row with room to spare.
pub static BATTERY: [Sprite; 4] = [
    Sprite {
        w: 18,
        h: 12,
        rows: &[
            0b011111111111111100,
            0b111111111111111110,
            0b110000000000000110,
            0b110110000000000110,
            0b110110000000000011,
            0b110110000000000011,
            0b110110000000000011,
            0b110110000000000011,
            0b110110000000000110,
            0b110000000000000110,
            0b111111111111111110,
            0b011111111111111100,
        ],
    },
    Sprite {
        w: 18,
        h: 12,
        rows: &[
            0b011111111111111100,
            0b111111111111111110,
            0b110000000000000110,
            0b110110110000000110,
            0b110110110000000011,
            0b110110110000000011,
            0b110110110000000011,
            0b110110110000000011,
            0b110110110000000110,
            0b110000000000000110,
            0b111111111111111110,
            0b011111111111111100,
        ],
    },
    Sprite {
        w: 18,
        h: 12,
        rows: &[
            0b011111111111111100,
            0b111111111111111110,
            0b110000000000000110,
            0b110110110110000110,
            0b110110110110000011,
            0b110110110110000011,
            0b110110110110000011,
            0b110110110110000011,
            0b110110110110000110,
            0b110000000000000110,
            0b111111111111111110,
            0b011111111111111100,
        ],
    },
    Sprite {
        w: 18,
        h: 12,
        rows: &[
            0b011111111111111100,
            0b111111111111111110,
            0b110000000000000110,
            0b110110110110110110,
            0b110110110110110011,
            0b110110110110110011,
            0b110110110110110011,
            0b110110110110110011,
            0b110110110110110110,
            0b110000000000000110,
            0b111111111111111110,
            0b011111111111111100,
        ],
    },
];

/// Tabler `bolt`, the charging mark beside [`BATTERY`]. The 1:1 render is 20 px
/// tall, past a panel row, so this one is rendered at 14 px with the stroke
/// thickened to 2.75 to keep the battery's weight — 10×12, the battery's height.
pub static BOLT: Sprite = Sprite {
    w: 10,
    h: 12,
    rows: &[
        0b0000010000,
        0b0000110000,
        0b0001110000,
        0b0001110000,
        0b0011011111,
        0b0110011111,
        0b1111100110,
        0b1111101100,
        0b0000111000,
        0b0000111000,
        0b0000110000,
        0b0000100000,
    ],
};
