# Typoena — PCBs

Deux projets KiCad 10, même form factor **99 × 45 mm**, 4 couches, et le même
netlist à quelques blocs près.

| Projet | Rôle |
| --- | --- |
| [`devboard/`](devboard/README.md) | carte fabriquée, portant une ESP32-S3-DevKitC-1 sur deux rangées de barrettes. **Figée** — elle décrit le matériel posé sur l'établi |
| [`mainboard/`](mainboard/README.md) | la carte de la machine : module ESP32-S3 soudé, pont USB-série intégré |
| [`common/`](common) | ce que les deux partagent : outillage, symboles maison |

Les valeurs de composants et leur source de datasheet sont communes aux deux
cartes, elles vivent dans [`DESIGN-NOTES.md`](DESIGN-NOTES.md).

## Deux projets, pourquoi deux dossiers

Un projet KiCad, c'est **un** schéma racine et **un** PCB. Les deux cartes ne
peuvent donc pas cohabiter dans un dossier : leurs feuilles hiérarchiques portent
les mêmes noms (`01-power.kicad_sch` … `04-io.kicad_sch`). Chaque carte a son
dossier, et tout le reste remonte dans `common/`.

## Prérequis

Aucun des deux éléments ci-dessous ne vit dans le dépôt : ils s'installent **par
machine**, et les projets ne s'ouvrent pas correctement sans eux.

### KiCad 10

Développé et vérifié avec **KiCad 10.0.5**. Le format de fichier du schéma est
`20250114`, partagé avec KiCad 9, donc un KiCad 9 devrait pouvoir l'ouvrir — mais
la bibliothèque ci-dessous s'installe dans un chemin versionné
(`KICAD10_3RD_PARTY`), donc il faudrait l'y réinstaller séparément.

### Bibliothèque JLCPCB (CDFER)

Elle fournit les symboles et empreintes des composants du catalogue JLCPCB, **avec
les champs `LCSC`, `Part` (MPN), `Class` (Basic/Preferred) et `Stock` déjà
renseignés**. C'est ce qui rend le BOM commandable sans saisir chaque référence à
la main.

Installation par `Outils` → `Gestionnaire de plugins et de contenu` → `Gérer les
dépôts`, ajouter :

```
https://raw.githubusercontent.com/CDFER/cd_fer-kicad-repository/main/repository.json
```

puis installer *JLCPCB KiCad Library* depuis l'onglet `Bibliothèques` (~127 Mo).

| | |
| --- | --- |
| Paquet | `com.github.CDFER.JLCPCB-Kicad-Library` |
| Version installée | **2025.07.18** |
| Licence | MIT — [dépôt](https://github.com/CDFER/JLCPCB-Kicad-Library) |
| Enregistre | `PCM_JLCPCB-*` (18 bibliothèques de symboles) et `PCM_JLCPCB` (empreintes) dans les tables **globales** |

Deux réserves, sans conséquence sur la conception mais bonnes à connaître :

- **Le canal d'installation est en retard sur le dépôt git** — la dernière version
  publiée date de juillet 2025, le dépôt est mis à jour quotidiennement par
  script. Les numéros LCSC et les empreintes ne changent pas ; en revanche les
  champs `Stock`, `Price` et `Class` sont vieux d'un an. Il faut de toute façon
  **revérifier les stocks au moment de commander**, en particulier pour les
  références Extended.
- Le paquet se déclare pour KiCad 8.0. Vérifié ici sous KiCad 10.0.5 : les
  bibliothèques se chargent et se tracent sans erreur.

## `common/` — l'outillage

```
common/
├── netlist.py         les composants que les deux cartes ont à l'identique
├── gen_sch.py         netlist.py + <carte>/board.py -> les 5 fichiers de schéma
├── kisch.py           émission de schéma KiCad 10 (symboles, fils, étiquettes)
├── gen_syms.py        écrit typoena.kicad_sym
├── gen_fab.py         gerbers, perçage, placement et BOM au format JLCPCB
├── check_pcb.py       contrôle qualité du PCB, lecture seule
├── projet.py          chemins d'un projet, résolus depuis son nom
└── typoena.kicad_sym  symboles maison
```

Chaque carte n'a plus qu'un `board.py` : son nom, son UUID racine, et les
composants qu'elle seule porte.

### Bibliothèque de symboles

`common/typoena.kicad_sym` — deux symboles absents des bibliothèques stock,
partagés par les deux cartes. **Aucune empreinte faite main** : tous les
composants utilisent des empreintes stock, relues par l'équipe bibliothèque de
KiCad.

- **BQ25896RTW** — dérivé du `Battery_Management:BQ25895RTW` stock. Les deux
  puces partagent le même WQFN-24-EP 4×4 mais **trois broches diffèrent**
  (2 `PSEL`, 3 `/PG`, 24 `NC` au lieu de `D+`, `D−`, `DSEL`). Le générateur
  contrôle les 25 broches contre la datasheet avant d'écrire, et l'empreinte reste
  celle du symbole d'origine.
- **TPS61023DRL** — dessiné d'après le § *Pin Configuration*, en SOT-563 stock.

Un symbole ne peut se tromper que sur le nom, le numéro et le type électrique de
ses broches — trois choses que l'ERC et la relecture du netlist attrapent. C'est
pourquoi le buck-boost 3V3 a été choisi **parmi les composants dont l'empreinte
existe déjà** (TPS63001 plutôt que TPS63802) : le raisonnement est dans
[`DESIGN-NOTES.md`](DESIGN-NOTES.md).

## Régénérer

```sh
cd hardware/pcb
python3 common/gen_syms.py             # bibliothèque de symboles
python3 common/gen_sch.py mainboard    # schéma : racine + 4 feuilles
kicad-cli sch erc --severity-all -o /tmp/erc.rpt \
  mainboard/typoena-mainboard.kicad_sch
```

`gen_sch.py` réécrit les cinq fichiers de schéma en entier : le relancer après une
retouche dans le GUI **écrase le travail manuel**.

### Le verrou `netlist.lock`

Une carte qui porte un `netlist.lock` est **figée** : `gen_sch.py` vérifie que son
netlist n'a pas bougé, puis s'arrête sans réécrire une seule feuille. Deux raisons
de ne jamais régénérer une carte figée :

- `netlist.py` sert les deux cartes ; une retouche faite pour l'une déplacerait le
  netlist de l'autre en silence. Le verrou la fait échouer ;
- le PCB rattache chaque empreinte à son symbole par un chemin d'UUID, et
  `kisch.uid()` est un uuid4 : une régénération tire de nouveaux UUID et **casse
  tous ces liens**. La mise à jour suivante repose les empreintes et perd le
  placement — c'est le contrôle `H1` de `check_pcb.py` qui surveille ce lien.

Poser le verrou est donc le dernier geste d'une carte terminée, et une carte dont
le PCB existe se verrouille sans attendre qu'elle parte en fabrication :

```sh
python3 common/gen_sch.py mainboard --lock
```

Pour reprendre une carte figée, supprimer son `netlist.lock` — en sachant que les
liens symbole/empreinte du PCB sont alors à refaire.

## Contrôler

```sh
cd hardware/pcb
python3 common/check_pcb.py mainboard                        # pendant le routage
python3 common/check_pcb.py mainboard --profil fabrication   # avant de commander
python3 common/check_pcb.py mainboard -v                     # tous les détails
```

**Lecture seule** : l'outil ne peut pas écrire dans le dépôt. Il rend `PASS` /
`WARN` / `FAIL` par contrôle et sort en erreur s'il reste un `FAIL`.

Il commence par **s'autotester** : la transformation empreinte → pastille est
validée contre la carte (coïncidences piste/pastille, orientation des boîtiers
deux bornes). Si elle ne tient pas, le script s'arrête au lieu de produire des
distances fausses.

Ce qu'il couvre, au-delà de l'ERC et du DRC :

| | |
| --- | --- |
| **A7** | remplissage périmé — un recouvrement zone/piste à 0,000 mm signifie qu'il faut remplir avant de lire quoi que ce soit |
| **B, C** | aucune piste sur In1.Cu, plans d'un seul tenant, **aucune via sur un nœud de commutation** |
| **D** | boucles chaudes et découplage : condensateur → broche, pastille GND → via, et la topologie en té |
| **E, F** | trou de via hors des pastilles CMS ; plafonds d'échappée par boîtier à pas fin, et couverture de la règle `.kicad_dru` |
| **G** | cols trop longs, capacité en courant des rails |
| **H** | lien schéma↔PCB, pastilles sans net — deux défauts qu'aucun outil KiCad ne signale |
| **I, J** | perçages sous le procédé, bandeau de vernis, signaux haute impédance |

Les seuils et leur justification vivent dans le script, au-dessus du contrôle
concerné. Les **budgets de courant sont des hypothèses** à corriger après mesure
au banc.

## Fabriquer

```sh
cd hardware/pcb
python3 common/gen_fab.py mainboard
```

Réexporte la BOM du dépôt depuis le schéma, puis écrit gerbers, perçage,
placement et BOM JLCPCB dans `<carte>/fab/`, qui est ignoré par git.

## Assemblage

Communs aux deux cartes, et portés par le tronc commun :

- **Non montés** — les trois JST-PH du bord (`J1` batterie, `J2` contact bouton,
  `J3` LED), le JST-PH clavier `J13` et le bouton QON `SW1`. Ils se posent à la
  main, donc ils sortent du BOM et du CPL. Chaque carte y ajoute ses headers
  traversants 2,54 mm, que JLCPCB n'assemble pas de toute façon.
- **`R1`, `R10`, `R11`, `R26`, `R27` en 0603** et non en 0402, comme tous les
  autres passifs : ce sont celles qui se reprennent au fer pendant la mise au
  point.
- **`D1`** porte `C19077497` — deux références JLCPCB existent pour le même
  SMF5.0A, c'est celle qu'on a trouvée en stock.

## Classes de nets

Communes aux deux cartes — même `.kicad_dru`, mêmes classes dans le `.kicad_pro`.

| Classe | Piste | Via | Nets |
| --- | --- | --- | --- |
| `Default` | 0,25 mm | 0,6 / 0,3 | tout le reste |
| `Power` | 0,6 mm | 0,8 / 0,4 | VBAT, VSYS, VBUS, PMID, +3V3, +5V, +3V3_SD, REGN |
| `Ground` | 0,6 mm | 0,8 / 0,4 | GND — le seul net de masse du schéma |
| `Switching` | 0,6 mm | 0,8 / 0,4 | les nœuds de commutation des trois convertisseurs et de la pompe de charge du panneau |
| `USB` | 0,25 mm | — | les deux paires D+/D−, en paire différentielle 0,25 / 0,2 |

La classe `Switching` existe pour être **visible en couleur** au routage : ce sont
les nets où la boucle doit être la plus courte possible, et ils ne se rattrapent
pas une fois la carte partie en fabrication.

## Construire sans interface

`kicad-cli` 10 est sur le `PATH` du poste de bureau. Le conteneur exécute KiCad
depuis une AppImage extraite, qui demande son propre environnement :

```sh
K=~/.local/share/com.jean.desktop/tools/kicad9
bash -c "source $K/kicad-env.sh && \$KICAD_CLI sch erc -o /tmp/erc.rpt --severity-all \
  hardware/pcb/mainboard/typoena-mainboard.kicad_sch"
```

Ne pas `source` cet environnement dans un shell qui sert à autre chose — son
`LD_LIBRARY_PATH` casse le `curl` du système.
