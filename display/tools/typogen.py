#!/usr/bin/env python3
"""Bake Typo (o tucano, the companion character) from Julien's reference drawing.

Base truth is a 1-bit threshold of `typo_ref.png` — the sprite is Julien's own
line art pixelated, never a redrawn interpretation (three hand-drawn attempts
were rejected before this pipeline; the thresholded mark was approved on sight).
Faces are the same grid MIRRORED (so Typo watches the writing column from the
side panel) plus small per-mood pixel overlays; the body stays unmirrored, as
drawn, for the boot splash.

No PIL: macOS `sips` does the downsampling and PNG→BMP conversion, and the BMP
is parsed by hand. Threshold sum(r,g,b) < 700 catches the reference's light pink
strokes on white.

  Regenerate:
    python3 display/tools/typogen.py [--preview /tmp/typo_preview.png]

Outputs, next to the crate source:
    display/src/typo/sprites.rs   (the packed row arrays; the Sprite/Mood API
                                   lives hand-written in typo/mod.rs)
And, with --preview, a contact-sheet PNG (needs rsvg-convert) for eyeballing.
"""
import argparse
import json
import os
import struct
import subprocess
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REF = os.path.join(HERE, 'typo_ref.png')
OUT_RS = os.path.join(HERE, '..', 'src', 'typo', 'sprites.rs')


def bmp_grid(path, cut=700):
    """Threshold a BMP to a 1-bit grid (1 = ink), trimmed of empty margins."""
    d = open(path, 'rb').read()
    off = struct.unpack_from('<I', d, 10)[0]
    w = struct.unpack_from('<i', d, 18)[0]
    h = struct.unpack_from('<i', d, 22)[0]
    step = struct.unpack_from('<H', d, 28)[0] // 8
    rb = ((w * step + 3) // 4) * 4
    g = []
    for y in range(abs(h)):
        sy = (abs(h) - 1 - y) if h > 0 else y
        g.append([1 if d[off+sy*rb+x*step]+d[off+sy*rb+x*step+1]+d[off+sy*rb+x*step+2] < cut else 0
                  for x in range(w)])
    rows = [i for i, r in enumerate(g) if any(r)]
    cols = [i for i in range(w) if any(r[i] for r in g)]
    return [r[cols[0]:cols[-1]+1] for r in g[rows[0]:rows[-1]+1]]


def resample(width, tmpdir):
    out = os.path.join(tmpdir, f'r{width}')
    subprocess.run(['sips', '--resampleWidth', str(width), REF, '--out', out + '.png'],
                   check=True, capture_output=True)
    subprocess.run(['sips', '-s', 'format', 'bmp', out + '.png', '--out', out + '.bmp'],
                   check=True, capture_output=True)
    return bmp_grid(out + '.bmp')


def flip(g):
    return [list(reversed(r)) for r in g]


def px(g, x, y, v=1):
    if 0 <= y < len(g) and 0 <= x < len(g[0]):
        g[y][x] = v


def line(g, x0, y0, x1, y1, v=1):
    dx, dy = abs(x1-x0), -abs(y1-y0)
    sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
    err = dx + dy
    while True:
        px(g, x0, y0, v)
        if x0 == x1 and y0 == y1:
            break
        e2 = 2 * err
        if e2 >= dy:
            err += dy; x0 += sx
        if e2 <= dx:
            err += dx; y0 += sy


def sparkle(g, x, y, r=2):
    px(g, x, y)
    for d in range(1, r + 1):
        px(g, x + d, y)
        px(g, x - d, y)
        px(g, x, y + d)
        px(g, x, y - d)


def stamp(g, x, y, rows):
    """Blit a '#'/space bitmap (a glyph like `?` or a note) at (x, y)."""
    for dy, row in enumerate(rows):
        for dx, c in enumerate(row):
            if c == '#':
                px(g, x + dx, y + dy)


def disc(g, cx, cy, r, v=1):
    """Filled circle — the shape of Typo's eye, so mood eyes stay round."""
    for y in range(cy - r, cy + r + 1):
        for x in range(cx - r, cx + r + 1):
            if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + 1:
                px(g, x, y, v)


def copy(g):
    return [r[:] for r in g]


# Friendly, rounded glyphs, sized for the 96 px grid. `?` floats behind Typo's
# head when he's curious; the eighth note sits in the pocket under his beak tip
# when he whistles at a fresh page.
QUESTION = [
    " ##### ",
    "##   ##",
    "     ##",
    "    ## ",
    "   ##  ",
    "  ##   ",
    "  ##   ",
    "       ",
    "  ##   ",
    "  ##   ",
]
NOTE = [
    "    ##",
    "   ###",
    "  # ##",
    "    # ",
    "    # ",
    "    # ",
    " ###  ",
    "####  ",
    "####  ",
]


def build(tmpdir):
    # ---- base grid straight from the reference (~96 px) ---------------------
    # Overlays below were originally tuned on a 48-grid; kx/ky rescale them onto
    # whatever the reference now trims to, so a single knob (the resample width)
    # moves the whole family.
    base = resample(112, tmpdir)      # ~97x96 — the mirrored mood family
    splash = resample(143, tmpdir)    # ~124x121 — the bigger unflipped boot mark
    compact = resample(78, tmpdir)    # tighter cut, no additions at all

    w, h = len(base[0]), len(base)    # flipped x = w-1-x
    kx, ky = w / 48.0, h / 48.0

    def sx(v):
        return round(v * kx)

    def sy(v):
        return round(v * ky)

    # Locate Typo's eye — the isolated ink dot right of the crown, left of the
    # beak — so every mood reshapes the reference's OWN round eye instead of
    # stamping a block over it. Window excludes the crown (<0.60w) and beak
    # (>0.76w), and the solid upper head (<0.11h).
    neutral = flip(copy(base))
    xs, ys = [], []
    for y in range(round(0.11 * h), round(0.25 * h)):
        for x in range(round(0.60 * w), round(0.76 * w)):
            if neutral[y][x]:
                xs.append(x)
                ys.append(y)
    ecx, ecy = (min(xs) + max(xs)) // 2, (min(ys) + max(ys)) // 2
    er = max(max(xs) - min(xs), max(ys) - min(ys)) // 2   # eye radius (~4)

    def brow(g, x0, y0, x1, y1, t=2):
        """A brow / lash line `t` px thick — bold enough to read on the panel."""
        for d in range(t):
            line(g, x0, y0 + d, x1, y1 + d)

    def hbar(g, x0, x1, y, t=2, v=1):
        """A thick horizontal bar: a level brow, or a closed-eye line."""
        for yy in range(y, y + t):
            for x in range(min(x0, x1), max(x0, x1) + 1):
                px(g, x, yy, v)

    def catchlight(g, r):
        """The white spark that turns a filled eye into a bright, alive one."""
        disc(g, ecx - er // 2, ecy - er // 2, r, 0)

    def face(mood):
        """All moods are pixel overlays on the bare mirrored reference. Typo
        faces left, so his inner brow / beak side is to the LEFT (−x). Every
        change is deliberately BOLD: at 96 px on e-ink a 2 px catchlight or a
        thin brow vanishes, so eyes resize by whole radii and brows/lids run
        3 px thick (the zen / determined lines run 5 px — a bare line is the
        first thing the partial-refresh fade eats between full refreshes, so
        those two carry extra mass), each mood reading at a glance."""
        g = flip(copy(base))
        if mood == 'neutral':
            return g
        if mood == 'frustrated':                        # pre-refresh: ghosting builds
            for y in range(ecy - er, ecy):              # heavy lid drops over the top half
                for x in range(ecx - er - 1, ecx + er + 2):
                    px(g, x, y, 0)
            brow(g, ecx - er - 2, ecy - er, ecx + er + 2, ecy - er - 3, t=3)  # furrowed, inner-low
            for x, y in ((8, 1), (3, 12), (27, 17), (44, 4), (46, 13), (18, 20)):
                sparkle(g, sx(x), sy(y), 1)             # ghost dust on his feathers
            return g
        # ---- the post-flash pool: one of these after every full refresh -----
        if mood == 'anticipation':                      # eyes wide, buzzing
            disc(g, ecx, ecy, er + 2)                   # a visibly bigger eye
            catchlight(g, max(2, er // 2))              # big bright spark
            brow(g, ecx - er - 1, ecy - er - 5, ecx + er + 1, ecy - er - 5, t=2)  # brow shot up
            sparkle(g, sx(2), sy(18), 3)
            sparkle(g, sx(45), sy(4), 3)
            return g
        if mood == 'curious':                           # engaged interest, not a frown
            disc(g, ecx, ecy, er + 1)                   # bright, alive eye
            catchlight(g, max(1, er // 3))
            by = ecy - er - 5                           # a bold, high, arched brow: the "oh?"
            brow(g, ecx - er - 1, by + 3, ecx - er // 3, by, t=3)
            hbar(g, ecx - er // 3, ecx + er // 3, by, t=3)
            brow(g, ecx + er // 3, by, ecx + er + 1, by + 3, t=3)
            stamp(g, w - len(QUESTION[0]) - sx(1), sy(1), QUESTION)   # ? behind his head
            return g
        if mood == 'determined':                        # locked in: bold level brow over an open eye
            hbar(g, ecx - er - 1, ecx + er + 1, ecy - er - 3, t=3)  # in the white gap: clear of both crown and eye
            return g
        if mood == 'zen':                               # eyes softly, evenly shut
            disc(g, ecx, ecy, er + 2, 0)                # clear the eye out
            hbar(g, ecx - er - 2, ecx + er + 2, ecy - 1, t=5)  # a calm, thick closed line — 5 px to hold up as it fades
            return g
        if mood == 'note':                              # whistling at the fresh page
            stamp(g, sx(1), sy(17), NOTE)               # ♪ dropped clear of the beak, a gap under the tip
            return g
        raise ValueError(mood)

    moods = ['neutral', 'frustrated', 'anticipation', 'curious',
             'determined', 'zen', 'note']
    # BODY is the boot splash only (unflipped); it can run bigger than the 96 px
    # face box, so it uses the larger `splash` cut. The faces stay on `base`.
    sprites = {'body': splash, 'mark_compact': compact}
    sprites.update({m: face(m) for m in moods})
    return sprites


def emit_rust(sprites):
    parts = [
        "//! GENERATED by display/tools/typogen.py — do not edit by hand.\n",
        "//! Thresholded from typo_ref.png (Julien's reference drawing); the mood\n",
        "//! faces are the mirrored base plus pixel overlays. Regenerate with:\n",
        "//!   python3 display/tools/typogen.py\n",
        "\n",
        "use super::Sprite;\n",
    ]
    for name, g in sprites.items():
        w, h = len(g[0]), len(g)
        parts.append(f"\npub(super) const {name.upper()}: Sprite = Sprite {{\n")
        parts.append(f"    w: {w},\n    h: {h},\n    rows: &[\n")
        for row in g:
            bits = 0
            for x, v in enumerate(row):
                if v:
                    bits |= 1 << (w - 1 - x)
            parts.append(f"        0x{bits:0{(w + 3) // 4}x},\n")
        parts.append("    ],\n};\n")
    return ''.join(parts)


def emit_preview(sprites, path):
    """Contact sheet (8x + 1x per sprite) via SVG -> rsvg-convert, to eyeball."""
    z, pad = 8, 20
    order = list(sprites)
    total_w = sum(len(sprites[k][0]) * z + pad for k in order) + pad
    total_h = max(len(sprites[k]) * z for k in order) + pad * 2 + 60
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="{total_h}">',
             f'<rect width="{total_w}" height="{total_h}" fill="#e8e4da"/>']
    x = pad
    for k in order:
        parts.append(f'<text x="{x}" y="{pad-6}" font-family="monospace" font-size="13">{k}</text>')
        for yy, row in enumerate(sprites[k]):
            for xx, c in enumerate(row):
                if c:
                    parts.append(f'<rect x="{x+xx*z}" y="{pad+yy*z}" width="{z}" height="{z}"/>')
        oy = pad + len(sprites[k]) * z + 10
        for yy, row in enumerate(sprites[k]):        # 1x preview
            for xx, c in enumerate(row):
                if c:
                    parts.append(f'<rect x="{x+xx}" y="{oy+yy}" width="1" height="1"/>')
        x += len(sprites[k][0]) * z + pad
    parts.append('</svg>')
    svg = path + '.svg'
    open(svg, 'w').write(''.join(parts))
    subprocess.run(['rsvg-convert', svg, '-o', path], check=True)
    os.remove(svg)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--preview', help='also render a contact-sheet PNG here')
    args = ap.parse_args()
    with tempfile.TemporaryDirectory() as tmpdir:
        sprites = build(tmpdir)
    open(OUT_RS, 'w').write(emit_rust(sprites))
    print(f'wrote {os.path.relpath(OUT_RS, HERE)}:',
          json.dumps({k: f'{len(v[0])}x{len(v)}' for k, v in sprites.items()}))
    if args.preview:
        emit_preview(sprites, args.preview)
        print(f'preview: {args.preview}')


if __name__ == '__main__':
    main()
