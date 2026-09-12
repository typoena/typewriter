#!/usr/bin/env python3
"""Génère le schéma d'une carte : tronc commun + delta de son `board.py`.

    python3 common/gen_sch.py mainboard          # génère
    python3 common/gen_sch.py devboard           # vérifie le verrou, n'écrit rien
    python3 common/gen_sch.py <carte> --lock     # (re)pose le verrou

Une carte qui porte un `netlist.lock` est **figée** : la commande vérifie que son
netlist n'a pas bougé, puis s'arrête sans réécrire une seule feuille.

Deux raisons de ne jamais régénérer une carte figée, l'une aussi dirimante que
l'autre :

- le tronc commun de `netlist.py` sert les deux cartes ; une retouche faite pour
  l'une déplacerait le netlist de l'autre en silence. Le verrou la fait échouer ;
- le PCB rattache chaque empreinte à son symbole par un chemin d'UUID, et
  `kisch.uid()` est un uuid4 : une régénération tire de nouveaux UUID et **casse
  tous ces liens**. Une carte dont le PCB existe se verrouille, sans attendre
  qu'elle parte en fabrication.

Poser le verrou est donc le dernier geste d'une carte terminée. Pour en reprendre
une, supprimer son `netlist.lock` — et rouvrir le PCB en sachant que les liens
symbole/empreinte sont à refaire.
"""
import difflib
import importlib.util
import os
import sys

ICI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, ICI)

import netlist  # noqa: E402


def charger(carte):
    chemin = os.path.join(os.path.dirname(ICI), carte, "board.py")
    if not os.path.exists(chemin):
        sys.exit(f"carte inconnue : {carte} (pas de {chemin})")
    spec = importlib.util.spec_from_file_location(f"board_{carte}", chemin)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def empreinte(n, board):
    """Sérialisation stable du netlist — les UUID en sont absents.

    `kisch.uid()` est un uuid4 : deux générations ne se comparent pas octet à
    octet, seule la liste des composants et de leurs nets est reproductible.
    """
    lignes = []
    for ref, lib_id, value, fp, pins, props in sorted(n.comps, key=lambda c: c[0]):
        broches = " ".join(f"{k}={v}" for k, v in sorted(pins.items()))
        extra = " ".join(f"{k}={v}" for k, v in sorted(props.items()))
        dnp = " DNP" if ref in board.DNP else ""
        nc = " NC:" + ",".join(board.NC.get(ref, [])) if board.NC.get(ref) else ""
        lignes.append(f"{ref}\t{lib_id}\t{value}\t{fp}\t{broches}\t{extra}{dnp}{nc}")
    return "\n".join(lignes) + "\n"


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    relock = "--lock" in sys.argv
    if len(args) != 1:
        sys.exit(__doc__)
    carte = args[0].rstrip("/")
    board = charger(carte)
    outdir = os.path.join(os.path.dirname(ICI), carte)

    n = netlist.Netlist()
    netlist.alimentation(n)
    netlist.decouplage_mcu(n)
    netlist.ecran(n)
    netlist.io(n)
    board.construire(n)
    netlist.drapeaux(n)
    netlist.referencer(n)
    netlist.remplacer(n, getattr(board, "REMPLACEMENTS", {}))

    lock = os.path.join(outdir, "netlist.lock")
    courant = empreinte(n, board)
    if relock:
        open(lock, "w").write(courant)
        print(f"{carte} figee : verrou pose sur {len(n.comps)} composants")
        return
    if os.path.exists(lock):
        fige = open(lock).read()
        if fige != courant:
            ecart = "\n".join(difflib.unified_diff(
                fige.splitlines(), courant.splitlines(),
                "netlist.lock", "genere", lineterm="", n=0))
            sys.exit(f"{carte} est figee et son netlist a bouge :\n{ecart}\n\n"
                     "Corrige la regression, ou reprends la carte en supprimant\n"
                     "son netlist.lock — en sachant que regenerer le schema tire\n"
                     "de nouveaux UUID et casse les liens symbole/empreinte du PCB.")
        print(f"{carte} figee : netlist conforme "
              f"({len(fige.splitlines())} composants), rien a regenerer")
        return

    netlist.emettre(board, n, outdir)


if __name__ == "__main__":
    main()
