#!/usr/bin/env python3
"""Petite bibliothèque d'émission de schémas KiCad 10.

Connectivité par étiquettes : chaque broche reçoit un moignon de fil de 2,54 mm
terminé par un label. C'est électriquement équivalent à des fils tracés, et ça évite
tout routage géométrique — le schéma se réorganise ensuite dans le GUI.
"""
import re
import uuid as _uuid

SYMDIR = "/usr/share/kicad/symbols"


def uid():
    return str(_uuid.uuid4())


# ---------------------------------------------------------------- lecture des libs
def _balanced(text, i):
    d = 0
    for j in range(i, len(text)):
        if text[j] == "(":
            d += 1
        elif text[j] == ")":
            d -= 1
            if d == 0:
                return text[i : j + 1]
    raise ValueError("parenthèses non équilibrées")


class SymbolLib:
    def __init__(self):
        self._cache = {}   # lib_id -> (bloc texte, {num: (x, y, angle)})
        self._files = {}

    def _load(self, lib):
        if lib not in self._files:
            path = f"{SYMDIR}/{lib}.kicad_sym"
            if lib == "typoena":
                path = ("/home/emmanuel/Documents/Developpement/esp32/typewriter/"
                        "hardware/pcb/mainboard/typoena.kicad_sym")
            self._files[lib] = open(path).read()
        return self._files[lib]

    def _raw(self, lib, name):
        text = self._load(lib)
        i = text.find(f'(symbol "{name}"')
        if i < 0:
            raise SystemExit(f"symbole introuvable: {lib}:{name}")
        return _balanced(text, i)

    def get(self, lib_id):
        if lib_id in self._cache:
            return self._cache[lib_id]
        lib, name = lib_id.split(":", 1)
        blk = self._raw(lib, name)

        # Résolution de (extends "PARENT") : KiCad n'accepte pas d'héritage dans les
        # lib_symbols embarqués, il faut aplatir. On garde les propriétés de l'enfant
        # et on reprend le dessin + les broches du parent.
        ext = re.search(r'\(extends "([^"]+)"\)', blk)
        if ext:
            parent_name = ext.group(1)
            parent = self._raw(lib, parent_name)
            sub = []
            for m in re.finditer(r'\(symbol "' + re.escape(parent_name) + r'_\d+_\d+"', parent):
                sub.append(_balanced(parent, m.start()))
            if not sub:
                raise SystemExit(f"{lib_id}: parent {parent_name} sans sous-symbole")
            body = "\n".join(
                s.replace(f'(symbol "{parent_name}_', f'(symbol "{name}_') for s in sub
            )
            # KiCad hérite aussi les attributs de tête que l'enfant ne redéclare pas.
            # Les omettre produit un avertissement ERC « symbole différent de la copie
            # en librairie » sur chaque instance.
            inherited = []
            for tok in ("pin_numbers", "pin_names", "exclude_from_sim", "in_bom",
                        "on_board"):
                if re.search(r"\(" + tok + r"[\s)]", blk):
                    continue
                m2 = re.search(r"\(" + tok + r"[\s)]", parent)
                if m2:
                    inherited.append("\t\t" + _balanced(parent, m2.start()))
            blk = blk.replace(ext.group(0), "").rstrip()
            assert blk.endswith(")")
            head_end = blk.index("\n")
            blk = (blk[:head_end] + "\n" + "\n".join(inherited) + blk[head_end:])
            blk = blk[:-1] + "\n" + body + "\n\t)"

        pins = {}
        for m in re.finditer(r"\(pin ", blk):
            pb = _balanced(blk, m.start())
            at = re.search(r"\(at ([-\d.]+) ([-\d.]+) ([-\d.]+)\)", pb)
            num = re.search(r'\(number "([^"]*)"', pb)
            if at and num:
                pins[num.group(1)] = (float(at.group(1)), float(at.group(2)), float(at.group(3)))
        if not pins:
            raise SystemExit(f"aucune broche dans {lib_id}")
        # Embarqué dans un schéma, seul le symbole racine porte le préfixe de
        # bibliothèque : les sous-symboles gardent leur nom nu (`R_0_1`, pas
        # `Device:R_0_1`). Les préfixer fait échouer le chargement du schéma.
        embedded = blk.replace(f'(symbol "{name}"', f'(symbol "{lib_id}"', 1)
        self._cache[lib_id] = (embedded, pins)
        return self._cache[lib_id]


LIB = SymbolLib()


# ---------------------------------------------------------------- éléments
def _eff(size=1.27, justify=None, hide=False):
    j = f"\n\t\t\t\t(justify {justify})" if justify else ""
    h = "\n\t\t\t(hide yes)" if hide else ""
    return f"""(effects
				(font
					(size {size} {size})
				){j}
			){h}"""


def wire(x1, y1, x2, y2):
    return f"""	(wire
		(pts
			(xy {x1} {y1}) (xy {x2} {y2})
		)
		(stroke
			(width 0)
			(type default)
		)
		(uuid "{uid()}")
	)
"""


def label(name, x, y, angle, glob=False):
    just = "left" if angle in (0, 90) else "right"
    if glob:
        return f"""	(global_label "{name}"
		(shape bidirectional)
		(at {x} {y} {angle})
		(fields_autoplaced yes)
		(effects
			(font
				(size 1.27 1.27)
			)
			(justify {just})
		)
		(uuid "{uid()}")
	)
"""
    return f"""	(label "{name}"
		(at {x} {y} {angle})
		(effects
			(font
				(size 1.27 1.27)
			)
			(justify {just} bottom)
		)
		(uuid "{uid()}")
	)
"""


def text(s, x, y, size=3.0):
    return f"""	(text "{s}"
		(exclude_from_sim no)
		(at {x} {y} 0)
		(effects
			(font
				(size {size} {size})
				(bold yes)
			)
			(justify left bottom)
		)
		(uuid "{uid()}")
	)
"""


def symbol_instance(lib_id, ref, value, footprint, x, y, pins, project, inst_path,
                    extra_props=None, dnp=False):
    _, pin_pos = LIB.get(lib_id)
    props = [
        ("Reference", ref, False),
        ("Value", value, False),
        ("Footprint", footprint, True),
        ("Datasheet", "", True),
        ("Description", "", True),
    ]
    for k, v in (extra_props or {}).items():
        props.append((k, v, True))
    out = [f"""	(symbol
		(lib_id "{lib_id}")
		(at {x} {y} 0)
		(unit 1)
		(exclude_from_sim no)
		(in_bom yes)
		(on_board yes)
		(dnp {"yes" if dnp else "no"})
		(fields_autoplaced yes)
		(uuid "{uid()}")
"""]
    dy = 0
    for name, val, hide in props:
        val = val.replace('"', "'")
        out.append(f"""		(property "{name}" "{val}"
			(at {x} {y - 12.7 - dy} 0)
			{_eff(hide=hide)}
		)
""")
        dy += 2.54
    for num in sorted(pin_pos, key=lambda n: (len(n), n)):
        out.append(f"""		(pin "{num}"
			(uuid "{uid()}")
		)
""")
    out.append(f"""		(instances
			(project "{project}"
				(path "{inst_path}"
					(reference "{ref}")
					(unit 1)
				)
			)
		)
	)
""")
    return "".join(out)


def pin_abs(lib_id, x, y, num):
    """Position absolue du point de connexion d'une broche, et direction du moignon."""
    _, pins = LIB.get(lib_id)
    if num not in pins:
        raise SystemExit(f"{lib_id}: broche {num} inexistante (dispo: {sorted(pins)})")
    px, py, ang = pins[num]
    ax, ay = x + px, y - py
    # direction vers le corps du symbole en coordonnées schéma (Y inversé)
    import math
    bx, by = math.cos(math.radians(ang)), -math.sin(math.radians(ang))
    return ax, ay, (-bx, -by)   # le moignon part à l'opposé du corps


HEADER = """(kicad_sch
	(version 20250114)
	(generator "typoena")
	(generator_version "10.0")
	(uuid "{root}")
	(paper "{paper}")
	(title_block
		(title "{title}")
		(date "{date}")
		(rev "{rev}")
		(company "Typoena")
		(comment 1 "{c1}")
		(comment 2 "{c2}")
	)
"""


def sheet(name, filename, x, y, w, h, sheet_uuid, project, root_uuid, page):
    """Symbole de feuille hiérarchique dans la racine.

    Pas de broches de feuille : la connectivité inter-feuilles passe par les
    étiquettes globales, qui traversent toute la hiérarchie.
    """
    return f"""	(sheet
		(at {x} {y})
		(size {w} {h})
		(fields_autoplaced yes)
		(stroke
			(width 0.1524)
			(type solid)
		)
		(fill
			(color 0 0 0 0.0000)
		)
		(uuid "{sheet_uuid}")
		(property "Sheetname" "{name}"
			(at {x} {y - 1.27} 0)
			(effects
				(font
					(size 2.54 2.54)
				)
				(justify left bottom)
			)
		)
		(property "Sheetfile" "{filename}"
			(at {x} {y + h + 2.54} 0)
			(effects
				(font
					(size 1.27 1.27)
				)
				(justify left top)
			)
		)
		(instances
			(project "{project}"
				(path "/{root_uuid}"
					(page "{page}")
				)
			)
		)
	)
"""
