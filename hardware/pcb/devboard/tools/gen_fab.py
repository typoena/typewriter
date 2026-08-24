#!/usr/bin/env python3
"""Fichiers de fabrication : gerbers, perçage, placement et BOM au format JLCPCB.

Lecture seule sur le projet, tout est écrit dans `fab/`, qui est ignoré par git.
Le fichier de placement porte les corrections de rotation de la table ci-dessous.

    python3 tools/gen_fab.py
"""
import csv
import os
import subprocess
import sys
import zipfile

ICI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ICI)
import check_pcb  # noqa: E402  — réutilise son lecteur de pastilles

CARTE = os.path.dirname(ICI)
NOM = "typoena-devboard"
PCB = os.path.join(CARTE, NOM + ".kicad_pcb")
BOM = os.path.join(CARTE, NOM + "-bom.csv")
FAB = os.path.join(CARTE, "fab")

# Rotation à ajouter à la valeur du PCB, en degrés, sens trigonométrique.
#
# La rotation d'un composant n'a de sens que relativement à une orientation zéro,
# et celle de la bibliothèque JLCPCB — l'orientation dans laquelle la pièce sort
# de la bande — n'est pas celle de l'empreinte KiCad. L'écart ne se voit que dans
# leur prévisualisation, où chaque valeur ci-dessous a été relevée à l'œil sur le
# dessin des broches, jamais calculée.
#
# À reprendre si une empreinte change de bibliothèque, ou si le fabricant change.
ROTATION_JLCPCB = {
    "U4": 180,   # TPS61023, SOT-563
    "U5": -90,   # CH343P, QFN-16
    "J1": 180,   # JST-PH 2 points
    "J2": 180,
    "J3": 180,
}

# Composants dont la position doit rester celle de l'origine d'empreinte plutôt
# que le centre du champ de pastilles — voir `centres()`. Vide tant qu'aucun ne
# l'exige ; une entrée ici se constate dans la prévisualisation, elle ne se déduit pas.
ORIGINE_BRUTE = set()

COUCHES = ("F.Cu,In1.Cu,In2.Cu,B.Cu,F.Paste,B.Paste,"
           "F.SilkS,B.SilkS,F.Mask,B.Mask,Edge.Cuts")

# Extensions Protel des couches qui partent chez le fabricant. Les fichiers de
# cartographie des perçages et le .gbrjob n'y figurent pas : ils décrivent la
# commande, ils ne la fabriquent pas.
POUR_ARCHIVE = (".gtl", ".g1", ".g2", ".gbl", ".gtp", ".gbp",
                ".gto", ".gbo", ".gts", ".gbs", ".gm1", ".drl")


def cli(*args):
    r = subprocess.run(["kicad-cli", "pcb"] + list(args),
                       capture_output=True, text=True)
    if r.returncode:
        sys.exit(f"kicad-cli a échoué :\n{r.stderr or r.stdout}")


def normalise(a):
    a = (a + 180) % 360 - 180
    return 180.0 if a == -180 else a


def centres():
    """Centre du champ de pastilles de chaque empreinte, en coordonnées carte.

    `kicad-cli export pos` donne l'**origine de l'empreinte**, alors que le
    fabricant place la pièce par son **centre**. Les deux coïncident pour la
    plupart des empreintes, et l'écart passe alors inaperçu ; il ne se voit que
    sur celles dont l'origine est excentrée — bord de sortie d'un USB-C,
    extrémité des broches d'un module — où la pièce apparaît décalée d'autant
    dans la prévisualisation du fabricant.
    """
    bd = check_pcb.Board(PCB)
    out = {}
    for ref, f in bd.footprints.items():
        ps = [p for p in f["pads"] if p.num]
        if not ps:
            continue
        out[ref] = ((min(p.x for p in ps) + max(p.x for p in ps)) / 2,
                    (min(p.y for p in ps) + max(p.y for p in ps)) / 2)
    return out


def main():
    os.makedirs(FAB, exist_ok=True)
    for f in os.listdir(FAB):
        os.remove(os.path.join(FAB, f))

    # --check-zones refait le remplissage avant de tracer : le gerber ne peut pas
    # hériter d'un remplissage périmé. --subtract-soldermask découpe la
    # sérigraphie des ouvertures de masque ici plutôt que de laisser le
    # fabricant le faire sans qu'on sache comment.
    cli("export", "gerbers", "--output", FAB + os.sep, "--layers", COUCHES,
        "--no-x2", "--subtract-soldermask", "--check-zones", PCB)
    cli("export", "drill", "--output", FAB + os.sep, "--format", "excellon",
        "--drill-origin", "absolute", "--excellon-units", "mm",
        "--excellon-zeros-format", "decimal", "--excellon-separate-th", PCB)

    brut = os.path.join(FAB, "_pos.csv")
    cli("export", "pos", "--output", brut, "--format", "csv", "--units", "mm",
        "--side", "both", "--exclude-dnp", PCB)

    with open(brut) as f:
        pieces = list(csv.DictReader(f))
    os.remove(brut)

    cen = centres()
    recadres = []
    cpl = os.path.join(FAB, NOM + "-cpl.csv")
    with open(cpl, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Designator", "Mid X", "Mid Y", "Layer", "Rotation"])
        for p in pieces:
            ref = p["Ref"]
            x, y = float(p["PosX"]), float(p["PosY"])
            if ref in cen and ref not in ORIGINE_BRUTE:
                cx, cy = cen[ref]
                if abs(cx - x) > 0.05 or abs(-cy - y) > 0.05:
                    recadres.append((ref, cx - x, -cy - y))
                x, y = cx, -cy
            rot = normalise(float(p["Rot"]) + ROTATION_JLCPCB.get(ref, 0))
            w.writerow([ref, f"{x:.4f}", f"{y:.4f}",
                        "top" if p["Side"] == "top" else "bottom", f"{rot:.4f}"])
    montes = {p["Ref"] for p in pieces}

    with open(BOM) as f:
        lignes = list(csv.DictReader(f))

    # Une référence LCSC, une ligne. Le fabricant rapproche chaque ligne d'un
    # article de son catalogue et refuse le même article deux fois. La BOM du
    # dépôt, elle, groupe par valeur : trois JST-PH identiques y occupent trois
    # lignes parce que leur champ Valeur porte leur rôle — « BATTERIE »,
    # « BOUTON contact » — et non une valeur de composant.
    groupes = {}
    for r in lignes:
        if r["DNP"] or not r["LCSC"]:
            continue
        # JLCPCB ne sait pas lire une plage « C18-C20 » : la BOM du dépôt est
        # exportée en désignateurs séparés, on se contente de filtrer.
        refs = [d.strip() for d in r["Reference"].split(",") if d.strip() in montes]
        if not refs:
            continue
        g = groupes.setdefault(r["LCSC"], {
            "refs": [], "valeurs": [], "mpn": r["MPN"],
            "empreinte": r["Footprint"].split(":")[-1]})
        g["refs"] += refs
        if r["Value"] not in g["valeurs"]:
            g["valeurs"].append(r["Value"])

    jlc = os.path.join(FAB, NOM + "-bom-jlcpcb.csv")
    couverts = set()
    fusions = []
    with open(jlc, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Comment", "Designator", "Footprint", "LCSC Part #"])
        for lcsc, g in groupes.items():
            # Quand les rôles divergent, c'est la référence fabricant qui les
            # réunit : elle décrit la pièce, ce que la valeur ne fait plus.
            if len(g["valeurs"]) == 1:
                commentaire = g["valeurs"][0]
            else:
                commentaire = g["mpn"] or " / ".join(g["valeurs"])
                fusions.append((lcsc, commentaire, g["refs"]))
            w.writerow([commentaire, ",".join(g["refs"]), g["empreinte"], lcsc])
            couverts |= set(g["refs"])

    archive = os.path.join(FAB, NOM + "-gerbers.zip")
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
        for nom in sorted(os.listdir(FAB)):
            if nom.endswith(POUR_ARCHIVE):
                z.write(os.path.join(FAB, nom), nom)

    ecart = montes ^ couverts
    print(f"  {len(montes)} composants montés · placement et BOM concordants"
          if not ecart else f"  !! écart placement/BOM : {sorted(ecart)}")
    print(f"  {len(ROTATION_JLCPCB)} rotation(s) corrigée(s) : "
          f"{', '.join(sorted(ROTATION_JLCPCB))}")
    if recadres:
        print(f"  {len(recadres)} position(s) recalée(s) sur le centre des pastilles :")
        for ref, dx, dy in sorted(recadres):
            print(f"      {ref:<5} dX={dx:+7.3f}  dY={dy:+7.3f}")
    print(f"  BOM : {len(groupes)} ligne(s), une par référence LCSC")
    for lcsc, commentaire, refs in fusions:
        print(f"      {lcsc} regroupe {', '.join(refs)} sous « {commentaire} »")
    print(f"  archive : {os.path.relpath(archive, CARTE)}")
    if ecart:
        sys.exit(1)


if __name__ == "__main__":
    main()
