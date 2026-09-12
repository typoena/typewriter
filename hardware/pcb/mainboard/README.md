# Typoena mainboard — carte unique PCBA

Une seule carte porte toute l'électronique de la machine : chargeur et power path,
ESP32-S3 soudé, étage de puissance du panneau, µSD, pont USB-série et les deux
ports USB-C.

| | |
| --- | --- |
| Schéma | hiérarchique, 4 feuilles — 106 composants, ERC 0 violation |
| Nets | 79 |
| BOM | 53 lignes, 41 référencées LCSC |
| PCB | 99 × 45 mm, 4 couches — **en cours de routage**, voir plus bas |
| Contrôle | `check_pcb.py mainboard` : 26 PASS · 4 WARN · 0 FAIL |
| Valeurs et leur source | [`../DESIGN-NOTES.md`](../DESIGN-NOTES.md) |

```
typoena-mainboard.kicad_sch   racine : les quatre feuilles
├── 01-power.kicad_sch     47 composants   chargeur, rails 3V3 et 5V, rail uSD, bouton
├── 02-mcu.kicad_sch        9              ESP32-S3-WROOM-1, strapping, découplage
├── 03-display.kicad_sch   20              étage de puissance du panneau, FPC
└── 04-io.kicad_sch        32              USB-C ×2, clavier interne, microSD, CH343P, points de test
```

## Forme de la carte

**99 × 45 mm, coins R3, quatre fixations Ø3,7 mm à 5 mm des coins** — la géométrie
de la [devboard](../devboard/README.md), reprise telle quelle, ports compris. Le
boîtier est construit dessus : `case/typoena-case.scad` pose `pcb_w = 99`,
`pcb_d = 45`, `pcb_r = 3` et ses quatre inserts aux mêmes coordonnées. Une
seule forme à tenir pour les deux cartes, un seul boîtier à imprimer.

| Décision | Raison |
| --- | --- |
| Format allongé | épouse une machine de 176 mm de large et laisse un rectangle propre pour la batterie de 94 × 32 mm. JLCPCB facturant la surface et non la forme, les ~4 400 mm² se paient au même prix sous n'importe quel format de surface égale |
| Module **en débord de bord** | son keepout fait 48 × 41 mm contre 18 × 25,5 mm pour le corps : le faire déborder sort l'essentiel de cette zone stérile de la carte, et c'est la configuration que préfère la datasheet |
| Les 3 ports groupés sur **un bord long** | une seule paroi à percer, et c'est la disposition qui éloigne le plus les convertisseurs à découpage de l'antenne |
| L'écran **n'impose rien** | on conserve le coupleur FFC et la rallonge 100 mm, donc `J4` se place où le routage l'arrange |

Le plancher en largeur est **~35 mm** : le socket µSD est profond de 15,4 mm depuis
le bord et le module large de 18 mm. En dessous il n'y a plus de canal pour router
derrière les connecteurs.

À nos vitesses (SPI 20 MHz, USB full-speed, I²C 400 kHz), l'allongement ne coûte
rien électriquement. Les vrais coûts sont un plan de masse plus étroit — compensé
par la couche dédiée — et une carte qui fléchit, d'où les fixations aux **quatre
coins**.

> :warning: **Commander un deuxième coupleur FFC.** Puisqu'on garde la rallonge, le
> coupleur 24↔24 reste dans le montage — et `hardware/bom.md:50` le note acheté à
> **un seul exemplaire, sans rechange**. C'est la seule pièce non redondée de la
> chaîne.

## Ce qu'elle a et que la devboard n'a pas

- **`U1`** — ESP32-S3-WROOM-1-N16R8 soudé, à la place des rangées DevKitC, avec son
  strapping (`R18`/`C14` sur EN, `R19` sur IO0) et ses boutons `SW2` RESET /
  `SW3` BOOT.
- **`U5`** — CH343P, pont USB-série, sur les paires de données de `J6` : l'USB-C de
  charge est aussi le port de programmation. L'USB natif de l'ESP32-S3 reste câblé
  côté hôte clavier (`J7` / `J13`).
- **`Q5` / `Q6`** — auto-reset croisé façon DevKitC : chaque transistor a sa source
  sur l'**autre** signal, si bien que EN et IO0 ne sont tirés bas que lorsque DTR
  et RTS diffèrent. Les deux asserter ensemble — ce que fait un terminal à
  l'ouverture du port — ne redémarre pas la carte.
- **`J9`** — header UART 6 points, non monté.
- **11 points de test** traversants sur les GPIO libres, voir plus bas.

Elle n'a **pas** le header de secours écran `J5` : la réimplémentation de l'étage
intégré est validée au banc.

## PCB — en cours de routage

Le PCB descend de celui de la devboard : contour, empilage, fixations et position
des trois ports en viennent tels quels.

| | |
| --- | --- |
| Posé | 94 empreintes, 768 segments, 124 vias, 4 zones |
| Plans | GND, PWR_3V3 et PWR_5V d'un seul tenant · **aucune via sur un nœud de commutation** |
| Cuivre en l'air | **0** |
| Connexions restantes | **2** |


L'import se fait par **`Outils` → `Mettre à jour le PCB depuis le schéma`** : KiCad
garantit les nets, les écrire à la main serait strictement plus risqué pour un
résultat identique.

> Le schéma porte son `netlist.lock`. Ne pas le régénérer : les empreintes sont
> rattachées à leurs symboles par un chemin d'UUID, et `gen_sch.py` en tirerait de
> nouveaux. Voir [`../README.md`](../README.md#le-verrou-netlistlock).

### Ce qu'il reste à poser

1. **Déplacer `TP10`** — sa pastille traversante tombe sur `KBD_5V_EN` et
   `SD_PWR_EN`, qui passent dessous sur B.Cu : 4 courts-circuits et 4 ponts de
   vernis. Un trou traversant perce toutes les couches, il ne peut pas se poser
   sur une zone routée
2. Basculer le champ `Value` des points de test sur **F.Silkscreen** (voir plus bas)
3. Les **2 dernières connexions**
4. Remplissage des plans

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

Le 3V3 sort du buck-boost à **~90 %** de rendement. Le BQ25896 intègre tout le
power path — sélection source/batterie, limitation d'entrée, BATFET — donc aucun
MOSFET ni diode discrète sur le chemin de puissance principal.

## Plan de broches

| Signal | GPIO | | Signal | GPIO |
| --- | --- | --- | --- | --- |
| EPD BUSY / RST / DC / CS | 4 / 5 / 6 / 7 | | `I2C_SDA` / `I2C_SCL` | **17 / 18** |
| EPD MOSI / SCK | 11 / 12 | | `PMIC_INT` | **16** |
| SD CS / MISO / SCK / MOSI | 10 / 13 / 14 / 15 | | `PWR_SENSE` (contact bouton) | **21** |
| USB D− / D+ (clavier) | 19 / 20 | | `SD_PWR_EN` | **40** |
| UART0 RX / TX | 44 / 43 | | `KBD_5V_EN` | **41** |
| BOOT | 0 | | `BTN_LED` | **38** |

**`PWR_SENSE` doit rester dans GPIO 0–21.** Sur ESP32-S3, seuls les RTC GPIO
réveillent d'un deep sleep (`ext0` / `ext1`) — et le « off » de cette machine *est*
un deep sleep. Même contrainte pour le reed switch de couvercle, à qui **IO2 est
réservé** (laissé non connecté ici, aucun connecteur n'est prévu à ce stade).

### Points de test

Les GPIO restants ne sortent pas sur une barrette : chacun a son **trou traversant
non monté**, posé où le routage l'arrange. L'empreinte `TestPoint_THTPad_*` se
retire d'elle-même du BOM et du CPL — ni ligne de BOM, ni pose.

**Deux tailles, selon l'usage.**

| | net | empreinte | perçage | pourquoi |
| --- | --- | --- | --- | --- |
| `TP1` | `GND` | Ø2,0 | **Ø1,0** | le retour, sans lequel un point de test de signal ne sert ni à sonder ni à alimenter |
| `TP3` | `+3V3` | Ø2,0 | **Ø1,0** | de quoi alimenter ce qu'on soude |
| `TP4`, `TP5` | `I2C_SDA`, `I2C_SCL` | Ø2,0 | **Ø1,0** | un périphérique I²C se câble sans rien redécider : le bus existe déjà pour le BQ25896, un composant de plus coûte **zéro broche** |
| `TP6`–`TP9` | `IO1`, `IO2`, `IO8`, `IO9` | Ø1,5 | Ø0,7 | les seules **ADC1** disponibles, et les seules capables de **réveiller d'un deep sleep** |
| `TP10`, `TP12`, `TP13` | `IO39`, `IO47`, `IO48` | Ø1,5 | Ø0,7 | numériques ordinaires |

Le Ø1,0 des cinq premiers n'est pas du confort : une broche carrée de 0,64 mm a
**0,9 mm de diagonale**, donc c'est le plus petit perçage qui laisse encore souder
une barrette et y mettre un cavalier. C'est là qu'on branche quelque chose
d'alimenté, et c'est la seule raison de dépenser la place.

Les huit GPIO ne recevront qu'un fil soudé une fois : Ø0,7 passe du 24 AWG à
l'aise et divise l'encombrement par deux.

> **Chaque `TP` doit porter son nom de net sur la sérigraphie.** Son champ `Value`
> l'a déjà ; il reste à le basculer de F.Fab vers F.Silkscreen dans pcbnew
> (`Édition` → `Modifier les propriétés des textes et graphiques`, filtré sur
> `TP*`). Sur une barrette, la position encode l'identité ; un trou isolé sans
> marquage ne dit rien, et ces treize-là sont alors inutilisables.

`IO1`, `IO8` et `IO9` sont les trois ADC1 libres — l'ADC2 est désactivé par le
Wi-Fi, et le reste de l'ADC1 est pris par l'écran et la µSD. `IO2` est RTC comme
eux, et c'est lui qui accueillera le **reed switch de couvercle** le jour venu.

> **`IO3`, `IO45` et `IO46` ne sont délibérément pas sortis.** Ce sont des broches
> de strapping : quelque chose qui les charge au démarrage empêche la carte de
> booter. Les rendre accessibles serait une invitation à un mode de panne difficile
> à diagnostiquer. `IO35`, `IO36` et `IO37` sont pris par la PSRAM octale.

**Il n'y a aucun port de debug matériel sur cette carte.** Le JTAG de l'ESP32-S3
demande `IO39`/`IO40`/`IO41`/`IO42`, or `IO40` porte `SD_PWR_EN` et `IO41`
`KBD_5V_EN` ; et l'USB-Serial/JTAG intégré est sur `IO19`/`IO20`, dépensées pour
l'hôte clavier. `TP10` est donc une simple broche numérique malgré son nom
JTAG, et `IO42` n'est pas sorti du tout — sa broche 35 est déclarée non connectée.

**Contrainte de placement** : garder les `TP` de GPIO **près de `U1`**, en grappe
lâche derrière le module. Pas pour les aligner — chacun va où il veut — mais parce
qu'une piste GPIO non terminée près de l'antenne rayonne. Deux millimètres de
moignon valent mieux que vingt.

## Ce qui reste à faire

- [ ] **Finir le routage.** 2 connexions restantes, plus les 13 points de test
      à importer et poser.
- [ ] **Relire le schéma.** 106 composants, relus par une seule paire d'yeux. L'ERC
      est à zéro et le netlist a été vérifié bloc par bloc, mais sur une carte
      qu'on fait fabriquer c'est le jalon qui compte.
- [ ] **Tracer les fils.** La connectivité passe par des **étiquettes globales**,
      pas par des fils : électriquement équivalent, ERC-propre, netlist identique.
      Mais c'est aride à lire. Les tracer est un travail de souris qui ne change
      pas le netlist — la découpe en feuilles, elle, est faite.
- [ ] **Confirmer les 25 perçages Ø0,2 mm** — les matrices thermiques de U1, U2 et
      U3 — sur
      la page de capacités du fabricant. Seul poste de la carte sous le procédé
      courant, donc le seul susceptible de changer de catégorie tarifaire.
- [ ] **Frais de chargeur à arbitrer.** Tous les passifs sont Basic ou Preferred,
      donc exemptés. Restent ~12 lignes Extended à 3 $ : les 5 ICs (incompressible),
      les 4 inductances (JLCPCB n'a **aucune** inductance de puissance en Basic) et
      les connecteurs.
- [ ] **Revérifier les stocks à la commande**, en particulier les références
      Extended : la bibliothèque JLCPCB installée date de juillet 2025.
- [ ] Reprendre `hardware/bom.md` et `hardware/wiring.md` : ils décrivent le montage
      de banc — devkit et deux perfboards — et non cette carte. Le brochage y est
      celui du DevKitC-1, donc faux pour le module nu.

## Points ouverts, à trancher au banc

- **La protection thermique de la cellule est désactivée.** La cellule EEMB n'a pas
  de NTC, donc `TS` est polarisé par un pont fixe au centre de la fenêtre autorisée.
  C'est la conséquence assumée du choix de cellule — détail et valeurs dans
  [`../DESIGN-NOTES.md`](../DESIGN-NOTES.md).
- **Le courant de charge n'est pas sourcé.** La fiche de la cellule EEMB 103395 n'a
  pas été consultée, et l'`ICHG` par défaut du BQ25896 n'est pas vérifié. `/CE` étant
  à la masse, c'est cette valeur par défaut qui s'applique au premier branchement,
  avant que le firmware ne parle en I²C — donc avant toute programmation possible.
- **Consommation du CH343P en veille.** Son `VIO` est sur le 3V3 permanent (c'est
  l'usage prévu de cette broche) tandis que `VDD5` vient de VBUS. À mesurer : si le
  quiescent est significatif devant les ~84 µA visés, l'alimenter autrement.
- **Luminosité de la LED du bouton.** Spécifiée 12 V, alimentée en 3V3 : elle
  fonctionnera faiblement. La résistance série se règle au bring-up, **sans
  descendre sous 100 Ω** — elle sert aussi à borner une inversion de connecteur.
- **Trois JST-PH identiques.** Batterie, contact et LED sont mutuellement
  enfichables : la protection contre le mésappariement est devenue électrique, pas
  mécanique. Le tableau des cas dans [`../DESIGN-NOTES.md`](../DESIGN-NOTES.md) fait
  partie du cahier des charges du layout (sérigraphie explicite, connecteur batterie
  écarté des deux autres).
- **Détection de carte µSD non câblée** (`DET_A` en l'air). Si elle devient utile,
  IO9 est libre et compatible.

### Budget de veille visé

ESP32-S3 en deep sleep ~10 µA + BQ25896 (BATFET passant, High-Z) 32 µA + TPS63001
40 µA + 2× SSD1683 en deep sleep mode 2 (`0x10`/`0x03`) 2 µA + µSD coupée ≈ **84 µA**,
soit ~5 ans. L'autodécharge de la cellule (~2–3 %/mois, l'équivalent de ~150 µA)
domine largement : en pratique la veille tient **~22 mois**, et c'est elle qui fixe
la limite, pas l'électronique. Tout ce qui passe sous ~50 µA optimise du bruit.

## Régénérer, contrôler, fabriquer

Tout l'outillage est dans [`../common/`](../common) et les commandes dans
[`../README.md`](../README.md).
