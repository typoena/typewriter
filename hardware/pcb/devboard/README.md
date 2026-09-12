# Typoena devboard — carte de mise au point

Elle porte toute l'électronique de la machine **sauf le MCU** : à sa place, deux
rangées de barrettes qui reçoivent une ESP32-S3-DevKitC-1. C'est la carte
fabriquée, celle qui est sur l'établi.

| | |
| --- | --- |
| Schéma | hiérarchique, 4 feuilles — 86 composants, ERC 0 violation |
| Nets | 62 |
| BOM | 40 lignes, 37 référencées LCSC |
| PCB | routé — 99 × 45 mm, 4 couches, 85 empreintes, 569 segments, 80 vias, 4 zones remplies, 0 connexion restante |
| Contrôle | `check_pcb.py devboard` : 26 PASS · 5 WARN · 0 FAIL |
| Valeurs et leur source | [`../DESIGN-NOTES.md`](../DESIGN-NOTES.md) |

```
typoena-devboard.kicad_sch    racine : les quatre feuilles
├── 01-power.kicad_sch     47 composants   chargeur, rails 3V3 et 5V, rail uSD, bouton
├── 02-mcu.kicad_sch        5              rangées DevKitC, découplage 3V3
├── 03-display.kicad_sch   21              étage de puissance du panneau, FPC, secours
└── 04-io.kicad_sch        13              USB-C ×2, clavier interne, microSD
```

## Figée

`netlist.lock` verrouille son netlist, et `gen_sch.py devboard` s'arrête après
l'avoir vérifié sans réécrire une feuille. Le verrou décrit exactement la carte
fabriquée — référence, symbole, valeur, empreinte, LCSC, MPN, non-monté — pas ce
que le générateur croit. Les deux ont divergé : la carte a été finie dans le GUI,
et `board.py` porte maintenant l'écart dans `REMPLACEMENTS` et `DNP`. Le mécanisme
est décrit dans [`../README.md`](../README.md#le-verrou-netlistlock).

## Ce qu'elle a et que la mainboard n'a pas

- **`J11` / `J12`** — deux rangées 1×22 au pas de 2,54 mm, 22,86 mm entre rangées,
  pour une ESP32-S3-DevKitC-1. Chaque signal garde la GPIO qu'il a sur la
  mainboard : le firmware est le même sur les deux cartes.
- **`J5`** — header 8 points vers une DESPI-C579, secours si l'étage écran intégré
  ne démarre pas. Il a servi : l'étage intégré est validé, et la mainboard part
  sans lui.
- **`J6` sans paires de données** — le pont USB-série est sur la carte de
  développement, le port USB-C n'est donc ici qu'un port de charge. Les 5k1 de CC
  restent : ce sont elles qui déclarent la carte comme consommateur.

Elle n'a **pas** de points de test sur ses GPIO libres : ils sont nés sur la
mainboard.

### Assemblage

En plus des non-montés communs aux deux cartes, JLCPCB n'assemble pas ses trois
headers traversants 2,54 mm : `J5`, `J11` et `J12`.

## Régénérer, contrôler, fabriquer

Tout l'outillage est dans [`../common/`](../common) et les commandes dans
[`../README.md`](../README.md).
