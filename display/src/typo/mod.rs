//! Typo, o tucano — the companion character's baked 1-bit sprites.
//!
//! The sprite data ([`sprites`]) is generated offline by `display/tools/typogen.py`
//! from Julien's reference drawing (`typo_ref.png`): a straight threshold of his
//! line art, never a redrawn interpretation. The [`BODY`] faces right, as drawn —
//! it fronts the boot splash. The mood faces are the same grid mirrored (Typo
//! watches the writing column from the side panel) plus a few pixels of overlay
//! per [`Mood`].
//!
//! Moods ride the e-ink refresh cycle, so every swap is free or already paid for:
//! [`Mood::Frustrated`] appears at a typing pause once ghosting has built up
//! (the pause repaint is a full-area partial anyway — the dust lands on *his*
//! feathers, the screen's residue, never blamed on the writer), and one of the
//! six [`POOL`] humors is drawn into each *earned* full-refresh frame, rotating
//! so the flash never plays the same beat twice (the boot-splash cleanup flash is
//! the one exception — Typo stays neutral there, greeting nobody mid-anticipation
//! before a word is written). The host render engine (`app::Panel`)
//! owns those transitions; the editor only stores the current mood and paints it.

use crate::Frame;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

mod sprites;

/// A 1-bit sprite: one `u128` per row, low `w` bits used, high bit (bit `w-1`) =
/// leftmost pixel — the same left-to-right literal convention as [`crate::Glyph`].
/// Set bit = ink. The `u128` row caps width at 128 px; the faces are 96 px, the
/// (unflipped) boot mark 124 px.
pub struct Sprite {
    pub w: u16,
    pub h: u16,
    rows: &'static [u128],
}

/// The unmirrored base sprite (faces right, as drawn) — the boot-splash mark.
/// Baked from a larger cut of the reference (124 px) than the 96 px faces: the
/// splash has the whole panel, so it takes the extra detail; the side-panel face
/// box does not.
pub static BODY: &Sprite = &sprites::BODY;

/// A tighter 67×67 cut of the same mark, for chrome the 124 px boot mark won't fit.
/// Baked with the family; nothing on-device uses it yet.
pub static MARK_COMPACT: &Sprite = &sprites::MARK_COMPACT;

/// The side-panel face for each mood — mirrored to face the writing column.
/// `Neutral` is the bare mirrored reference; every other mood overlays a few
/// pixels on it (an eyelid, a brow, a floating `?`), so the silhouette never
/// changes and a swap only ever touches a handful of pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mood {
    #[default]
    Neutral,
    /// Pre-refresh: ghosting has built up — half-lid eye, knitted brow, ghost
    /// dust on his feathers.
    Frustrated,
    Anticipation,
    Wink,
    Curious,
    Determined,
    Zen,
    Note,
}

/// The post-flash humor pool: after every full refresh the host rotates to the
/// next of these six, so the flash always lands on a fresh take on "keep going".
pub const POOL: [Mood; 6] = [
    Mood::Anticipation,
    Mood::Wink,
    Mood::Curious,
    Mood::Determined,
    Mood::Zen,
    Mood::Note,
];

/// The `face` pref's palette cycle: `"random"` (the shuffle-bag default) plus
/// every mood by name, so the `>` palette can pin Typo to one face — to preview
/// each on demand instead of waiting for a full refresh to roll it up, or to
/// keep a favourite. Mirrors `display::FONT_OPTIONS`; order is the cycle order.
pub const FACE_OPTIONS: [&str; 9] = [
    "random",
    "neutral",
    "anticipation",
    "wink",
    "curious",
    "determined",
    "zen",
    "note",
    "frustrated",
];

impl Mood {
    /// The mood a `face` pref string pins to, or `None` for `"random"` — and for
    /// any unrecognized value, so a hand-typed pref can never leave Typo faceless;
    /// it just falls back to the random rotation. Inverse of the [`FACE_OPTIONS`]
    /// names.
    pub fn from_name(name: &str) -> Option<Mood> {
        Some(match name {
            "neutral" => Mood::Neutral,
            "anticipation" => Mood::Anticipation,
            "wink" => Mood::Wink,
            "curious" => Mood::Curious,
            "determined" => Mood::Determined,
            "zen" => Mood::Zen,
            "note" => Mood::Note,
            "frustrated" => Mood::Frustrated,
            _ => return None, // "random" or unknown → the shuffle-bag rotation
        })
    }

    pub fn face(self) -> &'static Sprite {
        match self {
            Mood::Neutral => &sprites::NEUTRAL,
            Mood::Frustrated => &sprites::FRUSTRATED,
            Mood::Anticipation => &sprites::ANTICIPATION,
            Mood::Wink => &sprites::WINK,
            Mood::Curious => &sprites::CURIOUS,
            Mood::Determined => &sprites::DETERMINED,
            Mood::Zen => &sprites::ZEN,
            Mood::Note => &sprites::NOTE,
        }
    }
}

/// Paint a sprite at `(x, y)`, each sprite pixel as a `scale`×`scale` block of
/// ink. Transparent: only set bits are painted, so the sprite composes over
/// whatever paper (or splash layout) is already there — the frame is rebuilt
/// white on every `draw_into`, and the dark theme's whole-frame invert flips the
/// sprite along with everything else.
pub fn blit_sprite(f: &mut Frame, x: i32, y: i32, s: &Sprite, scale: i32) {
    for (sy, &row) in s.rows.iter().enumerate() {
        for sx in 0..s.w as i32 {
            if row >> (s.w as i32 - 1 - sx) & 1 == 1 {
                Rectangle::new(
                    Point::new(x + sx * scale, y + sy as i32 * scale),
                    Size::new(scale as u32, scale as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(f)
                .unwrap(); // Frame's DrawTarget error is Infallible
            }
        }
    }
}
