#!/usr/bin/env python3
"""Génère hardware/pcb/mainboard-v2/typoena.kicad_sym.

- BQ25896RTW : dérivé du BQ25895RTW stock (même WQFN-24-EP 4x4), avec les trois
  broches qui diffèrent renommées/retypées (2 D+ -> PSEL, 3 D- -> /PG, 24 DSEL -> NC).
- TPS61023DRL : dessiné à partir de la datasheet TI (§ Pin Configuration).
  Le buck-boost 3V3 utilise le symbole ET l'empreinte stock du TPS63001.
"""
import re

STOCK = "/usr/share/kicad/symbols/Battery_Management.kicad_sym"
OUT = "/home/emmanuel/Documents/Developpement/esp32/typewriter/hardware/pcb/mainboard-v2/typoena.kicad_sym"


def grab(text, start_marker):
    i = text.find(start_marker)
    if i < 0:
        raise SystemExit(f"introuvable: {start_marker}")
    d = 0
    for j in range(i, len(text)):
        if text[j] == "(":
            d += 1
        elif text[j] == ")":
            d -= 1
            if d == 0:
                return text[i : j + 1]
    raise SystemExit("parenthèses non équilibrées")


def pin_blocks(blk):
    """Itère sur les blocs `(pin ...)` équilibrés, en rendant (début, fin, texte)."""
    for m in re.finditer(r"\(pin ", blk):
        i = m.start()
        d = 0
        for j in range(i, len(blk)):
            if blk[j] == "(":
                d += 1
            elif blk[j] == ")":
                d -= 1
                if d == 0:
                    yield i, j + 1, blk[i : j + 1]
                    break


def retype_pin(blk, number, new_name, new_type):
    """Renomme et retype la broche `number`.

    Le découpage se fait par équilibrage de parenthèses, pas par regex : un `.*?`
    en DOTALL traverse allègrement les broches voisines et renomme la mauvaise.
    """
    target = f'(number "{number}"'
    hits = [(i, j, t) for i, j, t in pin_blocks(blk) if target in t]
    if len(hits) != 1:
        raise SystemExit(f"broche {number}: {len(hits)} correspondance(s), attendu 1")
    i, j, txt = hits[0]
    m = re.match(r'\(pin (\w+) (\w+)', txt)
    old_name = re.search(r'\(name "([^"]*)"', txt)
    if not m or not old_name:
        raise SystemExit(f"broche {number}: bloc illisible")
    new_txt = txt.replace(f"(pin {m.group(1)} {m.group(2)}", f"(pin {new_type} {m.group(2)}", 1)
    new_txt = new_txt.replace(f'(name "{old_name.group(1)}"', f'(name "{new_name}"', 1)
    print(f"  broche {number}: {old_name.group(1)} ({m.group(1)}) -> {new_name} ({new_type})")
    return blk[:i] + new_txt + blk[j:]


def set_prop(blk, name, value):
    pat = re.compile(r'\(property "' + re.escape(name) + r'" "(?:[^"\\]|\\.)*"')
    if not pat.search(blk):
        raise SystemExit(f"propriété {name} introuvable")
    return pat.sub(f'(property "{name}" "{value}"', blk, count=1)


# ---------------------------------------------------------------- BQ25896
stock = open(STOCK).read()
bq = grab(stock, '(symbol "BQ25895RTW"')

bq = retype_pin(bq, "2", "PSEL", "input")
bq = retype_pin(bq, "3", "~{PG}", "open_collector")
bq = retype_pin(bq, "24", "NC", "no_connect")

bq = set_prop(bq, "Value", "BQ25896RTW")
bq = set_prop(bq, "Datasheet", "https://www.ti.com/lit/ds/symlink/bq25896.pdf")
bq = set_prop(
    bq,
    "Description",
    "I2C Controlled Single Cell Li-Ion 3A Fast Charger with NVDC Power Path, "
    "ADC, OTG boost and ship mode. VBUS 3.9..14V. WQFN-24-EP 4x4mm. "
    "Derive du symbole stock BQ25895RTW: broches 2/3/24 differentes (PSEL, /PG, NC).",
)
bq = set_prop(bq, "ki_keywords", "1-cell Battery-Charger Power-Path NVDC I2C ADC ship-mode OTG")
# renomme le symbole et ses sous-unités
bq = bq.replace('(symbol "BQ25895RTW"', '(symbol "BQ25896RTW"', 1)
bq = bq.replace('"BQ25895RTW_0_1"', '"BQ25896RTW_0_1"')
bq = bq.replace('"BQ25895RTW_1_1"', '"BQ25896RTW_1_1"')
# La Description mentionne volontairement le symbole d'origine : on ne contrôle que
# les identifiants structurels.
assert '(symbol "BQ25895' not in bq, "sous-symbole BQ25895 non renommé"
assert '(property "Value" "BQ25895' not in bq, "Value non renommée"
assert bq.count('(symbol "BQ25896RTW') == 3, "les 3 identifiants BQ25896 attendus"

# Contrôle du brochage complet contre la datasheet SLUSC76C §6 (Pin Functions).
EXPECTED = {
    "1": "VBUS", "2": "PSEL", "3": "~{PG}", "4": "STAT", "5": "SCL", "6": "SDA",
    "7": "~{INT}", "8": "OTG", "9": "~{CE}", "10": "ILIM", "11": "TS", "12": "~{QON}",
    "13": "BAT", "14": "BAT", "15": "SYS", "16": "SYS", "17": "PGND", "18": "PGND",
    "19": "SW", "20": "SW", "21": "BTST", "22": "REGN", "23": "PMID", "24": "NC",
    "25": "PGND",
}
got = {}
for _i, _j, t in pin_blocks(bq):
    n = re.search(r'\(number "([^"]*)"', t).group(1)
    got[n] = re.search(r'\(name "([^"]*)"', t).group(1)
bad = {k: (v, got.get(k)) for k, v in EXPECTED.items() if got.get(k) != v}
if bad or set(got) != set(EXPECTED):
    raise SystemExit(f"brochage BQ25896 incorrect: {bad} / extra={set(got)-set(EXPECTED)}")
print("  brochage BQ25896 conforme à la datasheet (25 broches)")

# ---------------------------------------------------------------- helpers de dessin
def pin(kind, x, y, angle, name, number, length=2.54):
    return f"""			(pin {kind} line
				(at {x} {y} {angle})
				(length {length})
				(name "{name}"
					(effects
						(font
							(size 1.27 1.27)
						)
					)
				)
				(number "{number}"
					(effects
						(font
							(size 1.27 1.27)
						)
					)
				)
			)
"""


def box_symbol(name, value, footprint, datasheet, description, keywords, fp_filters,
               x0, y0, x1, y1, pins, ref_y, val_y):
    body = "".join(pins)
    return f"""	(symbol "{name}"
		(exclude_from_sim no)
		(in_bom yes)
		(on_board yes)
		(in_pos_files yes)
		(duplicate_pin_numbers_are_jumpers no)
		(property "Reference" "U"
			(at {x0} {ref_y} 0)
			(show_name no)
			(do_not_autoplace no)
			(effects
				(font
					(size 1.27 1.27)
				)
				(justify left bottom)
			)
		)
		(property "Value" "{value}"
			(at {x0} {val_y} 0)
			(show_name no)
			(do_not_autoplace no)
			(effects
				(font
					(size 1.27 1.27)
				)
				(justify left bottom)
			)
		)
		(property "Footprint" "{footprint}"
			(at 0 {y1 - 5.08} 0)
			(show_name no)
			(do_not_autoplace no)
			(hide yes)
			(effects
				(font
					(size 1.27 1.27)
				)
			)
		)
		(property "Datasheet" "{datasheet}"
			(at 0 {y1 - 7.62} 0)
			(show_name no)
			(do_not_autoplace no)
			(hide yes)
			(effects
				(font
					(size 1.27 1.27)
				)
			)
		)
		(property "Description" "{description}"
			(at 0 {y1 - 10.16} 0)
			(show_name no)
			(do_not_autoplace no)
			(hide yes)
			(effects
				(font
					(size 1.27 1.27)
				)
			)
		)
		(property "ki_keywords" "{keywords}"
			(at 0 0 0)
			(show_name no)
			(do_not_autoplace no)
			(hide yes)
			(effects
				(font
					(size 1.27 1.27)
				)
			)
		)
		(property "ki_fp_filters" "{fp_filters}"
			(at 0 0 0)
			(show_name no)
			(do_not_autoplace no)
			(hide yes)
			(effects
				(font
					(size 1.27 1.27)
				)
			)
		)
		(symbol "{name}_0_1"
			(rectangle
				(start {x0} {y0})
				(end {x1} {y1})
				(stroke
					(width 0.254)
					(type default)
				)
				(fill
					(type background)
				)
			)
		)
		(symbol "{name}_1_1"
{body}		)
		(embedded_fonts no)
	)
"""


# ---------------------------------------------------------------- TPS61023
# Datasheet SLVSF14B §5 : 1 FB, 2 EN, 3 VIN, 4 GND, 5 SW, 6 VOUT
tps61023 = box_symbol(
    "TPS61023DRL",
    "TPS61023DRL",
    "Package_TO_SOT_SMD:SOT-563",
    "https://www.ti.com/lit/ds/symlink/tps61023.pdf",
    "Boost 3.7A switch, VIN 0.5..5.5V, VOUT up to 5.5V, SOT-563 (DRL) 6-pin",
    "boost regulator dc-dc step-up",
    "*SOT?563*",
    -7.62, -7.62, 7.62, 7.62,
    [
        pin("power_in", -10.16, 5.08, 0, "VIN", "3"),
        pin("input", -10.16, 0, 0, "EN", "2"),
        pin("input", -10.16, -5.08, 0, "FB", "1"),
        pin("power_out", 10.16, 5.08, 180, "VOUT", "6"),
        pin("passive", 10.16, 0, 180, "SW", "5"),
        pin("power_in", 0, -10.16, 90, "GND", "4"),
    ],
    ref_y=8.89, val_y=11.43,
)

# ---------------------------------------------------------------- écriture
header = """(kicad_symbol_lib
	(version 20241209)
	(generator "typoena")
	(generator_version "10.0")
"""
with open(OUT, "w") as f:
    f.write(header)
    f.write("\t" + bq.replace("\n\t\t", "\n\t\t") + "\n")
    f.write(tps61023)
    f.write(")\n")
print("écrit:", OUT)
