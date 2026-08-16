# Typoena mainboard v2 — carte unique PCBA

Projet KiCad 10 **neuf**, indépendant de [`../mainboard/`](../mainboard) (KiCad 9), qui
reste en place et non modifié comme référence. Rien à migrer, donc rien à casser.

| | |
| --- | --- |
| Schéma | hiérarchique, 4 feuilles — **97 composants, ERC 0 violation** |
| Nets | 79, 342 connexions |
| BOM | 45 lignes, **42 référencées LCSC** (les 3 autres sont des connecteurs non montés) |
| PCB | **squelette prêt** — 130 × 45 mm, 4 couches, DRC 0 violation. Empreintes à importer |
| Valeurs et leur source | [`DESIGN-NOTES.md`](DESIGN-NOTES.md) |

```
typoena-mainboard-v2.kicad_sch   racine : les quatre feuilles
├── 01-power.kicad_sch     47 composants   chargeur, rails 3V3 et 5V, rail uSD, bouton
├── 02-mcu.kicad_sch       10              ESP32-S3, strapping, découplage
├── 03-display.kicad_sch   21              étage de puissance du panneau, FPC, secours
└── 04-io.kicad_sch       19              USB-C ×2, microSD, pont USB-série, extension J10
```

Ce que la carte remplace : les deux perfboards, le devkit, et les trois modules externes
(HW-373, MT3608, DESPI-C579). Ce qu'elle apporte : lecture du niveau de batterie, charge
propre en usage, coupure sous tension critique, extinction soft.

## Prérequis

Ce projet ne s'ouvre pas correctement sans les deux éléments ci-dessous. Aucun des deux
ne vit dans le dépôt : ils s'installent **par machine**.

### KiCad 10

Développé et vérifié avec **KiCad 10.0.5**. Le format de fichier du schéma est
`20250114`, partagé avec KiCad 9, donc un KiCad 9 devrait pouvoir l'ouvrir — mais la
bibliothèque ci-dessous s'installe dans un chemin versionné (`KICAD10_3RD_PARTY`), donc
il faudrait l'y réinstaller séparément.

> :warning: **Ne pas ouvrir l'ancien projet [`../mainboard/`](../mainboard) avec KiCad 10.**
> Il est en KiCad 9 et la migration est irréversible. C'est précisément pour ça que la v2
> est un projet neuf plutôt qu'une évolution.

### Bibliothèque JLCPCB (CDFER)

Elle fournit les symboles et empreintes des composants du catalogue JLCPCB, **avec les
champs `LCSC`, `Part` (MPN), `Class` (Basic/Preferred) et `Stock` déjà renseignés**.
C'est ce qui rend le BOM commandable sans saisir chaque référence à la main.

Installation par `Outils` → `Gestionnaire de plugins et de contenu` → `Gérer les dépôts`,
ajouter :

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

- **Le canal d'installation est en retard sur le dépôt git** — la dernière version publiée
  date de juillet 2025, le dépôt est mis à jour quotidiennement par script. Les numéros
  LCSC et les empreintes ne changent pas ; en revanche les champs `Stock`, `Price` et
  `Class` sont vieux d'un an. Il faut de toute façon **revérifier les stocks au moment de
  commander**, en particulier pour les références Extended.
- Le paquet se déclare pour KiCad 8.0. Vérifié ici sous KiCad 10.0.5 : les bibliothèques
  se chargent et se tracent sans erreur.

## Forme de la carte — décidé

**130 × 45 mm, 4 couches.** Le boîtier sera adapté à la carte, et non l'inverse : c'est
ce qui a débloqué le layout et permis d'optimiser pour la carte plutôt que pour une
cavité existante.

| Décision | Raison |
| --- | --- |
| Format allongé 130 × 45 | épouse une machine de 176 mm de large et laisse un rectangle propre pour la batterie de 94 × 32 mm. À surface égale (~5 800 mm²) le PCB coûte le même prix qu'un 90 × 60 — JLCPCB facture la surface, pas la forme |
| Module **en débord de bord** | son keepout fait 48 × 41 mm contre 18 × 25,5 mm pour le corps : le faire déborder sort l'essentiel de cette zone stérile de la carte, et c'est la configuration que préfère la datasheet. La contrainte 50 × 70 de la v1 l'interdisait |
| Les 3 ports utilisateur groupés sur **un bord long** | une seule paroi à percer, et c'est la disposition qui éloigne le plus les convertisseurs à découpage de l'antenne |
| L'écran **n'impose rien** | on conserve le coupleur FFC et la rallonge 100 mm, donc `J4` se place où le routage l'arrange |

Le plancher en largeur est **~35 mm** : le socket µSD est profond de 15,4 mm depuis le
bord et le module large de 18 mm. En dessous il n'y a plus de canal pour router derrière
les connecteurs.

À nos vitesses (SPI 20 MHz, USB full-speed, I²C 400 kHz), l'allongement ne coûte rien
électriquement. Les vrais coûts sont un plan de masse plus étroit — compensé par la
couche dédiée — et une carte de 130 mm qui fléchit, d'où **5 à 6 points de fixation**
au lieu de 4.

> :warning: **Commander un deuxième coupleur FFC.** Puisqu'on garde la rallonge, le
> coupleur 24↔24 reste dans le montage — et `hardware/bom.md:50` le note acheté à **un seul
> exemplaire, sans rechange**. C'est la seule pièce non redondée de la chaîne.

## PCB — squelette

`typoena-mainboard-v2.kicad_pcb` contient le contour (130 × 45, coins R3), l'**empilage
JLCPCB 4 couches** (1,6 mm, finition ENIG), **6 trous de fixation** M2 — les quatre coins
plus deux à mi-portée, parce qu'une carte de 130 mm fléchit — et les **classes de nets**.
DRC à 0 violation.

Il ne contient **délibérément aucune empreinte**. KiCad les importe lui-même par
`Outils` → `Mettre à jour le PCB depuis le schéma`, en un clic, avec les nets garantis
corrects par son propre importeur. Les écrire à la main aurait été strictement plus
risqué pour un résultat identique.

### Classes de nets

| Classe | Piste | Via | Nets |
| --- | --- | --- | --- |
| `Default` | 0,25 mm | 0,6 / 0,3 | tout le reste |
| `Power` | 0,6 mm | 0,8 / 0,4 | GND, VBAT, VSYS, VBUS, PMID, +3V3, +5V, +3V3_SD, REGN |
| `Switching` | 0,6 mm | 0,8 / 0,4 | les nœuds de commutation des trois convertisseurs et de la pompe de charge du panneau |
| `USB` | 0,25 mm | — | les deux paires D+/D−, en paire différentielle 0,25 / 0,2 |

La classe `Switching` existe pour être **visible en couleur** au routage : ce sont les
nets où la boucle doit être la plus courte possible, et ils ne se rattrapent pas une
fois la carte partie en fabrication.

### Ordre de travail au layout

1. Les **connecteurs** sur le bord long — ils fixent tout le reste
2. Le **module en débord**, au bord opposé, keepout hors carte
3. Les **trois blocs de puissance**, boucles de commutation compactes en premier
4. Le reste, puis les plans de masse

## Architecture

```
USB-C charge ──VBUS──┬─► BQ25896 ──BAT──► [JST-PH] LiPo 3700 mAh
   (2× 5k1 Rd sur CC)│      │  L 1µH
                     │      └──SYS──┬──► TPS63001 ──► 3V3  (MCU, EPD, µSD, CH343P)
                     └─► CH343P     │
                        (D+/D−)     └──► TPS61023 ──► 5V ──► VBUS clavier
                                          EN ← IO41           (USB-C, 2× 56k Rp sur CC)
3V3 ──► AO3401A ──► 3V3_SD ──► µSD
        EN ← IO40
3V3 ──► L 47µH + Si1308EDL + 3× MBR0530 ──► PREVGH / PREVGL ──► FPC 24p ──► panneau
```

Le rendement vers le 3V3 passe de **56 %** (boost 5 V puis LDO) à **~90 %**. Tout le
power path discret de la v1 — deux AO3401A, un 2N7002, SS34, 1N4148 et leur réseau de
polarisation — disparaît dans le BQ25896.

## Plan de broches

| Signal | GPIO | | Signal | GPIO |
| --- | --- | --- | --- | --- |
| EPD BUSY / RST / DC / CS | 4 / 5 / 6 / 7 | | `I2C_SDA` / `I2C_SCL` | **17 / 18** |
| EPD MOSI / SCK | 11 / 12 | | `PMIC_INT` | **16** |
| SD CS / MISO / SCK / MOSI | 10 / 13 / 14 / 15 | | `PWR_SENSE` (contact bouton) | **21** |
| USB D− / D+ (clavier) | 19 / 20 | | `SD_PWR_EN` | **40** |
| UART0 RX / TX | 44 / 43 | | `KBD_5V_EN` | **41** |
| BOOT | 0 | | `BTN_LED` | **38** |
| | | | WS2812 | **48** |

**`PWR_SENSE` doit rester dans GPIO 0–21.** Sur ESP32-S3, seuls les RTC GPIO réveillent
d'un deep sleep (`ext0` / `ext1`) — et le « off » de cette machine *est* un deep sleep.
Même contrainte pour le reed switch de couvercle, à qui **IO2 est réservé** (laissé
non connecté ici, aucun connecteur n'est prévu à ce stade).

### Connecteur d'extension `J10`

Les 7 GPIO restants sortent sur un header 1×12 au pas de 2,54 mm, **non monté** : ce
sont des trous métallisés, donc ni ligne de BOM, ni frais de chargeur, ni pose. On y
soude un header ou directement un fil, plus tard, sans rien redessiner.

| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GND | 3V3 | SDA | SCL | IO1 | IO2 | IO8 | IO9 | IO39 | IO42 | IO47 | GND |

Les quatre premières broches suivent l'ordre **Qwiic / STEMMA QT** (GND, 3V3, SDA, SCL) :
un périphérique I²C se câble dessus sans rien décoder. Et l'I²C est le chemin
d'extension le moins cher qui soit — le bus existe déjà pour le BQ25896, un composant
de plus coûte **zéro broche**.

`IO1`, `IO8` et `IO9` sont les trois ADC1 (les seuls utilisables, l'ADC2 étant
désactivé par le Wi-Fi). `IO2` est RTC, donc capable de réveiller d'un deep sleep —
c'est lui qui accueillera le reed switch de couvercle le jour venu.

> **`IO3`, `IO45` et `IO46` ne sont délibérément pas sortis.** Ce sont des broches de
> strapping : quelque chose qui les charge au démarrage empêche la carte de booter.
> Les rendre accessibles serait une invitation à un mode de panne difficile à
> diagnostiquer.

Hors limites : `26–37` (flash + PSRAM octal), `43/44` (UART console), `0/3/45/46`
(strapping).

**Contrainte de placement** : `J10` doit être posé **loin du keepout antenne** de
48 × 21 mm. Des pistes GPIO non terminées à proximité du module sont des antennes
parasites — c'est là qu'est le vrai coût de ce connecteur, dans le routage, pas dans
les trous.

## Bibliothèque locale

`typoena.kicad_sym` — deux symboles absents des bibliothèques stock. **Aucune empreinte
faite main** : tous les composants utilisent des empreintes stock, relues par l'équipe
bibliothèque de KiCad.

- **BQ25896RTW** — dérivé du `Battery_Management:BQ25895RTW` stock. Les deux puces
  partagent le même WQFN-24-EP 4×4 mais **trois broches diffèrent** (2 `PSEL`, 3 `/PG`,
  24 `NC` au lieu de `D+`, `D−`, `DSEL`). Le générateur contrôle les 25 broches contre
  la datasheet avant d'écrire, et l'empreinte reste celle du symbole d'origine.
- **TPS61023DRL** — dessiné d'après le § *Pin Configuration*, en SOT-563 stock.

Un symbole ne peut se tromper que sur le nom, le numéro et le type électrique de ses
broches — trois choses que l'ERC et la relecture du netlist attrapent. C'est pourquoi le
buck-boost 3V3 a été choisi **parmi les composants dont l'empreinte existe déjà**
(TPS63001 plutôt que TPS63802) : le raisonnement est dans
[`DESIGN-NOTES.md`](DESIGN-NOTES.md).

## Régénérer

Le schéma a été produit par les scripts de [`tools/`](tools) plutôt qu'à la souris.
C'est un **amorçage ponctuel, pas un pipeline** : dès que le schéma est retouché dans
le GUI, les scripts deviennent une archive de la façon dont il a été créé, et il ne
faut plus les relancer sous peine d'écraser le travail manuel.

```sh
cd hardware/pcb/mainboard-v2
python3 tools/gen_syms.py     # bibliothèque de symboles
python3 tools/gen_sch.py      # schéma
kicad-cli sch erc --severity-all -o /tmp/erc.rpt typoena-mainboard-v2.kicad_sch
```

`kicad-cli` 10.0.5 est dans le `PATH` sur cette machine. La recette headless de
[`../README.md`](../README.md) pointe vers un AppImage KiCad 9 sous
`~/.local/share/com.jean.desktop/`, qui n'existe pas ici.

## Ce qui reste à faire

- [ ] **Relire le schéma.** 96 composants, relus par une seule paire d'yeux. L'ERC est à
      zéro et le netlist a été vérifié bloc par bloc, mais sur une première carte c'est
      le jalon qui compte — et il coûte incomparablement moins cher qu'après fabrication.
- [ ] **Tracer les fils.** La connectivité passe par des **étiquettes globales**, pas par
      des fils : électriquement équivalent, ERC-propre, et le netlist est identique à ce
      qu'il serait avec des fils. Mais c'est aride à lire. Les tracer est un travail de
      souris qui ne change pas le netlist — la découpe en feuilles, elle, est faite.
- [ ] **Layout 4 couches** `SIG/GND/PWR/SIG` sur 130 × 45 mm. **Plus bloqué** — le
      boîtier suivra la carte. Ordre : connecteurs sur le bord long d'abord, module en
      débord au bord opposé, puis les trois blocs de puissance, puis le routage en
      commençant par les boucles de commutation (elles ne se rattrapent pas après).
- [ ] **Connecter la pastille mécanique `MP`** du connecteur FPC à la masse au layout :
      le symbole générique `Conn_01x24` n'a pas de broche pour elle.
- [ ] **Frais de chargeur à arbitrer.** Tous les passifs sont Basic ou Preferred, donc
      exemptés. Restent ~12 lignes Extended à 3 $ : les 5 ICs (incompressible), les
      4 inductances (JLCPCB n'a **aucune** inductance de puissance en Basic), les
      connecteurs, et le WS2812B — ce dernier ne sert qu'à une étape de QC et peut être
      supprimé si l'on veut économiser une ligne.
- [ ] **Revérifier les stocks à la commande**, en particulier les références Extended :
      la bibliothèque JLCPCB installée date de juillet 2025.
- [ ] Reprendre `docs/bom.md` et `../README.md`, qui décrivent encore la v1 comme la
      carte de référence.

## Points ouverts, à trancher au banc

- **La protection thermique de la cellule est désactivée.** La cellule EEMB n'a pas de
  NTC, donc `TS` est polarisé par un pont fixe au centre de la fenêtre autorisée. C'est
  la conséquence assumée du choix de cellule — détail et valeurs dans
  [`DESIGN-NOTES.md`](DESIGN-NOTES.md).
- **Consommation du CH343P en veille.** Son `VIO` est sur le 3V3 permanent (c'est
  l'usage prévu de cette broche) tandis que `VDD5` vient de VBUS. À mesurer : si le
  quiescent est significatif devant les ~84 µA visés, l'alimenter autrement.

### Budget de veille visé

ESP32-S3 en deep sleep ~10 µA + BQ25896 (BATFET passant, High-Z) 32 µA + TPS63001
40 µA + 2× SSD1683 en deep sleep mode 2 (`0x10`/`0x03`) 2 µA + µSD coupée ≈ **84 µA**,
soit ~5 ans. L'autodécharge de la cellule (~2–3 %/mois, l'équivalent de ~150 µA) domine
largement : en pratique la veille tient **~22 mois**, et c'est elle qui fixe la limite,
pas l'électronique. Tout ce qui passe sous ~50 µA optimise du bruit.
- **Luminosité de la LED du bouton.** Spécifiée 12 V, alimentée en 3V3 : elle
  fonctionnera faiblement. La résistance série se règle au bring-up, **sans descendre
  sous 100 Ω** — elle sert aussi à borner une inversion de connecteur.
- **Trois JST-PH identiques.** Batterie, contact et LED sont mutuellement enfichables :
  la protection contre le mésappariement est devenue électrique, pas mécanique. Le
  tableau des cas dans `DESIGN-NOTES.md` fait partie du cahier des charges du layout
  (sérigraphie explicite, connecteur batterie écarté des deux autres).
- **Détection de carte µSD non câblée** (`DET_A` en l'air). Si elle devient utile,
  IO9 est libre et compatible.
