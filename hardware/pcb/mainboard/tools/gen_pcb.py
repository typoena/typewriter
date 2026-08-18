#!/usr/bin/env python3
"""Génère le squelette du PCB : contour, empilage 4 couches, trous de fixation.

Volontairement SANS les empreintes. KiCad les importe lui-même en un clic
(« Mettre à jour le PCB depuis le schéma ») avec les nets garantis corrects par
son propre importeur — les inliner à la main serait strictement plus risqué pour
un résultat identique.

Ce que ce script apporte et que l'import ne fait pas : le contour à la bonne cote,
l'empilage JLCPCB 4 couches, et les fixations.
"""
import os
import re
import uuid as _uuid

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   "typoena-mainboard.kicad_pcb")

W, H = 94.0, 45.0
R = 3.0
X0, Y0 = 40.0, 40.0
HOLE_INSET = 5.0


def uid():
    return str(_uuid.uuid4())


def edge(x1, y1, x2, y2):
    return f"""	(gr_line
		(start {x1:.3f} {y1:.3f})
		(end {x2:.3f} {y2:.3f})
		(stroke
			(width 0.1)
			(type default)
		)
		(layer "Edge.Cuts")
		(uuid "{uid()}")
	)
"""


def arc(sx, sy, mx, my, ex, ey):
    return f"""	(gr_arc
		(start {sx:.3f} {sy:.3f})
		(mid {mx:.3f} {my:.3f})
		(end {ex:.3f} {ey:.3f})
		(stroke
			(width 0.1)
			(type default)
		)
		(layer "Edge.Cuts")
		(uuid "{uid()}")
	)
"""


MH_LIB = "MountingHole:MountingHole_2.2mm_M2"
MH_PATH = "/usr/share/kicad/footprints/MountingHole.pretty/MountingHole_2.2mm_M2.kicad_mod"


def mounting_hole(x, y, n):
    """Trou Ø2,2 mm : vis M2 auto-taraudeuses dans des entretoises, pas de taraudage.

    On recopie l'empreinte de la bibliotheque telle quelle plutot que de la
    redessiner : une copie a la main ne correspond jamais exactement et le DRC
    signale un ecart avec la librairie a chaque fois.
    """
    src = open(MH_PATH).read().rstrip()
    assert src.startswith('(footprint "MountingHole_2.2mm_M2"')
    # nom qualifie + placement, injectes juste apres l'ouverture
    src = src.replace('(footprint "MountingHole_2.2mm_M2"',
                      f'(footprint "{MH_LIB}"\n\t\t(at {x:.3f} {y:.3f})\n\t\t(uuid "{uid()}")', 1)
    # les UUID internes doivent etre uniques d'une instance a l'autre
    src = re.sub(r'\(uuid "[0-9a-f-]{36}"\)',
                 lambda _m: f'(uuid "{uid()}")', src)
    # le repere sur la couche de fabrication : sur la serigraphie il deborde du contour
    src = src.replace('"Reference" "REF**"', f'"Reference" "MH{n}"')
    src = re.sub(r'(\(property "Reference"[^\n]*\n(?:[^\n]*\n)*?\s*)\(layer "F.SilkS"\)',
                 r'\1(layer "F.Fab")', src)
    src = src.replace("(attr exclude_from_pos_files)",
                      "(attr exclude_from_pos_files exclude_from_bom)")
    return "\t" + src.replace("\n", "\n\t") + "\n"


HEADER = f"""(kicad_pcb
	(version 20241229)
	(generator "typoena")
	(generator_version "10.0")
	(general
		(thickness 1.6)
		(legacy_teardrops no)
	)
	(paper "A3")
	(title_block
		(title "Typoena mainboard")
		(date "2026-08-16")
		(rev "A")
		(company "Typoena")
		(comment 1 "130 x 45 mm, 4 couches - squelette, empreintes a importer depuis le schema")
	)
	(layers
		(0 "F.Cu" signal)
		(4 "In1.Cu" signal)
		(6 "In2.Cu" signal)
		(2 "B.Cu" signal)
		(9 "F.Adhes" user "F.Adhesive")
		(11 "B.Adhes" user "B.Adhesive")
		(13 "F.Paste" user)
		(15 "B.Paste" user)
		(5 "F.SilkS" user "F.Silkscreen")
		(7 "B.SilkS" user "B.Silkscreen")
		(1 "F.Mask" user)
		(3 "B.Mask" user)
		(17 "Dwgs.User" user "User.Drawings")
		(19 "Cmts.User" user "User.Comments")
		(21 "Eco1.User" user "User.Eco1")
		(23 "Eco2.User" user "User.Eco2")
		(25 "Edge.Cuts" user)
		(27 "Margin" user)
		(31 "F.CrtYd" user "F.Courtyard")
		(29 "B.CrtYd" user "B.Courtyard")
		(35 "F.Fab" user)
		(33 "B.Fab" user)
		(39 "User.1" user)
		(41 "User.2" user)
		(43 "User.3" user)
		(45 "User.4" user)
	)
	(setup
		(stackup
			(layer "F.SilkS" (type "Top Silk Screen"))
			(layer "F.Paste" (type "Top Solder Paste"))
			(layer "F.Mask" (type "Top Solder Mask") (thickness 0.01))
			(layer "F.Cu" (type "copper") (thickness 0.035))
			(layer "dielectric 1" (type "prepreg") (thickness 0.2104) (material "FR4") (epsilon_r 4.29) (loss_tangent 0.02))
			(layer "In1.Cu" (type "copper") (thickness 0.0152))
			(layer "dielectric 2" (type "core") (thickness 1.065) (material "FR4") (epsilon_r 4.29) (loss_tangent 0.02))
			(layer "In2.Cu" (type "copper") (thickness 0.0152))
			(layer "dielectric 3" (type "prepreg") (thickness 0.2104) (material "FR4") (epsilon_r 4.29) (loss_tangent 0.02))
			(layer "B.Cu" (type "copper") (thickness 0.035))
			(layer "B.Mask" (type "Bottom Solder Mask") (thickness 0.01))
			(layer "B.Paste" (type "Bottom Solder Paste"))
			(layer "B.SilkS" (type "Bottom Silk Screen"))
			(copper_finish "ENIG")
			(dielectric_constraints no)
		)
		(pad_to_mask_clearance 0)
		(allow_soldermask_bridges_in_footprints no)
		(tenting front back)
	)
	(net 0 "")
"""


def main():
    body = [HEADER]

    # contour : rectangle a coins arrondis
    x1, y1, x2, y2 = X0, Y0, X0 + W, Y0 + H
    body += [
        edge(x1 + R, y1, x2 - R, y1),
        edge(x2, y1 + R, x2, y2 - R),
        edge(x2 - R, y2, x1 + R, y2),
        edge(x1, y2 - R, x1, y1 + R),
    ]
    k = R * (1 - 0.70710678)   # point median de l'arc a 45 deg
    body += [
        arc(x1 + R, y1, x1 + k, y1 + k, x1, y1 + R),
        arc(x2 - R, y1, x2 - k, y1 + k, x2, y1 + R),
        arc(x2, y2 - R, x2 - k, y2 - k, x2 - R, y2),
        arc(x1, y2 - R, x1 + k, y2 - k, x1 + R, y2),
    ]

    # 6 fixations : les 4 coins plus 2 a mi-portee, une carte de 130 mm flechit
    xs = [x1 + HOLE_INSET, x1 + W / 2, x2 - HOLE_INSET]
    ys = [y1 + HOLE_INSET, y2 - HOLE_INSET]
    n = 1
    for yy in ys:
        for xx in xs:
            body.append(mounting_hole(xx, yy, n))
            n += 1

    body.append(")\n")
    open(OUT, "w").write("".join(body))
    print(f"ecrit: {OUT}")
    print(f"  contour {W} x {H} mm, coins R{R}, {n - 1} trous de fixation Ø2,2")


if __name__ == "__main__":
    main()
