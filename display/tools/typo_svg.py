#!/usr/bin/env python3
"""Bake Typo's README badge from the firmware's generated sprites.rs.

Same rule as typo_web.py in typoena-site: re-read THOSE packed rows (never a
redrawn interpretation). Ink flips with the reader's colour scheme via a media
query inside the SVG, so the mark stays visible on GitHub's dark theme.

  Regenerate:
    python3 display/tools/typo_svg.py
"""
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
SPRITES_RS = os.path.join(HERE, '..', 'src', 'typo', 'sprites.rs')
OUT_SVG = os.path.join(HERE, '..', '..', 'docs', 'assets', 'typo.svg')
SPRITE = 'BODY'

INK_LIGHT = '#1c1917'
INK_DARK = '#faf9f7'


def parse_sprite(path, name):
    src = open(path).read()
    m = re.search(
        rf'const {name}: Sprite = Sprite \{{\s*w: (\d+),\s*h: (\d+),\s*rows: &\[(.*?)\]',
        src, re.S)
    w, h = int(m.group(1)), int(m.group(2))
    rows = [int(tok, 16) for tok in re.findall(r'0x([0-9a-f]+)', m.group(3))]
    assert len(rows) == h, name
    return w, h, [[(bits >> (w - 1 - x)) & 1 for x in range(w)] for bits in rows]


def path_d(grid):
    """One path for the whole sprite: each horizontal run of ink is a 1-tall rect."""
    parts = []
    for y, row in enumerate(grid):
        x = 0
        while x < len(row):
            if row[x]:
                run = x
                while run < len(row) and row[run]:
                    run += 1
                parts.append(f'M{x} {y}h{run - x}v1h-{run - x}z')
                x = run
            else:
                x += 1
    return ''.join(parts)


def main():
    w, h, grid = parse_sprite(SPRITES_RS, SPRITE)
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">\n'
        f'<title>Typo, o tucano</title>\n'
        f'<style>path{{fill:{INK_LIGHT}}}'
        f'@media(prefers-color-scheme:dark){{path{{fill:{INK_DARK}}}}}</style>\n'
        f'<path d="{path_d(grid)}"/>\n'
        '</svg>\n'
    )
    os.makedirs(os.path.dirname(OUT_SVG), exist_ok=True)
    open(OUT_SVG, 'w').write(svg)
    print(f'wrote {os.path.relpath(OUT_SVG, os.path.join(HERE, "..", ".."))} '
          f'({SPRITE.lower()}, {w}x{h}, {len(svg)} bytes)')


if __name__ == '__main__':
    main()
