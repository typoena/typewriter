# Manufacturing — Typoena enclosure

The model is nominal geometry. This correction is applied at slicing. Without it
the parts do not assemble.

| correction | value |
| --- | --- |
| **XY compensation** | **0.08 mm per side** — contours shrink, holes grow |

Measured: 0.164 mm of over-extrusion across a feature, hence 0.08 a side. Z needs
no correction.

The value is an offset applied to every polygon, so it is stated **per side**: a
contour loses it off each edge and a hole gains it. In Cura that is **Horizontal
Expansion** (`xy_offset`, under *Shell*) at **-0.08**. Leave `hole_xy_offset`
(*Hole Horizontal Expansion*) at 0 — it stacks on top of this one and would
correct every insert bore twice.
