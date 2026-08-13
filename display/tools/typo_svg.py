#!/usr/bin/env python3
"""Bake Typo's README badge from Julien's reference drawing.

Same base truth and same threshold as typogen.py — this just skips the e-ink
grid. The firmware sprite is 124 px wide because the panel says so; the README
is vector, so it thresholds the reference at 512 px and keeps the line art
crisp at any zoom. Ink flips with the reader's colour scheme via a media query
inside the SVG, so the mark stays visible on GitHub's dark theme.

  Regenerate:
    python3 display/tools/typo_svg.py
"""
import os
import tempfile

import typogen

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_SVG = os.path.join(HERE, '..', '..', 'docs', 'assets', 'typo.svg')
WIDTH = 512

INK_LIGHT = '#1c1917'
INK_DARK = '#faf9f7'


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
    with tempfile.TemporaryDirectory() as td:
        grid = typogen.resample(WIDTH, td)
    w, h = len(grid[0]), len(grid)
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
          f'({w}x{h}, {len(svg) / 1024:.1f} KB)')


if __name__ == '__main__':
    main()
