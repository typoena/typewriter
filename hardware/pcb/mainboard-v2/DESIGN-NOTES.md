# Typoena mainboard v2 — valeurs de conception et leur source

Chaque valeur non évidente du schéma, avec la page de datasheet dont elle sort. Le
schéma porte les valeurs ; ce fichier porte le *pourquoi*, une seule fois.

## BQ25896RTW — chargeur / power path (datasheet SLUSC76C)

Adresse I²C **0x6B**. Boîtier WQFN-24-EP 4×4, empreinte stock
`Package_DFN_QFN:Texas_RTW_WQFN-24-1EP_4x4mm_P0.5mm_EP2.7x2.7mm_ThermalVias`.

Le symbole est dérivé du `Battery_Management:BQ25895RTW` stock : **trois broches
diffèrent** entre les deux puces, tout le reste est identique.

| Broche | BQ25895 | BQ25896 |
| --- | --- | --- |
| 2 | D+ | **PSEL** |
| 3 | D− | **/PG** |
| 24 | DSEL | **NC** |

Régénérable : `scratchpad/gen_syms.py` contrôle le brochage complet des 25 broches
contre le §6 de la datasheet avant d'écrire.

### Valeurs

| Élément | Valeur | Source |
| --- | --- | --- |
| Inductance SW → SYS | **1 µH** | §10.2 Typical Application |
| C sur VBUS | 1 µF | §10.2 |
| C sur PMID | 10 µF | §10.2 (8,2 µF au schéma type ; 10 µF est la valeur E-standard la plus proche) |
| C sur BTST (vers SW) | 47 nF | broche 21 : « connect the 0.047 µF bootstrap capacitor from SW to BTST » |
| C sur REGN | 4,7 µF / 10 V | broche 22 |
| C sur BAT | 10 µF | broche 13/14 : « connect a 10 µF closely to the BAT pin » |
| C sur SYS | 2 × 10 µF | broche 15/16 : « connect a 20 µF closely to the SYS pin » |
| R sur ILIM | **150 Ω** | `IINMAX = KILIM / RILIM`, KILIM 320…390 A·Ω (§8.5) → plafond 2,4 A typ / 2,6 A max |

**Pourquoi 150 Ω et pas les 260 Ω du schéma type.** Le plafond matériel ne doit pas
être la contrainte qui mord : en charge, l'entrée alimente *à la fois* la charge
(~1,5 A visés) et le système (~0,5 A à 3V3 plus le clavier). 260 Ω plafonnerait à
1,5 A et brimerait la charge dès que la machine tourne. Le firmware fixe la vraie
limite par `IINLIM` (REG00) selon l'alimentation branchée.

### Broches de configuration câblées en dur

| Broche | Niveau | Raison |
| --- | --- | --- |
| PSEL (2) | **GND** | déclare une alimentation « adapter » et non un port USB hôte : limite par défaut plus haute |
| OTG (8) | **GND** | l'OTG est délibérément inutilisé — le 5 V clavier vient du TPS61023. Le maintenir bas garantit que le boost ne peut pas démarrer |
| /CE (9) | **GND** | charge active par défaut : **la carte charge sans une seule ligne de firmware** |
| TS (11) | pont **10 k (REGN) / 12 k (GND)** | voir ci-dessous |

**Le pont sur TS.** La cellule EEMB n'expose pas de thermistance. Le BQ25896 suspend
la charge si TS sort de sa fenêtre, donc il faut le polariser dans la plage
« température normale » : entre `V(T3)` = 44,75 % et `V(T2)` = 68,25 % de REGN pour
une charge à plein régime (§8.5, JEITA Thermistor Comparator). Le pont 10 k/12 k met
TS à **54,5 %** de REGN, bien au centre. REGN étant coupé hors charge (§9.2.6 : « the
REGN LDO stays off to minimize the quiescent current »), ce pont **ne consomme rien en
veille** — ce qui compte, vu que le budget de veille est le critère de conception.

> :warning: Le revers : la protection thermique de la cellule est désactivée. C'est la
> conséquence assumée d'une cellule sans NTC. Si un jour la cellule en expose une, le
> pont se remplace par la thermistance et le firmware n'a rien à changer.

## TPS63001DRCR — buck-boost 3V3 (datasheet SLVS520)

Sortie **fixe 3,3 V**, VSON-10-EP 3×3 (boîtier DRC). Symbole **et** empreinte stock :
`Regulator_Switching:TPS63001` + `Package_SON:Texas_DRC0010J_ThermalVias`.

| Élément | Valeur | Source |
| --- | --- | --- |
| Inductance L1↔L2 | **2,2 µH** | §8.2, Table 1 (VLF4012-2R2) |
| C entrée | 10 µF | §8.2 |
| C sortie | **2 × 10 µF** | §8.2 |

> :warning: Ne pas « améliorer » ce filtre. §8.2.1 : *« The TPS63000 series have internal
> loop compensation. Therefore, the external LC filter has to be selected according to
> the internal compensation. »* Mettre 22 µF parce qu'on en a en stock déstabilise la
> boucle.

| Broche | Niveau | Raison |
| --- | --- | --- |
| EN (6) | **VSYS** | le 3V3 doit monter sans firmware et ne jamais être coupé — le MCU dort dessus |
| VINA (8) | **VSYS** | alimentation de l'étage de contrôle |
| PS/SYNC (7) | **GND** | 0 = mode économie d'énergie actif, ce qui donne le quiescent annoncé |
| FB (10) | **VOUT** | §8.2.1 : sur la version à sortie fixe, FB **doit** être relié à VOUT. Pas de pont de contre-réaction |

### Pourquoi pas le TPS63802, qui était meilleur sur le papier

Le TPS63802 fait 11 µA de quiescent et 2 A sur toute la plage, contre 40 µA typ
(50 max) et 800 mA en mode boost ici. Il a été écarté parce que son boîtier VSON-HR
(DLA0010A) n'existe dans aucune bibliothèque KiCad : il aurait fallu dessiner
l'empreinte à la main, et **une empreinte faite main est le seul artefact d'un schéma
qu'aucun outil ne peut valider** — une numérotation de pastilles inversée passe l'ERC,
le DRC et la relecture des Gerbers, et ne se découvre qu'à la mise sous tension.

Ce qu'on a payé pour supprimer ce risque :

| | TPS63802 | TPS63001 | Effet réel |
| --- | --- | --- | --- |
| Quiescent | 11 µA | 40 µA | veille 25 → 22 mois (l'autodécharge de la cellule, ~150 µA, domine de toute façon) |
| Sortie, mode buck | 2 A | 1200 mA | pic mesuré ~600–700 mA → ~2× de marge |
| Sortie, mode boost | 2 A | **800 mA** | le mode boost ne s'enclenche que sous VSYS 3,3 V, soit **sous le seuil de coupure basse** qu'on implémente déjà |
| Prix ×1 | ~1,3 $ | 1,73 $ | |

En prime, la sortie fixe supprime le pont 511 k / 91 k et sa dérive.

## TPS61023DRL — boost 5 V clavier (datasheet SLVSF14B)

| Élément | Valeur | Source |
| --- | --- | --- |
| Inductance VSYS→SW | **1 µH** | §8.2 Typical Application |
| C entrée | 10 µF | §8.2 |
| C sortie | 2 × 22 µF | §8.2 |
| Pont FB, R1 (VOUT→FB) | **750 kΩ** 1 % | calcul ci-dessous |
| Pont FB, R2 (FB→GND) | **100 kΩ** 1 % | idem |

VREF = 595 mV (PWM) → 0,595 × (1 + 750/100) = **5,06 V**, dans la fenêtre USB
4,75–5,25 V. Les ~6 µA du pont sont sur le rail 5 V, coupé en veille : sans effet sur
le budget.

`EN` (2) porte un **pulldown 100 kΩ** : le rail clavier est **éteint par défaut**, y
compris GPIO flottant et pendant le deep sleep.

## Les quatre inductances de puissance

Aucune n'existe en Basic — **JLCPCB n'a aucune inductance de puissance dans sa
bibliothèque Basic**, ni la bibliothèque CDFER qui la suit. Ce sont donc quatre lignes
Extended, dont une mutualisée.

| Réf | Valeur | Référence | LCSC | Empreinte |
| --- | --- | --- | --- | --- |
| L1 (BQ25896) | 1 µH, 4,15 A, 18 mΩ | Sunlord SWPA4030S1R0NT | C42193 | `L_APV_ANR4030` |
| L3 (TPS61023) | **même référence que L1** | — | C42193 | `L_APV_ANR4030` |
| L2 (TPS63001) | 2,2 µH, 2,9 A, 44 mΩ | Bourns SRN4018-2R2M | C913207 | `L_Bourns-SRN4018` |
| L4 (panneau) | 47 µH, 1,2 A, 200 mΩ | Sunlord SWPA6045S470MT | C36414 | `L_APV_ANR6045` |

Mutualiser L1 et L3 sur la même référence économise une ligne de BOM, soit 3 $ de frais
de chargeur.

### Pourquoi une empreinte « APV » pour un composant Sunlord

C'est déroutant à la lecture, donc autant l'écrire : `L_APV_ANR4030` porte le nom d'une
série d'un autre fabricant, mais son motif est **identique au millième près** à celui que
Sunlord recommande pour le SWPA4030S — pastilles **1,10 × 3,70 mm, entraxe 3,00 mm**.
Idem pour `L_APV_ANR6045` face au SWPA6045S : **1,70 × 5,70 mm, entraxe 4,50 mm**. Les
deux séries partagent le même code de taille (4030 = 4,0 × 4,0 × 3,0 mm) et donc la même
implantation.

La correspondance a été établie **par calcul** — en analysant les 692 empreintes
d'inductances de KiCad et en comparant taille de pastille et entraxe aux cotes `a`/`b`/`c`
du tableau *Recommended Land Pattern* de la datasheet SWPA — et non à l'œil. Ça évite de
dessiner deux empreintes à la main.

> :warning: **L'erreur à ne pas reproduire.** Ces quatre inductances étaient initialement
> spécifiées en 0805 et 1210. À 1 µH ou 2,2 µH, un boîtier 0805 contient une inductance
> **de signal calibrée pour ~50 mA**, pas les 1 à 4 A de nos rails. Ni l'ERC ni la
> relecture du netlist ne peuvent voir ce genre d'erreur : elle ne vit que dans le choix
> d'empreinte. Les vraies inductances font 4 × 4 et 6 × 6 mm, ce qui pèse sur le budget
> de surface de la carte.

## Panneau GDEY0579T93 — étage de puissance (datasheet §9 Reference Circuit)

Recopie littérale du circuit publié. Le SSD1683 est en COG sur la dalle et pilote un
étage à découpage **externe** : il sort la commande de grille (`GDR`, broche 2) et lit
le courant (`RESE`, broche 3) ; l'inductance, le MOSFET et les diodes sont à nous.

| Réf | Valeur |
| --- | --- |
| L | 47 µH / 500 mA |
| Q | Si1308EDL (SOT-323) |
| D ×3 | **B5819W** (SOD-123) — substitution, voir ci-dessous |
| R grille | 1 MΩ |
| R shunt | 2,2 Ω |
| R sur TSCL / TSDA | 10 kΩ ×2 |
| C sur 3V3 | 4,7 µF / 25 V |
| C sur VDHR, VGH, VSL, VGL, VCOM, VSH, VDD, VCI, PREVGH, PREVGL | 1 µF / 25 V ×9 |

`BS` (broche 8) à la masse : sélectionne le SPI 4 fils. Alimenté depuis le **3V3**,
aucun rail supplémentaire.

### La seule substitution par rapport au circuit de référence

Les trois MBR0530 sont remplacées par des **B5819W** (`C8598`). Motif : la MBR0530 est
Extended chez JLCPCB, la B5819W est **Basic** — 3 $ de frais de chargeur en moins et
un stock de 648 000 pièces au lieu de 87 000.

Elle est mieux dimensionnée sur les trois axes qui comptent ici : 40 V au lieu de 30,
1 A au lieu de 0,5, même boîtier SOD-123. La tension directe est comparable (600 mV
à 1 A contre 550 mV à 500 mA), et aux courants réels de cette pompe de charge —
quelques dizaines de mA — les deux tournent autour de 300 mV. C'est cette tension qui
fixe l'écart entre les rails ±15 V théoriques et réels, donc c'est le paramètre à
surveiller au banc si le contraste du panneau surprend.

Aucune autre substitution : le **Si1308EDL** est conservé tel quel malgré son statut
Extended. Le SSD1683 pilote sa grille avec un driver faible, et le choix d'origine tient
à sa très faible charge de grille (1,4 nC). Le 2N7002W en SOT-323, Preferred et dix fois
moins cher, ne commute que 115 mA contre 1,5 A — ce n'est pas un substitut.

## Bloc bouton

Les trois connecteurs off-board sont en **JST-PH 2,0 mm 2 points** (batterie, contact,
LED) — c'est le format en stock. Ce choix abandonne la règle anti-mésappariement de
`docs/bom.md:40` : la protection devient électrique.

| Erreur de branchement | Protection |
| --- | --- |
| Batterie → connecteur contact | 10 kΩ série → ~420 µA dans la diode de clamp du GPIO |
| Batterie → connecteur LED | **résistance série 100 Ω, jamais 0 Ω** → ~40 mA au lieu d'un court sur le 3V3 |
| Batterie inversée → connecteur LED | même résistance, via la diode de corps du 2N7002 |
| Contact ↔ LED intervertis | inoffensif dans les deux sens |

LED du bouton mesurée : s'allume à 3 V comme à 5 V, tension nominale annoncée 12 V —
elle embarque donc presque certainement sa propre résistance dimensionnée pour 12 V.
Alimentée en 3V3 elle tirera ~1 mA et **sera peu lumineuse** ; c'est assumé, la valeur
de la résistance série se règle au bring-up sans jamais descendre sous 100 Ω.

## Contraintes de brochage MCU

`PWR_SENSE` et le reed switch de couvercle doivent être dans **GPIO 0–21** : sur
ESP32-S3, seuls les RTC GPIO réveillent d'un deep sleep (`ext0` / `ext1`). C'est la
seule contrainte de brochage qui ne se voit pas sur un schéma.
