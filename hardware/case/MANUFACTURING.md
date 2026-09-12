# Manufacturing — Typoena enclosure

The model is nominal geometry: every dimension in `typoena-case.scad` is the
dimension the finished part must measure. The machine's own error belongs to the
process, so it is corrected here, in the slicer, and never in the model.

| setting | Cura | value |
| --- | --- | --- |
| **XY compensation** | *Horizontal Expansion* (`xy_offset`, under *Shell*) | **0** |
| **Hole XY compensation** | *Hole Horizontal Expansion* (`hole_xy_offset`) | **0** |
| **Z** | — | no correction |

The qualified filament lands on nominal: a baseplate comes out 170.7 × 98.7 to
within what a caliper resolves, X and Y alike. There is nothing to take out, and
`hole_xy_offset` stays at 0 as well — it stacks on top of the first one and would
correct every insert bore twice.
