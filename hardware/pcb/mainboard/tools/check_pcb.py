#!/usr/bin/env python3
"""Contrôle qualité de la mainboard : tout ce qui se vérifie par le calcul.

LECTURE SEULE. Ce script n'ouvre aucun fichier du projet en écriture, et déduit
son chemin de `__file__` — jamais en dur. C'est délibéré : `gen_sch.py` écrit en
chemins absolus et a déjà réécrit le vrai schéma pendant un essai censé se
dérouler dans un bac à sable.

Usage :
    python3 tools/check_pcb.py [--profil routage|fabrication] [-v]

Le profil `routage` (défaut) tolère ce qui est normal en cours de travail :
connexions restantes, sérigraphie, pistes en l'air. Le profil `fabrication`
n'en tolère aucune.
"""
import argparse
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
PROJ = os.path.dirname(HERE)
PCB = os.path.join(PROJ, "typoena-mainboard.kicad_pcb")
SCH = os.path.join(PROJ, "typoena-mainboard.kicad_sch")
DRU = os.path.join(PROJ, "typoena-mainboard.kicad_dru")
PRO = os.path.join(PROJ, "typoena-mainboard.kicad_pro")


# ---------------------------------------------------------------- lecture s-exp

def sexp(text, i=0):
    """Lit une s-expression. Contrairement à kisch._balanced(), saute les chaînes.

    Nécessaire pour *lire* un .kicad_pcb quelconque : un guillemet peut contenir
    des parenthèses, et un compteur naïf se désynchronise dessus.
    """
    n = len(text)
    while i < n and text[i] != "(":
        i += 1
    if i >= n:
        return None, n
    out, i = [], i + 1
    while i < n:
        c = text[i]
        if c == "(":
            node, i = sexp(text, i)
            out.append(node)
        elif c == ")":
            return out, i + 1
        elif c == '"':
            j, buf = i + 1, []
            while text[j] != '"':
                if text[j] == "\\":
                    j += 1
                buf.append(text[j])
                j += 1
            out.append("".join(buf))
            i = j + 1
        elif c.isspace():
            i += 1
        else:
            j = i
            while j < n and not text[j].isspace() and text[j] not in '()"':
                j += 1
            out.append(text[i:j])
            i = j
    return out, i


def kids(node, name):
    return [c for c in node if isinstance(c, list) and c and c[0] == name]


def kid(node, name):
    r = kids(node, name)
    return r[0] if r else None


def num(v, default=0.0):
    try:
        return float(v)
    except (TypeError, ValueError):
        return default


# ------------------------------------------------------------------- géométrie

def pad_abs(fx, fy, frot_deg, px, py):
    """Position absolue d'une pastille. Le Y de KiCad descend, d'où le signe.

    Cette transformation est le point où l'on se trompe : la faire à l'envers
    produit des positions plausibles mais fausses, donc des collisions et des
    distances imaginaires. `autotest()` la valide contre la carte avant tout.
    """
    r = -math.radians(frot_deg)
    c, s = math.cos(r), math.sin(r)
    return fx + px * c - py * s, fy + px * s + py * c


def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def point_to_rect(p, cx, cy, hw, hh, rot_deg):
    """Distance d'un point au bord d'un rectangle orienté. Négative si dedans."""
    r = -math.radians(rot_deg)
    dx, dy = p[0] - cx, p[1] - cy
    lx = dx * math.cos(-r) - dy * math.sin(-r)
    ly = dx * math.sin(-r) + dy * math.cos(-r)
    ox, oy = abs(lx) - hw, abs(ly) - hh
    if ox < 0 and oy < 0:
        return max(ox, oy)
    return math.hypot(max(ox, 0), max(oy, 0))


def rect_corners(cx, cy, hw, hh, rot_deg):
    r = -math.radians(rot_deg)
    c, sn = math.cos(r), math.sin(r)
    return [(cx + dx * c - dy * sn, cy + dx * sn + dy * c)
            for dx, dy in ((-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh))]


def _seg_seg(p1, p2, p3, p4):
    def pt_seg(p, a, b):
        vx, vy = b[0] - a[0], b[1] - a[1]
        L2 = vx * vx + vy * vy
        t = 0.0 if L2 == 0 else max(0.0, min(1.0, ((p[0]-a[0])*vx + (p[1]-a[1])*vy) / L2))
        return math.hypot(p[0]-a[0]-t*vx, p[1]-a[1]-t*vy)
    d1 = (p2[0]-p1[0], p2[1]-p1[1]); d2 = (p4[0]-p3[0], p4[1]-p3[1])
    den = d1[0]*d2[1] - d1[1]*d2[0]
    if abs(den) > 1e-12:
        t = ((p3[0]-p1[0])*d2[1] - (p3[1]-p1[1])*d2[0]) / den
        u = ((p3[0]-p1[0])*d1[1] - (p3[1]-p1[1])*d1[0]) / den
        if 0 <= t <= 1 and 0 <= u <= 1:
            return 0.0
    return min(pt_seg(p1, p3, p4), pt_seg(p2, p3, p4),
               pt_seg(p3, p1, p2), pt_seg(p4, p1, p2))


def rect_dist(a, b):
    """Écart bord à bord entre deux rectangles orientés. 0 s'ils se touchent.

    L'approximation par un rayon circonscrit sur l'un des deux fabrique des
    chevauchements qui n'existent pas — ce qui m'a fait annoncer deux collisions
    imaginaires avant que ce script existe.
    """
    A = rect_corners(*a)
    B = rect_corners(*b)
    return min(_seg_seg(A[i], A[(i+1) % 4], B[j], B[(j+1) % 4])
               for i in range(4) for j in range(4))


# --------------------------------------------------------------- modèle carte

class Pad:
    __slots__ = ("ref", "num", "net", "kind", "shape", "x", "y", "hw", "hh",
                 "rot", "drill", "layers")

    def is_smd(self):
        return self.kind == "smd"

    def is_hole(self):
        return self.drill > 0


class Board:
    def __init__(self, path):
        self.text = open(path, encoding="utf-8").read()
        root, _ = sexp(self.text)
        self.root = root
        self.pads, self.footprints = [], {}
        for fp in kids(root, "footprint"):
            ref = None
            for p in kids(fp, "property"):
                if len(p) > 2 and p[1] == "Reference":
                    ref = p[2]
            if not ref:
                continue
            at = kid(fp, "at")
            fx, fy = num(at[1]), num(at[2])
            frot = num(at[3]) if len(at) > 3 else 0.0
            val = ""
            for pr in kids(fp, "property"):
                if len(pr) > 2 and pr[1] == "Value":
                    val = pr[2]
            self.footprints[ref] = {"x": fx, "y": fy, "rot": frot,
                                    "lib": fp[1] if len(fp) > 1 else "",
                                    "value": val, "pads": []}
            for pd in kids(fp, "pad"):
                pat = kid(pd, "at")
                px, py = num(pat[1]), num(pat[2])
                prot = num(pat[3]) if len(pat) > 3 else 0.0
                sz = kid(pd, "size")
                w, h = num(sz[1]), num(sz[2])
                dr = kid(pd, "drill")
                nt = kid(pd, "net")
                ly = kid(pd, "layers")
                p = Pad()
                p.ref, p.num = ref, pd[1]
                p.kind, p.shape = pd[2], pd[3]
                p.x, p.y = pad_abs(fx, fy, frot, px, py)
                # Cotes telles qu'écrites, et angle ABSOLU : dans le fichier
                # carte l'angle de pastille n'est pas relatif à l'empreinte
                # (J4 : empreinte 90°, pastille 90°, et la pastille est bien
                # étroite en y). On laisse point_to_rect faire tourner le point
                # plutôt que d'échanger les côtés — échanger *et* tourner
                # revenait à appliquer la rotation deux fois.
                p.hw, p.hh = w / 2, h / 2
                p.rot = prot
                p.drill = num(dr[1]) if dr and len(dr) > 1 else 0.0
                p.net = nt[1] if nt and len(nt) > 1 else ""
                p.layers = [str(x) for x in ly[1:]] if ly else []
                self.pads.append(p)
                self.footprints[ref]["pads"].append(p)

        self.segments = []
        for s in kids(root, "segment"):
            st, en = kid(s, "start"), kid(s, "end")
            nt = kid(s, "net")
            self.segments.append({
                "a": (num(st[1]), num(st[2])), "b": (num(en[1]), num(en[2])),
                "w": num(kid(s, "width")[1]), "layer": kid(s, "layer")[1],
                "net": nt[1] if nt and len(nt) > 1 else "",
            })
        self.vias = []
        for v in kids(root, "via"):
            at = kid(v, "at")
            nt = kid(v, "net")
            self.vias.append({
                "p": (num(at[1]), num(at[2])),
                "size": num(kid(v, "size")[1]), "drill": num(kid(v, "drill")[1]),
                "net": nt[1] if nt and len(nt) > 1 else "",
            })
        self.zones = []
        for z in kids(root, "zone"):
            nm, nt = kid(z, "name"), kid(z, "net")
            lay = kid(z, "layers") or kid(z, "layer")
            fill = kid(z, "fill")
            pr = kid(z, "priority")
            self.zones.append({
                "name": nm[1] if nm and len(nm) > 1 else "(sans nom)",
                "net": nt[1] if nt and len(nt) > 1 else "",
                "layers": [str(x) for x in lay[1:]] if lay else [],
                "filled": bool(fill and "yes" in [str(x) for x in fill[:2]]),
                "islands": len(kids(z, "filled_polygon")),
                "priority": int(num(pr[1])) if pr else 0,
                "keepout": bool(kid(z, "keepout")),
            })

    def net_pads(self, net):
        return [p for p in self.pads if p.net == net]

    def net_segments(self, net):
        return [s for s in self.segments if s["net"] == net]

    def net_vias(self, net):
        return [v for v in self.vias if v["net"] == net]

    def perimeter(self, ref):
        """Broches périphériques : on écarte le pavé thermique et ses sous-formes,
        qui fausseraient le pas et n'ont pas de numéro exploitable."""
        return [p for p in self.footprints[ref]["pads"]
                if p.is_smd() and p.num and min(p.hw, p.hh) * 2 < 0.8]

    def pitch(self, ref):
        """Pas le plus serré entre broches périphériques de nets différents."""
        ps = self.perimeter(ref)
        best = 99.0
        for i, a in enumerate(ps):
            for b in ps[i + 1:]:
                if a.net and a.net == b.net:
                    continue
                best = min(best, dist((a.x, a.y), (b.x, b.y)))
        return best


# ------------------------------------------------------------------- autotest

def autotest(bd):
    """Valide `pad_abs` contre la carte : des extrémités de piste doivent
    coïncider avec des centres de pastille. Sans cet accord, tout le reste est
    du bruit — et on préfère ne rien dire que dire faux."""
    hits = 0
    for s in bd.segments:
        for end in (s["a"], s["b"]):
            for p in bd.pads:
                if p.net and p.net == s["net"] and dist(end, (p.x, p.y)) < 0.01:
                    hits += 1
                    break

    # Second garde-fou, sur l'orientation des boîtes : sur un composant à deux
    # bornes, le grand axe d'une pastille est perpendiculaire à la droite qui
    # joint les deux. Une règle d'échange fausse le trahit immédiatement.
    bad = []
    for ref, fp in bd.footprints.items():
        ps = [q for q in fp["pads"] if q.is_smd() and q.num]
        if len(ps) != 2 or abs(ps[0].hw - ps[0].hh) < 0.05:
            continue
        vx, vy = ps[1].x - ps[0].x, ps[1].y - ps[0].y
        cs = rect_corners(ps[0].x, ps[0].y, ps[0].hw, ps[0].hh, ps[0].rot)
        ex = max(c[0] for c in cs) - min(c[0] for c in cs)
        ey = max(c[1] for c in cs) - min(c[1] for c in cs)
        if abs(ex - ey) < 0.05:
            continue
        if (abs(vx) > abs(vy)) == (ex > ey):
            bad.append(ref)
    return hits, bad


# ------------------------------------------------------------- connectivité F.Cu

class Union:
    def __init__(self):
        self.p = {}

    def find(self, a):
        self.p.setdefault(a, a)
        while self.p[a] != a:
            self.p[a] = self.p[self.p[a]]
            a = self.p[a]
        return a

    def join(self, a, b):
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            self.p[ra] = rb


def fcu_graph(bd, net):
    """Connexions du net sur F.Cu seule — sans vias ni zones.

    Sert à distinguer « la broche et son condensateur se touchent » de « tous
    deux descendent séparément dans le plan », qui n'est pas la même chose pour
    une boucle de découplage.
    """
    u = Union()
    segs = [s for s in bd.net_segments(net) if s["layer"] == "F.Cu"]
    for i, s in enumerate(segs):
        u.join(("s", i, 0), ("s", i, 1))
    for i, s in enumerate(segs):
        for j, t in enumerate(segs[i + 1:], i + 1):
            for ea, a in enumerate((s["a"], s["b"])):
                for eb, b in enumerate((t["a"], t["b"])):
                    if dist(a, b) < 0.01:
                        u.join(("s", i, ea), ("s", j, eb))
    for p in bd.net_pads(net):
        if "F.Cu" not in p.layers and "*.Cu" not in p.layers:
            continue
        key = ("p", p.ref, p.num)
        for i, s in enumerate(segs):
            for ea, e in enumerate((s["a"], s["b"])):
                if point_to_rect(e, p.x, p.y, p.hw, p.hh, p.rot) <= s["w"] / 2 + 0.01:
                    u.join(key, ("s", i, ea))
    return u


# ------------------------------------------------------------------ kicad-cli

def run_cli(args, outdir):
    exe = shutil.which("kicad-cli")
    if not exe:
        return None, "kicad-cli introuvable dans le PATH"
    try:
        r = subprocess.run([exe] + args, capture_output=True, text=True, timeout=600)
        return r, None
    except subprocess.TimeoutExpired:
        return None, "kicad-cli a dépassé le délai"


def parse_drc(path):
    """Rapport DRC -> {catégorie: [bloc texte]}."""
    if not os.path.exists(path):
        return {}
    txt = open(path, encoding="utf-8", errors="replace").read()
    out = defaultdict(list)
    for blk in re.split(r"\n(?=\[)", txt):
        m = re.match(r"\[(\w+)\]", blk)
        if m:
            out[m.group(1)].append(blk)
    return out


# --------------------------------------------------------------------- rapport

PASS, WARN, FAIL, INFO = "PASS", "WARN", "FAIL", "INFO"
_ORDER = {FAIL: 0, WARN: 1, INFO: 2, PASS: 3}


class Report:
    def __init__(self, verbose):
        self.rows = []
        self.verbose = verbose

    def add(self, cid, label, verdict, summary, details=()):
        self.rows.append((cid, label, verdict, summary, list(details)))

    def worst(self, ignore=()):
        v = [r[2] for r in self.rows if r[0] not in ignore]
        return min(v, key=lambda x: _ORDER[x]) if v else PASS

    def show(self):
        colour = {PASS: "\033[32m", WARN: "\033[33m", FAIL: "\033[31m", INFO: "\033[36m"}
        plain = not sys.stdout.isatty()
        for cid, label, verdict, summary, details in self.rows:
            c = "" if plain else colour[verdict]
            z = "" if plain else "\033[0m"
            print(f"  {c}{verdict:<4}{z} {cid:<4} {label:<44} {summary}")
            if details and (self.verbose or verdict in (FAIL, WARN)):
                for d in details[: (200 if self.verbose else 12)]:
                    print(f"            {d}")
                if not self.verbose and len(details) > 12:
                    print(f"            … {len(details) - 12} de plus (-v)")


# ============================================================ tables du projet

# Nœuds hachés des quatre étages. Budgets à ~40 % au-dessus du mesuré : une
# reprise ordinaire passe, une régression qui double un nœud est signalée.
SWITCHING = {
    "PMIC_SW": 8.0, "PMIC_BTST": 5.0,
    "L_3V3_A": 6.0, "L_3V3_B": 6.0,
    "SW_5V": 8.0,
    "EPD_SW": 16.0,      # l'écart L4<->Q4 le dicte, pas le tracé
    "EPD_CPMID": 8.0,
}

# (condensateur, net du rail, boîtier, broche du boîtier, rôle, seuil WARN, seuil FAIL)
LOOPS = [
    ("C2", "PMID", "U2", "PMID", "entrée buck", 5.0, 8.0),
    ("C6", "VSYS", "U2", "VSYS", "sortie", 5.0, 8.0),
    ("C7", "VSYS", "U2", "VSYS", "sortie", 5.0, 8.0),
    ("C5", "VBAT", "U2", "VBAT", "batterie", 5.0, 8.0),
    ("C8", "VSYS", "U3", "VSYS", "entrée", 5.0, 8.0),
    ("C9", "+3V3", "U3", "+3V3", "sortie", 5.0, 8.0),
    ("C30", "+3V3", "U3", "+3V3", "sortie", 5.0, 8.0),
    ("C10", "VSYS", "U4", "VSYS", "entrée", 5.0, 8.0),
    ("C11", "+5V", "U4", "+5V", "sortie", 5.0, 8.0),
    ("C12", "+5V", "U4", "+5V", "sortie", 5.0, 8.0),
    ("C19", "EPD_PREVGL", "D3", "EPD_PREVGL", "redresseur", 8.0, 12.0),
    ("C20", "EPD_PREVGH", "D5", "EPD_PREVGH", "redresseur", 8.0, 12.0),
]

# Découplage : le 100 nF est celui qui décide, les réservoirs tolèrent la distance.
DECOUPLING = [
    ("C15", "+3V3", "U1", "100 nF", 2.5, 5.0),
    ("C29", "+3V3", "U5", "100 nF", 2.5, 5.0),
    ("C16", "+3V3", "U1", "10 µF", 8.0, 14.0),
    ("C17", "+3V3", "U1", "22 µF", 8.0, 14.0),
]

# Pavés thermiques : y loger des vias est l'opération recommandée, pas un défaut.
THERMAL_PADS = {"U1", "U2", "U3", "U5"}

# Empreintes qui n'existent QUE sur le PCB : elles n'ont aucun symbole au schéma,
# donc « Mettre à jour le PCB depuis le schéma » avec « supprimer les empreintes
# en trop » les efface toutes d'un coup, sans rien signaler. C'est arrivé.
# Quatre coins : à 94 mm la carte fléchit assez peu pour s'en contenter, et une
# tête de #6-32 (courtyard Ø7,9) coûte trop de place pour en poser six.
MECHANICAL = {"MountingHole": 4}


# Criticité de chaque condensateur, déclarée à la main. La géométrie ne peut pas
# la deviner : elle ignore qu'un 22 µF sert des salves Wi-Fi, qu'un pont sur EN
# ne fait rien de rapide, ou qu'un rail de panneau est suivi de 100 mm de nappe.
# Ce que la géométrie sait faire, c'est vérifier que cette liste est COMPLÈTE —
# tout condensateur non déclaré ressort en avertissement, pour qu'aucun nouveau
# composant n'échappe au jugement en silence.
#
#   critique  : boucle où le courant s'inverse à chaque cycle, ou découplage HF.
#               La via de masse est le chemin de retour, seuils appliqués.
#   réservoir : courant lent ou nul. Le remplissage de surface suffit.
#   flottant  : aucune pastille de masse, par construction.
CAP_ROLE = {
    # --- chargeur BQ25896
    "C1":  ("critique",  "entrée VBUS, contre U2"),
    "C2":  ("critique",  "entrée PMID du buck — courant haché"),
    "C3":  ("flottant",  "bootstrap, entre BTST et SW"),
    "C4":  ("critique",  "REGN, alimente les drivers de grille"),
    "C5":  ("critique",  "réservoir BAT, contre U2"),
    "C6":  ("critique",  "sortie SYS du buck"),
    "C7":  ("critique",  "sortie SYS du buck"),
    # --- buck-boost 3V3
    "C8":  ("critique",  "entrée VSYS de U3 — courant haché en mode buck"),
    "C9":  ("critique",  "sortie +3V3 de U3"),
    "C30": ("critique",  "sortie +3V3 de U3"),
    # --- boost 5V
    "C10": ("critique",  "entrée VSYS de U4"),
    "C11": ("critique",  "sortie +5V — boucle chaude du boost"),
    "C12": ("critique",  "sortie +5V — boucle chaude du boost"),
    # --- MCU
    "C14": ("réservoir", "RC de démarrage sur EN, rien de rapide"),
    "C15": ("critique",  "100 nF de U1 — les fronts du cœur"),
    "C16": ("critique",  "10 µF de U1 — fournit les salves Wi-Fi"),
    "C17": ("critique",  "22 µF de U1 — idem"),
    # --- pompe de charge du panneau
    "C18": ("flottant",  "condensateur volant, entre SW et CPMID"),
    "C19": ("critique",  "redresseur PREVGL"),
    "C20": ("critique",  "redresseur PREVGH"),
    "C21": ("réservoir", "+3V3 du panneau, 100 mm de nappe derrière"),
    "C22": ("réservoir", "+3V3 du panneau"),
    "C23": ("réservoir", "rail EPD_VDD, en cul-de-sac vers J4"),
    "C24": ("réservoir", "rail EPD_VSH"),
    "C25": ("réservoir", "rail EPD_VSL"),
    "C26": ("réservoir", "rail EPD_VCOM"),
    "C27": ("réservoir", "rail EPD_VDHR"),
    # --- divers
    "C13": ("réservoir", "rail µSD commuté"),
    "C28": ("réservoir", "CH343_V3, LDO interne du pont série"),
    "C29": ("critique",  "100 nF de U5"),
}

# Budgets de courant. HYPOTHÈSES, à corriger après mesure au banc.
#   VBUS/PMID : plafond ILIM en entrée, R1 = 150 Ω -> 2,4 A typ / 2,6 A max (DESIGN-NOTES)
#   VBAT      : PAS le plafond d'entrée — ce nœud ne voit que la charge (~1,5 A visés)
#               ou la décharge (~1,25 A, tout le système sur la cellule). 2,0 A donne
#               33 % de marge et couvre un ICHG relevé par le firmware.
#   VSYS           : alimente les deux convertisseurs
#   +3V3           : MCU + panneau + µSD, les salves Wi-Fi étant servies par C16/C17
#   +5V            : port USB clavier standard
CURRENT = {"VBUS": 2.6, "PMID": 2.6, "VBAT": 2.0, "VSYS": 2.0,
           "+3V3": 1.0, "+5V": 0.5, "+3V3_SD": 0.2, "PMIC_REGN": 0.05}

# Répartition du courant par rail : la source, puis les consommateurs et ce
# qu'ils tirent. Le contrôle en déduit le courant de CHAQUE tronçon par une
# coupe — voir segment_currents(). Sans cette table, comparer le segment le plus
# étroit au budget du net entier accuse à tort tout rail qui se divise : les
# deux pastilles d'un USB-C, les deux bras de VSYS, une dérivation de tirage.
#
# Les valeurs sont des HYPOTHÈSES, à corriger après mesure au banc :
#   VBUS  : plafond ILIM en entrée (R1 = 150 Ω) ; U5 ne tire que son VDD5
#   VBAT  : charge ~1,5 A visés / décharge ~1,25 A -> 2,0 A avec marge
#   VSYS  : U3 alimente le 3V3 (~0,50 A prélevés), L3 le boost 5V (~0,75 A) ;
#           la broche VIN de U4 ne porte que son circuit de commande
#   +5V   : port USB clavier standard ; le pont R10/R11 ne tire rien
FLOW = {
    "VBUS":    ("J6", {"U2": 2.60, "U5": 0.03}),
    "VBAT":    ("J1", {"U2": 2.00}),
    "VSYS":    ("U2", {"U3": 0.50, "L3": 0.75, "U4": 0.02}),
    "PMID":    ("U2", {"C2": 2.60}),
    "+5V":     ("U4", {"J7": 0.50}),
    "+3V3_SD": ("Q1", {"J8": 0.20}),
}

# 10 °C est la valeur prudente par défaut d'IPC-2221, pas une limite physique :
# 20 à 30 °C sur une piste de puissance est banal en conception commerciale, et
# c'est vers 40-50 °C que le vieillissement du stratifié entre en jeu. On garde
# donc 10 °C comme cible et 20 °C comme limite — d'autant que ipc_rise() est
# lui-même pessimiste, pour les raisons expliquées à sa définition.
RISE_TARGET, RISE_FAIL = 10.0, 20.0

# Au-delà, un condensateur n'appartient plus à la boucle du convertisseur dont
# il partage le rail : c'est un réservoir local, que le remplissage suffit à
# servir. Les boucles chaudes mesurées sur cette carte tiennent en 2 à 5 mm.
LOOP_RADIUS = 8.0

NECK_MAX = 3.0        # au-delà, ce n'est plus un col mais un tronçon sous-dimensionné
VIA_PAD_MARGIN = 0.15  # 3x le recalage typique du vernis épargne (±0,05 mm)
GND_VIA_WARN, GND_VIA_FAIL = 1.3, 2.5
MASK_DAM = 0.4         # bandeau de vernis garanti par le fabricant
DRILL_FLOOR = 0.3      # plancher du procédé courant JLCPCB 4 couches


def ipc_current(width_mm, dT=10.0, thickness_um=35.0):
    """IPC-2221, cuivre externe : I = k·ΔT^0.44·A^0.725, A en mils²."""
    mils = width_mm / 0.0254
    area = mils * (thickness_um / 35.0) * 1.378
    return 0.048 * (dT ** 0.44) * (area ** 0.725)


def ipc_rise(width_mm, amps, thickness_um=35.0):
    """Échauffement estimé, la formule inversée.

    Trois fois conservateur pour cette carte, et il faut le savoir avant de
    juger : IPC-2221 suppose une piste ISOLÉE, en air calme, de longueur
    INFINIE, à l'équilibre. Ici un plan de masse plein est à 0,2 mm en dessous
    (IPC-2152, qui remplace 2221 précisément pour ça, donne couramment 1,5 à
    2 fois la capacité), les tronçons font quelques millimètres et sont bridés
    par le cuivre de leurs deux extrémités, et le courant de pointe n'est pas
    un régime permanent. L'échauffement réel est donc nettement inférieur.
    """
    mils = width_mm / 0.0254
    area = mils * (thickness_um / 35.0) * 1.378
    k = 0.048 * (area ** 0.725)
    if k <= 0 or amps <= 0:
        return 0.0
    return (amps / k) ** (1 / 0.44)


# =================================================================== contrôles

def check_kicad(bd, rep, outdir, profile):
    erc = os.path.join(outdir, "erc.rpt")
    r, err = run_cli(["sch", "erc", "--severity-all", "-o", erc, SCH], outdir)
    if err:
        rep.add("A1", "ERC du schéma", WARN, err)
    else:
        n = len(re.findall(r"^\[", open(erc, encoding="utf-8", errors="replace").read(), re.M))
        rep.add("A1", "ERC du schéma", PASS if n == 0 else FAIL, f"{n} violation(s)")

    drcf = os.path.join(outdir, "drc.rpt")
    r, err = run_cli(["pcb", "drc", "--severity-all", "-o", drcf, PCB], outdir)
    if err:
        rep.add("A2", "DRC", WARN, err)
        return {}
    cat = parse_drc(drcf)

    # A7 — remplissage périmé. Signature : une zone recouvre une piste, écart 0,000.
    # Trois fois dans l'historique du projet, c'était ça et rien d'autre ; le
    # signaler d'abord évite de lire une carte qui n'existe pas.
    stale = [b for b in cat.get("clearance", [])
             if "Zone" in b and re.search(r"réel 0,0000|actual 0\.0000", b)]
    if stale:
        rep.add("A7", "remplissage des zones", FAIL,
                f"{len(stale)} recouvrement(s) zone/piste à 0,000 mm",
                ["remplis les zones (touche B) et relance : les contrôles suivants",
                 "portent sinon sur une géométrie périmée."])
    else:
        rep.add("A7", "remplissage des zones", PASS, "à jour")

    hard = ["clearance", "hole_clearance", "hole_to_hole", "drill_out_of_range",
            "track_width", "annular_width", "shorting_items", "tracks_crossing",
            "courtyards_overlap", "malformed_courtyard", "solder_mask_bridge"]
    tot = sum(len(cat.get(k, [])) for k in hard)
    det = [f"{k} : {len(cat[k])}" for k in hard if cat.get(k)]
    rep.add("A2", "DRC — isolation, perçage, courtyard, vernis",
            PASS if tot == 0 else FAIL, f"{tot} violation(s)", det)

    soft = sum(len(cat.get(k, [])) for k in ("isolated_copper", "copper_sliver"))
    rep.add("A3", "DRC — cuivre isolé, échardes", PASS if soft == 0 else WARN,
            f"{soft} violation(s)")

    # A4 — un frein thermique affamé sur une via *interne à un pavé* est bénin :
    # la via baigne dans le cuivre plein du pavé, le rayon que KiCad compte n'est
    # pas son chemin réel.
    starved = cat.get("starved_thermal", [])
    exempt = [b for b in starved
              if re.search(r"\[GND\] de (%s)\b" % "|".join(THERMAL_PADS), b)]
    real = len(starved) - len(exempt)
    rep.add("A4", "DRC — freins thermiques affamés", PASS if real == 0 else WARN,
            f"{real} réel(s), {len(exempt)} exempté(s) (vias dans un pavé)")

    dang = sum(len(cat.get(k, [])) for k in ("track_dangling", "via_dangling"))
    v = PASS if dang == 0 else (FAIL if profile == "fabrication" else WARN)
    rep.add("A5", "DRC — pistes et vias en l'air", v, f"{dang}")

    unc = len(cat.get("unconnected_items", []))
    v = PASS if unc == 0 else (FAIL if profile == "fabrication" else INFO)
    rep.add("A6a", "connexions restantes", v, f"{unc}")

    silk = sum(len(v2) for k, v2 in cat.items() if k.startswith("silk"))
    v = PASS if silk == 0 else (FAIL if profile == "fabrication" else INFO)
    rep.add("A6b", "sérigraphie", v, f"{silk} (cosmétique, à traiter en dernier)")
    return cat


def check_layers(bd, rep):
    inner = [s for s in bd.segments if s["layer"] == "In1.Cu"]
    rep.add("B1", "aucune piste sur le plan de masse (In1.Cu)",
            PASS if not inner else FAIL, f"{len(inner)} segment(s)",
            [f"{s['net']} {s['a']} -> {s['b']}" for s in inner])

    copper = [z for z in bd.zones if not z["keepout"]]
    unfilled = [z for z in copper if not z["islands"]]
    rep.add("B4", "toutes les zones sont remplies",
            PASS if not unfilled else FAIL,
            f"{len(copper) - len(unfilled)}/{len(copper)}",
            [f"{z['name']} ({z['net']}) vide" for z in unfilled])

    for z in copper:
        if "In1.Cu" in z["layers"]:
            rep.add("B2", f"plan {z['name']} d'un seul tenant",
                    PASS if z["islands"] == 1 else FAIL, f"{z['islands']} îlot(s)")
    for z in copper:
        if "In2.Cu" in z["layers"] and z["net"] != "GND":
            rep.add("B3", f"plan {z['name']} d'un seul tenant",
                    PASS if z["islands"] == 1 else WARN, f"{z['islands']} îlot(s)")


def check_switching(bd, rep):
    bad_via, bad_layer, long_ = [], [], []
    for net, budget in sorted(SWITCHING.items()):
        segs = bd.net_segments(net)
        if not segs:
            continue
        nv = len(bd.net_vias(net))
        layers = sorted({s["layer"] for s in segs})
        L = sum(dist(s["a"], s["b"]) for s in segs)
        if nv:
            bad_via.append(f"{net} : {nv} via(s)")
        if len(layers) > 1:
            bad_layer.append(f"{net} : {', '.join(layers)}")
        if L > budget:
            long_.append(f"{net} : {L:.1f} mm pour un budget de {budget:.0f}")
    # Une via sur un nœud de commutation impose un dégagement dans le plan de
    # masse juste sous la boucle chaude : on paie l'inductance à l'aller et la
    # fente au retour. C'est le contrôle le plus important du lot.
    rep.add("C1", "aucune via sur un nœud de commutation",
            PASS if not bad_via else FAIL, f"{len(bad_via)} net(s) en défaut", bad_via)
    rep.add("C2", "nœuds de commutation sur une seule couche",
            PASS if not bad_layer else FAIL, f"{len(bad_layer)} net(s)", bad_layer)
    rep.add("C3", "longueur des nœuds de commutation",
            PASS if not long_ else WARN, f"{len(long_)} au-dessus du budget", long_)


def _pad(bd, ref, net):
    for p in bd.footprints.get(ref, {"pads": []})["pads"]:
        if p.net == net:
            return p
    return None


def _pin(bd, ref, net):
    return _pad(bd, ref, net)


def check_loops(bd, rep):
    rows, worst = [], PASS
    for c, net, ic, icnet, role, w, f in LOOPS:
        a, b = _pad(bd, c, net), _pin(bd, ic, icnet)
        if not a or not b:
            continue
        d = dist((a.x, a.y), (b.x, b.y))
        v = FAIL if d > f else (WARN if d > w else PASS)
        worst = min(worst, v, key=lambda x: _ORDER[x])
        if v != PASS:
            rows.append(f"{c} ({role}) -> {ic} : {d:.2f} mm (seuil {w:.0f}/{f:.0f})")
    rep.add("D1", "boucles chaudes : condensateur -> broche", worst,
            f"{len(LOOPS)} liaisons contrôlées", rows)

    rows, worst = [], PASS
    for c, net, ic, kind, w, f in DECOUPLING:
        a, b = _pad(bd, c, net), _pin(bd, ic, net)
        if not a or not b:
            continue
        d = dist((a.x, a.y), (b.x, b.y))
        v = FAIL if d > f else (WARN if d > w else PASS)
        worst = min(worst, v, key=lambda x: _ORDER[x])
        rows.append(f"{c} ({kind}) -> {ic} : {d:.2f} mm" + ("" if v == PASS else "  <-- au-dessus du seuil"))
    rep.add("D2", "découplage : condensateur -> broche", worst,
            f"{len(DECOUPLING)} contrôlés", rows)

    # D4 — la via de masse EST le chemin de retour. Une pastille 0805 fait
    # ~1,45 mm : à 1,3 mm de son centre la via la touche encore ; au-delà de
    # 2,5 mm on a inséré un moignon mesurable dans la boucle.
    gnd_vias = [v["p"] for v in bd.vias if v["net"] == "GND"]
    rows, worst = [], PASS
    caps = sorted(r for r in bd.footprints if r.startswith("C"))
    undeclared = [r for r in caps if r not in CAP_ROLE]
    for ref in caps:
        role, why = CAP_ROLE.get(ref, (None, ""))
        pads = bd.footprints[ref]["pads"]
        g = [p for p in pads if p.net == "GND"]
        rails = "/".join(sorted({p.net for p in pads if p.net and p.net != "GND"}))
        if role == "flottant":
            # on vérifie la déclaration contre la géométrie, pas l'inverse
            if g:
                rows.append(f"{ref} : déclaré flottant mais il a une pastille GND")
                worst = min(worst, WARN, key=lambda x: _ORDER[x])
            continue
        if not g or not gnd_vias:
            continue
        d = min(dist((g[0].x, g[0].y), v) for v in gnd_vias)
        if role is None:
            rows.append(f"{ref} ({rails}) : criticité non déclarée — via à {d:.2f} mm")
            worst = min(worst, WARN, key=lambda x: _ORDER[x])
            continue
        if role != "critique":
            continue
        v = FAIL if d > GND_VIA_FAIL else (WARN if d > GND_VIA_WARN else PASS)
        worst = min(worst, v, key=lambda x: _ORDER[x])
        if v != PASS:
            rows.append(f"{ref} ({rails}) : via GND à {d:.2f} mm — {why}")
    n_crit = sum(1 for r, (k, _) in CAP_ROLE.items() if k == "critique")
    rep.add("D4", "retour : pastille GND -> via la plus proche", worst,
            f"{len(caps)} condensateurs, {n_crit} déclarés critiques, "
            f"{len(undeclared)} sans criticité · seuils {GND_VIA_WARN}/{GND_VIA_FAIL} mm",
            rows)

    # D5 — le condensateur doit être *traversé*. S'il n'a aucun lien de cuivre en
    # surface avec la broche qu'il sert, il ne la sert pas préférentiellement :
    # les deux descendent chacun dans le plan et le condensateur est en dérivation.
    rows = []
    for c, net, ic, kind, *_ in DECOUPLING:
        a, b = _pad(bd, c, net), _pin(bd, ic, net)
        if not a or not b:
            continue
        u = fcu_graph(bd, net)
        if u.find(("p", a.ref, a.num)) != u.find(("p", b.ref, b.num)):
            rows.append(f"{c} et {ic} ne se rejoignent que par le plan")
    rep.add("D5", "topologie : le condensateur est traversé",
            PASS if not rows else WARN, f"{len(rows)} en dérivation", rows)


def check_vias(bd, rep):
    smd = [p for p in bd.pads if p.is_smd()]
    rows, worst = [], PASS
    for v in bd.vias:
        best, who = 99.0, None
        for p in smd:
            d = point_to_rect(v["p"], p.x, p.y, p.hw, p.hh, p.rot)
            if d < best:
                best, who = d, p
        if who is None:
            continue
        # Une via délibérément posée dans un pavé thermique du même net est
        # l'opération recommandée, pas un défaut.
        if who.ref in THERMAL_PADS and who.net == v["net"]:
            continue
        margin = best - v["drill"] / 2
        verdict = FAIL if margin < 0 else (WARN if margin < VIA_PAD_MARGIN else PASS)
        worst = min(worst, verdict, key=lambda x: _ORDER[x])
        if verdict != PASS:
            rows.append(f"via {v['net']} @({v['p'][0]:.2f},{v['p'][1]:.2f}) "
                        f"Ø{v['drill']} -> {who.ref}.{who.num} : {margin:+.3f} mm")
    rep.add("E1", "trou de via hors des pastilles CMS", worst,
            f"{len(bd.vias)} vias, marge visée {VIA_PAD_MARGIN} mm", rows)


def check_escapes(bd, rep, classes):
    dru = open(DRU, encoding="utf-8").read()
    listed = set(re.findall(r"B\.Reference == \'(\w+)\'", dru))
    fine = {r: bd.pitch(r) for r in bd.footprints if bd.pitch(r) <= 0.55}
    missing = sorted(set(fine) - listed)
    # La règle d'échappée du .kicad_dru énumère ses boîtiers à la main : ce
    # contrôle est ce qui empêche un composant à pas fin d'y échapper en silence.
    rep.add("F3", "règle DRU : couverture des boîtiers à pas fin",
            PASS if not missing else WARN,
            f"{len(fine)} boîtier(s) au pas ≤ 0,55 mm, {len(missing)} hors règle",
            [f"{r} (pas {fine[r]:.2f} mm) absent de la règle" for r in missing])

    def clearance_of(net):
        c = classes.get(net)
        return c["clearance"] if c else 0.2

    rows, worst, table = [], PASS, []
    for ref in sorted(fine):
        pads = bd.perimeter(ref)
        ceilings = {}
        for p in pads:
            if not p.net:
                continue
            # Deux broches voisines du même net ne se contraignent pas : la piste
            # peut viser leur milieu. C'est ce qui rend J4.15/16 routables en
            # 0,6 mm là où une broche isolée plafonnerait à 0,2.
            grp = [q for q in pads
                   if q.net == p.net and dist((p.x, p.y), (q.x, q.y)) <= fine[ref] * 1.6]
            cx = sum(q.x for q in grp) / len(grp)
            cy = sum(q.y for q in grp) / len(grp)
            reach = min((point_to_rect((cx, cy), q.x, q.y, q.hw, q.hh, q.rot)
                         for q in pads if q.net != p.net), default=99.0)
            if reach > 50:
                continue
            iso = 0.2 if ref in listed else max(clearance_of(p.net), 0.2)
            ceilings[p.num] = 2 * (reach - iso)
        if ceilings:
            table.append(f"{ref} : plafond {min(ceilings.values()):.2f} à "
                         f"{max(ceilings.values()):.2f} mm selon la broche")
    # Pas de verdict sur ce qui est déjà tracé : une piste peut sortir par le
    # côté libre d'une pastille, et le plafond ci-dessous suppose un départ
    # centré. Le DRC mesure la géométrie réelle et fait foi. Cette table sert
    # AVANT de router — c'est la seule chose qu'aucun outil ne donne.
    rows = table
    rep.add("F1", "plafonds d'échappée à pas fin", INFO,
            f"{len(fine)} boîtier(s) — à viser avant de router", rows)


def check_widths(bd, rep, classes):
    by_net = defaultdict(list)
    for s in bd.segments:
        by_net[s["net"]].append(s)
    rows, worst = [], PASS
    for net, segs in sorted(by_net.items()):
        cls = classes.get(net)
        if not cls:
            continue
        under = [s for s in segs if s["w"] < cls["track_width"] - 1e-6]
        if not under:
            continue
        longest = max(dist(s["a"], s["b"]) for s in under)
        if longest > NECK_MAX:
            worst = min(worst, WARN, key=lambda x: _ORDER[x])
            rows.append(f"{net} : {len(under)} tronçon(s) sous {cls['track_width']} mm, "
                        f"le plus long {longest:.1f} mm")
    rep.add("G1", "tronçons plus étroits que leur classe", worst,
            f"cols tolérés jusqu'à {NECK_MAX} mm", rows)

    rows, worst, parallel = [], PASS, 0
    for net, (source, sinks) in sorted(FLOW.items()):
        for sg, amps in segment_currents(bd, net, source, sinks):
            L = dist(sg["a"], sg["b"])
            if L <= NECK_MAX:        # un col est trop court pour chauffer
                continue
            if amps <= 0:            # chemin parallèle : pas le goulot à lui seul
                parallel += 1
                continue
            rise = ipc_rise(sg["w"], amps)
            v = FAIL if rise > RISE_FAIL else (WARN if rise > RISE_TARGET else PASS)
            worst = min(worst, v, key=lambda x: _ORDER[x])
            if v != PASS:
                rows.append(f"{net} : {sg['w']:.2f} mm sur {L:.1f} mm pour {amps:.2f} A "
                            f"-> ΔT ≈ {rise:.0f} °C "
                            f"({sg['a'][0]:.1f},{sg['a'][1]:.1f})")
    rep.add("G2", "échauffement, tronçon par tronçon", worst,
            f"cible {RISE_TARGET:.0f} °C, limite {RISE_FAIL:.0f} °C · "
            f"{parallel} tronçon(s) sur chemin parallèle · estimation pessimiste",
            sorted(set(rows)))


def segment_currents(bd, net, source, sinks):
    """Courant de chaque tronçon, par coupe.

    On retire un segment : si un consommateur se retrouve séparé de la source,
    ce segment portait son courant. S'il ne sépare rien, un chemin parallèle
    existe et le segment n'est pas le goulot — on ne peut pas lui attribuer le
    total, et c'est exactement l'erreur que ce contrôle faisait avant.
    """
    segs = bd.net_segments(net)
    pads = bd.net_pads(net)
    vias = bd.net_vias(net)
    planed = any(z["net"] == net and z["islands"] for z in bd.zones)
    pts_all = [(("s", i, e), (sg["a"], sg["b"])[e])
               for i, sg in enumerate(segs) for e in (0, 1)]

    def components(skip):
        u = Union()
        pts = [(k, p) for k, p in pts_all if k[1] != skip]
        for i, _ in enumerate(segs):
            if i != skip:
                u.join(("s", i, 0), ("s", i, 1))
        for a in range(len(pts)):
            for b in range(a + 1, len(pts)):
                if dist(pts[a][1], pts[b][1]) < 0.01:
                    u.join(pts[a][0], pts[b][0])
        for p in pads:
            k = ("p", p.ref, p.num)
            u.find(k)
            for key, pt in pts:
                # demi-largeur comprise : l'axe d'une piste de 0,7 mm peut
                # s'arrêter 0,3 mm avant le bord et la recouvrir quand même
                if point_to_rect(pt, p.x, p.y, p.hw, p.hh, p.rot) <= segs[key[1]]["w"] / 2 + 0.01:
                    u.join(k, key)
        for j, v in enumerate(vias):
            k = ("v", j)
            for key, pt in pts:
                if dist(pt, v["p"]) <= v["size"] / 2:
                    u.join(k, key)
            for p in pads:
                if point_to_rect(v["p"], p.x, p.y, p.hw, p.hh, p.rot) <= 0.01:
                    u.join(k, ("p", p.ref, p.num))
        if planed:      # un plan rempli court-circuite tout ce qui y descend
            anchor = ("plan", net)
            for j, _ in enumerate(vias):
                u.join(("v", j), anchor)
            for p in pads:
                if p.drill > 0:
                    u.join(("p", p.ref, p.num), anchor)
        return u

    src = [("p", p.ref, p.num) for p in pads if p.ref == source]
    out = []
    for i, sg in enumerate(segs):
        u = components(i)
        roots = {u.find(k) for k in src}
        cut = 0.0
        for ref, amps in sinks.items():
            keys = [("p", p.ref, p.num) for p in pads if p.ref == ref]
            if keys and not any(u.find(k) in roots for k in keys):
                cut += amps
        out.append((sg, cut))
    return out


def check_cross(bd, rep, outdir):
    # Les feuilles du projet, et elles seules : un .kicad_sch égaré dans le
    # dossier (sauvegarde, essai « -v2 ») ajouterait des UUID et ferait passer
    # ce contrôle pour de mauvaises raisons.
    root = open(SCH, encoding="utf-8").read()
    sheets = [SCH] + [os.path.join(PROJ, m) for m in
                      re.findall(r'\(property "Sheetfile" "([^"]+)"', root)]
    sch_uuids = set()
    for f in sheets:
        if os.path.exists(f):
            sch_uuids |= set(re.findall(r'\(uuid "([^"]+)"\)',
                                        open(f, encoding="utf-8").read()))
    paths = re.findall(r'\(path "/([0-9a-f-]+)/([0-9a-f-]+)"\)', bd.text)
    orphan = [c for _, c in paths if c not in sch_uuids]
    # gen_sch.py renouvelle les UUID à chaque exécution ; les empreintes du PCB
    # les référencent. Un chemin orphelin annonce qu'une mise à jour depuis le
    # schéma reposera les empreintes et perdra le placement.
    rep.add("H1", "empreintes rattachées à leur symbole",
            PASS if not orphan else FAIL,
            f"{len(paths) - len(orphan)}/{len(paths)} résolues",
            [f"chemin orphelin : {c}" for c in orphan])

    for lib, expected in MECHANICAL.items():
        got = sum(1 for f in bd.footprints.values() if lib in f["lib"])
        rep.add("H4", f"empreintes mécaniques ({lib})",
                PASS if got >= expected else FAIL, f"{got}/{expected}",
                [] if got >= expected else
                ["sans symbole au schéma, elles sont effacées en silence par",
                 "« supprimer les empreintes en trop » lors d'une mise à jour."])

    # Une pastille sans net n'a ni chevelu, ni violation DRC, ni rien. C'est
    # ainsi que la pastille mécanique de J4 est restée invisible tout le projet.
    nonet = [p for p in bd.pads
             if not p.net and p.num and p.kind != "np_thru_hole"
             and not p.ref.startswith("MH")]
    rep.add("H2", "pastilles sans net", PASS if not nonet else WARN,
            f"{len(nonet)}", [f"{p.ref}.{p.num} ({p.shape})" for p in nonet])


def check_fab(bd, rep):
    drills = Counter()
    for p in bd.pads:
        if p.is_hole():
            drills[round(p.drill, 3)] += 1
    for v in bd.vias:
        drills[round(v["drill"], 3)] += 1
    below = {d: n for d, n in drills.items() if d < DRILL_FLOOR}
    who = defaultdict(set)
    for p in bd.pads:
        if p.is_hole() and p.drill < DRILL_FLOOR:
            who[round(p.drill, 3)].add(p.ref)
    rep.add("I1", "perçages sous le procédé courant",
            PASS if not below else WARN,
            f"plancher {DRILL_FLOOR} mm — " +
            ", ".join(f"Ø{d}×{n}" for d, n in sorted(drills.items())),
            [f"Ø{d} : {n} trous ({', '.join(sorted(who[d]))}) — "
             f"à confirmer sur la page de capacités du fabricant"
             for d, n in sorted(below.items())])

    rows, worst = [], PASS
    refs = sorted(bd.footprints)
    for i, ra in enumerate(refs):
        pa = [p for p in bd.footprints[ra]["pads"] if p.is_smd()]
        for rb in refs[i + 1:]:
            pb = [p for p in bd.footprints[rb]["pads"] if p.is_smd()]
            best = 99.0
            for a in pa:
                for b in pb:
                    if abs(a.x - b.x) > 6 or abs(a.y - b.y) > 6:
                        continue
                    if a.net and a.net == b.net:
                        continue
                    best = min(best, rect_dist((a.x, a.y, a.hw, a.hh, a.rot),
                                               (b.x, b.y, b.hw, b.hh, b.rot)))
            if best < MASK_DAM:
                worst = min(worst, WARN, key=lambda x: _ORDER[x])
                rows.append(f"{ra} / {rb} : ~{best:.2f} mm")
    rep.add("I3", "bandeau de vernis entre composants", worst,
            f"seuil {MASK_DAM} mm", rows)


def check_outline(bd, rep):
    xs, ys = [], []
    for m in re.finditer(r"\(gr_(?:line|rect|arc)\b((?:(?!\n\t\().)*)", bd.text, re.S):
        if '"Edge.Cuts"' not in m.group(1):
            continue
        for a, b in re.findall(r"\((?:start|end|mid|center) ([\d.-]+) ([\d.-]+)\)", m.group(1)):
            xs.append(float(a))
            ys.append(float(b))
    if not xs:
        rep.add("I5", "contour de carte", WARN, "aucun Edge.Cuts trouvé")
        return
    px = [p.x for p in bd.pads]
    py = [p.y for p in bd.pads]
    slack = [("gauche", min(px) - min(xs)), ("droite", max(xs) - max(px)),
             ("haut", min(py) - min(ys)), ("bas", max(ys) - max(py))]
    big = [f"{c} : {v:.1f} mm" for c, v in slack if v > 5]
    rep.add("I5", "contour vs cuivre utile", INFO,
            f"carte {max(xs)-min(xs):.1f} x {max(ys)-min(ys):.1f} mm, "
            f"composants {max(px)-min(px):.1f} x {max(py)-min(py):.1f}",
            [f"marge {c}" for c in big] or
            [f"marges : " + " · ".join(f"{c} {v:.1f}" for c, v in slack)])


def check_sensitive(bd, rep):
    sw_segs = [s for s in bd.segments if s["net"] in SWITCHING]
    rows, worst = [], PASS
    for net, level in (("FB_5V", WARN), ("PMIC_TS", INFO)):
        segs = bd.net_segments(net)
        if not segs:
            rows.append(f"{net} : pas encore routé")
            continue
        best = 99.0
        for s in segs:
            for t in sw_segs:
                for e in (s["a"], s["b"]):
                    for e2 in (t["a"], t["b"]):
                        best = min(best, dist(e, e2))
        if best < 1.0:
            worst = min(worst, level, key=lambda x: _ORDER[x])
            rows.append(f"{net} : {best:.2f} mm d'un nœud de commutation")
        else:
            rows.append(f"{net} : {best:.2f} mm du plus proche nœud de commutation")
    # FB_5V est le seul signal capable de dégrader une régulation : 60 kΩ
    # d'impédance de Thévenin, dans la boucle d'asservissement du boost.
    # PMIC_TS est un pont fixe qui ne mesure rien (cellule sans thermistance).
    rep.add("J1", "signaux haute impédance vs commutation", worst,
            "FB_5V critique, PMIC_TS informatif", rows)

    rows, worst = [], PASS
    for a, b in (("USB_DP", "USB_DM"), ("USB_PROG_DP", "USB_PROG_DM")):
        sa, sb = bd.net_segments(a), bd.net_segments(b)
        if not sa or not sb:
            rows.append(f"{a}/{b} : pas encore routés")
            continue
        la, lb = {s["layer"] for s in sa}, {s["layer"] for s in sb}
        La = sum(dist(s["a"], s["b"]) for s in sa)
        Lb = sum(dist(s["a"], s["b"]) for s in sb)
        if la != lb or len(la) > 1:
            worst = min(worst, WARN, key=lambda x: _ORDER[x])
            rows.append(f"{a}/{b} : couches {sorted(la)} vs {sorted(lb)}")
        rows.append(f"{a}/{b} : {La:.1f} / {Lb:.1f} mm (écart {abs(La - Lb):.1f})")
    rep.add("J3", "paires USB sur une couche unique", worst, "", rows)


def load_classes():
    import json
    d = json.load(open(PRO, encoding="utf-8"))
    ns = d["net_settings"]
    by_name = {c["name"]: c for c in ns["classes"]}
    out = {}
    for p in ns.get("netclass_patterns", []):
        c = by_name.get(p["netclass"])
        if c:
            out[p["pattern"]] = c
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--profil", choices=("routage", "fabrication"), default="routage")
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args()

    if not os.path.exists(PCB):
        sys.exit(f"introuvable : {PCB}")
    bd = Board(PCB)

    hits, bad = autotest(bd)
    if hits < 5:
        sys.exit("autotest : seulement %d coïncidences piste/pastille — la "
                 "transformation est suspecte, on s'arrête plutôt que de mentir." % hits)
    if bad:
        sys.exit("autotest : orientation de pastille incohérente sur %s — "
                 "la règle d'échange largeur/hauteur est fausse, on s'arrête."
                 % ", ".join(sorted(bad)[:8]))

    print(f"\n  carte : {len(bd.footprints)} empreintes · {len(bd.segments)} segments · "
          f"{len(bd.vias)} vias · {len(bd.zones)} zones")
    print(f"  profil : {args.profil} · autotest : {hits} coïncidences, "
          f"orientations cohérentes\n")

    rep = Report(args.verbose)
    outdir = tempfile.mkdtemp(prefix="checkpcb-")
    try:
        check_kicad(bd, rep, outdir, args.profil)
        check_layers(bd, rep)
        check_switching(bd, rep)
        check_loops(bd, rep)
        check_vias(bd, rep)
        check_escapes(bd, rep, load_classes())
        check_widths(bd, rep, load_classes())
        check_cross(bd, rep, outdir)
        check_fab(bd, rep)
        check_outline(bd, rep)
        check_sensitive(bd, rep)
    finally:
        shutil.rmtree(outdir, ignore_errors=True)

    rep.show()
    n = Counter(r[2] for r in rep.rows)
    print(f"\n  {n[PASS]} PASS · {n[WARN]} WARN · {n[FAIL]} FAIL · {n[INFO]} INFO\n")
    return 1 if n[FAIL] else 0


if __name__ == "__main__":
    sys.exit(main())
