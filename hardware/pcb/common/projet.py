#!/usr/bin/env python3
"""Chemins d'un projet de carte, résolus depuis le nom passé sur la ligne de commande.

Les outils de `common/` servent les deux cartes : aucun ne doit porter de chemin
en dur. `check_pcb.py` a déjà eu à s'en défendre — un chemin absolu dans un
générateur a réécrit le vrai schéma pendant un essai censé rester en bac à sable.
"""
import os
import sys

# common/ vit dans hardware/pcb/ ; les projets de carte sont ses voisins.
RACINE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class Projet:
    def __init__(self, carte):
        self.carte = carte
        self.dir = os.path.join(RACINE, carte)
        self.nom = "typoena-" + carte
        base = os.path.join(self.dir, self.nom)
        self.pcb = base + ".kicad_pcb"
        self.sch = base + ".kicad_sch"
        self.dru = base + ".kicad_dru"
        self.pro = base + ".kicad_pro"
        self.bom = base + "-bom.csv"
        self.fab = os.path.join(self.dir, "fab")
        self.lock = os.path.join(self.dir, "netlist.lock")


def cartes():
    return sorted(d for d in os.listdir(RACINE)
                  if os.path.exists(os.path.join(RACINE, d, "board.py")))


def depuis_argv():
    """Premier argument non-option de la ligne de commande."""
    noms = [a.rstrip("/") for a in sys.argv[1:] if not a.startswith("-")]
    if not noms:
        sys.exit(f"usage : {os.path.basename(sys.argv[0])} <carte> [options]\n"
                 f"cartes : {', '.join(cartes())}")
    if noms[0] not in cartes():
        sys.exit(f"carte inconnue : {noms[0]} (cartes : {', '.join(cartes())})")
    return Projet(noms[0])
