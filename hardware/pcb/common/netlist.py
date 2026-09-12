#!/usr/bin/env python3
"""Tronc commun du schéma : tout ce que les deux cartes portent à l'identique.

Chaque carte ajoute son delta dans son `board.py` — c'est `gen_sch.py` qui
assemble les deux et émet les feuilles.

Connectivité par étiquettes (voir kisch.py). Le schéma est plat : la découpe en
feuilles hiérarchiques se fait ensuite dans le GUI, elle ne change pas le netlist.
"""
import os

import kisch
from kisch import LIB, uid, wire, label, text, symbol_instance, pin_abs, HEADER

# empreintes courantes
R04 = "Resistor_SMD:R_0402_1005Metric"
C04 = "Capacitor_SMD:C_0402_1005Metric"
C06 = "Capacitor_SMD:C_0603_1608Metric"
C08 = "Capacitor_SMD:C_0805_2012Metric"
R06 = "Resistor_SMD:R_0603_1608Metric"
R08 = "Resistor_SMD:R_0805_2012Metric"
SOT23 = "Package_TO_SOT_SMD:SOT-23"
SOT323 = "Package_TO_SOT_SMD:SOT-323_SC-70"
SOD123 = "Diode_SMD:D_SOD-123"
PH2 = "Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal"
PH4 = "Connector_JST:JST_PH_S4B-PH-K_1x04_P2.00mm_Horizontal"
HDR22 = "Connector_PinHeader_2.54mm:PinHeader_1x22_P2.54mm_Vertical"
USBC = "Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12"


class Netlist:
    """Accumulateur de composants, dans l'ordre de déclaration."""

    def __init__(self):
        self.comps = []

    def C(self, ref, lib_id, value, fp, pins, **props):
        self.comps.append([ref, lib_id, value, fp, pins, props])


# =====================================================================  ALIMENTATION

def alimentation(n):
    C = n.C
    C("U2", "typoena:BQ25896RTW", "BQ25896RTW",
      "Package_DFN_QFN:Texas_RTW_WQFN-24-1EP_4x4mm_P0.5mm_EP2.7x2.7mm_ThermalVias",
      {"1": "VBUS", "2": "GND", "3": "PMIC_PG", "4": "PMIC_STAT", "5": "I2C_SCL",
       "6": "I2C_SDA", "7": "PMIC_INT", "8": "GND", "9": "GND", "10": "PMIC_ILIM",
       "11": "PMIC_TS", "12": "PMIC_QON", "13": "VBAT", "15": "VSYS", "17": "GND",
       "19": "PMIC_SW", "21": "PMIC_BTST", "22": "PMIC_REGN", "23": "PMID"},
      LCSC="C181475", MPN="BQ25896RTWR")
    C("L1", "Device:L", "1uH 4.15A", "Inductor_SMD:L_APV_ANR4030",
      {"1": "PMIC_SW", "2": "VSYS"}, LCSC="C42193", MPN="SWPA4030S1R0NT")
    C("C1", "Device:C", "1uF", C06, {"1": "VBUS", "2": "GND"})
    C("C2", "Device:C", "10uF", C08, {"1": "PMID", "2": "GND"})
    C("C3", "Device:C", "47nF", C06, {"1": "PMIC_BTST", "2": "PMIC_SW"})
    C("C4", "Device:C", "4.7uF", C08, {"1": "PMIC_REGN", "2": "GND"})
    C("C5", "Device:C", "10uF", C08, {"1": "VBAT", "2": "GND"})
    C("C6", "Device:C", "10uF", C08, {"1": "VSYS", "2": "GND"})
    C("C7", "Device:C", "10uF", C08, {"1": "VSYS", "2": "GND"})
    # R1, R10/R11 et R26/R27 sont en 0603 et non en 0402 : elles se reprennent au
    # fer pendant la mise au point.
    C("R1", "Device:R", "150R", R06, {"1": "PMIC_ILIM", "2": "GND"})
    C("R2", "Device:R", "10k", R04, {"1": "PMIC_REGN", "2": "PMIC_TS"})
    C("R3", "Device:R", "12k", R04, {"1": "PMIC_TS", "2": "GND"})
    C("R4", "Device:R", "10k", R04, {"1": "+3V3", "2": "PMIC_PG"})
    C("R5", "Device:R", "10k", R04, {"1": "+3V3", "2": "PMIC_STAT"})
    C("R6", "Device:R", "10k", R04, {"1": "+3V3", "2": "PMIC_INT"})
    C("SW1", "Switch:SW_Push", "QON", "Button_Switch_SMD:SW_SPST_B3U-1000P",
      {"1": "PMIC_QON", "2": "GND"})
    # TVS unidirectionnelle : symbole zener, cathode (broche 1) sur VBUS.
    # Device:D_TVS est bidirectionnel — broches A1/A2, aucune polarité
    # exprimée, donc rien que l'ERC puisse refuser si la cathode part à la
    # masse. Contrôle H3 de check_pcb.py.
    C("D1", "Device:D_Zener", "SMF5.0A", SOD123, {"1": "VBUS", "2": "GND"})
    C("J1", "Connector_Generic:Conn_01x02", "BATTERIE JST-PH", PH2,
      {"1": "VBAT", "2": "GND"})

    # --- buck-boost 3V3
    # Version a sortie FIXE 3,3 V : FB se cable sur VOUT (§8.2.1 « for the fixed output
    # voltage option the feedback pin needs to be connected to VOUT »), donc aucun pont
    # de contre-reaction. PS/SYNC a la masse = mode economie d'energie actif.
    C("U3", "Regulator_Switching:TPS63001", "TPS63001DRCR",
      "Package_SON:Texas_DRC0010J_ThermalVias",
      {"1": "+3V3", "2": "L_3V3_B", "3": "GND", "4": "L_3V3_A", "5": "VSYS",
       "6": "VSYS", "7": "GND", "8": "VSYS", "9": "GND", "10": "+3V3", "11": "GND"},
      LCSC="C28060", MPN="TPS63001DRCR")
    C("L2", "Device:L", "2.2uH 2.9A", "Inductor_SMD:L_Bourns-SRN4018",
      {"1": "L_3V3_A", "2": "L_3V3_B"}, LCSC="C913207", MPN="SRN4018-2R2M")
    # Le filtre LC doit suivre la compensation interne (§8.2.1) : on reprend les valeurs
    # du schema type, 10 uF en entree et 2 x 10 uF en sortie. Ne pas « ameliorer ».
    C("C8", "Device:C", "10uF", C08, {"1": "VSYS", "2": "GND"})
    C("C9", "Device:C", "10uF", C08, {"1": "+3V3", "2": "GND"})
    C("C30", "Device:C", "10uF", C08, {"1": "+3V3", "2": "GND"})

    # --- boost 5V clavier
    C("U4", "typoena:TPS61023DRL", "TPS61023DRL", "Package_TO_SOT_SMD:SOT-563",
      {"1": "FB_5V", "2": "KBD_5V_EN", "3": "VSYS", "4": "GND", "5": "SW_5V",
       "6": "+5V"}, LCSC="C919459", MPN="TPS61023DRLR")
    # meme reference que L1 : une ligne de BOM en moins (3 $ de frais de chargeur)
    C("L3", "Device:L", "1uH 4.15A", "Inductor_SMD:L_APV_ANR4030",
      {"1": "VSYS", "2": "SW_5V"}, LCSC="C42193", MPN="SWPA4030S1R0NT")
    C("C10", "Device:C", "10uF", C08, {"1": "VSYS", "2": "GND"})
    C("C11", "Device:C", "22uF", C08, {"1": "+5V", "2": "GND"})
    C("C12", "Device:C", "22uF", C08, {"1": "+5V", "2": "GND"})
    C("R10", "Device:R", "56k 1%", R06, {"1": "+5V", "2": "FB_5V"})
    C("R11", "Device:R", "7.5k 1%", R06, {"1": "FB_5V", "2": "GND"})
    C("R12", "Device:R", "100k", R04, {"1": "KBD_5V_EN", "2": "GND"})

    # --- rail uSD commuté
    C("Q1", "Transistor_FET:AO3401A", "AO3401A", SOT23,
      {"1": "SD_GATE", "2": "+3V3", "3": "+3V3_SD"})
    C("R13", "Device:R", "100k", R04, {"1": "+3V3", "2": "SD_GATE"})
    C("Q2", "Transistor_FET:2N7002", "2N7002", SOT23,
      {"1": "SD_PWR_EN", "2": "GND", "3": "SD_GATE"})
    C("R14", "Device:R", "100k", R04, {"1": "SD_PWR_EN", "2": "GND"})
    C("C13", "Device:C", "10uF", C08, {"1": "+3V3_SD", "2": "GND"})

    # --- bouton : contact + LED
    C("J2", "Connector_Generic:Conn_01x02", "BOUTON contact JST-PH", PH2,
      {"1": "BTN_SW", "2": "GND"})
    C("R15", "Device:R", "10k", R04, {"1": "BTN_SW", "2": "PWR_SENSE"})
    C("J3", "Connector_Generic:Conn_01x02", "BOUTON LED JST-PH", PH2,
      {"1": "BTN_LED_A", "2": "BTN_LED_K"})
    C("R16", "Device:R", "100R", R04, {"1": "+3V3", "2": "BTN_LED_A"})
    C("Q3", "Transistor_FET:2N7002", "2N7002", SOT23,
      {"1": "BTN_LED", "2": "GND", "3": "BTN_LED_K"})
    C("R17", "Device:R", "100k", R04, {"1": "BTN_LED", "2": "GND"})


def decouplage_mcu(n):
    """Découplage du rail 3V3 au pied du MCU, quelle que soit sa forme."""
    n.C("C15", "Device:C", "100nF", C04, {"1": "+3V3", "2": "GND"})
    n.C("C16", "Device:C", "10uF", C08, {"1": "+3V3", "2": "GND"})
    n.C("C17", "Device:C", "22uF", C08, {"1": "+3V3", "2": "GND"})


# =====================================================================  ECRAN

# Broches du FPC volontairement en l'air.
EPD_NC = ["1", "4", "19"]


def ecran(n):
    C = n.C
    # Variante `_MountingPin` du symbole : elle seule expose la pastille mécanique MP
    # que porte l'empreinte. Sans elle, MP n'a aucun net — donc ni chevelu, ni
    # violation DRC, et le connecteur ne tient que par ses 24 pastilles de 0,3 mm.
    # Les 24 broches numérotées ont la même géométrie que Conn_01x24.
    C("J4", "Connector_Generic_MountingPin:Conn_01x24_MountingPin",
      "FPC 24p 0.5mm - PANNEAU",
      "Connector_FFC-FPC:Jushuo_AFC07-S24FCA-00_1x24-1MP_P0.50_Horizontal",
      {"2": "EPD_GDR", "3": "EPD_RESE", "5": "EPD_VDHR", "6": "EPD_TSCL",
       "7": "EPD_TSDA", "8": "GND", "9": "EPD_BUSY", "10": "EPD_RST", "11": "EPD_DC",
       "12": "EPD_CS", "13": "EPD_SCK", "14": "EPD_MOSI", "15": "+3V3", "16": "+3V3",
       "17": "GND", "18": "EPD_VDD", "20": "EPD_VSH", "21": "EPD_PREVGH",
       "22": "EPD_VSL", "23": "EPD_PREVGL", "24": "EPD_VCOM", "MP": "GND"})

    C("L4", "Device:L", "47uH 1.2A", "Inductor_SMD:L_APV_ANR6045",
      {"1": "+3V3", "2": "EPD_SW"}, LCSC="C36414", MPN="SWPA6045S470MT")
    C("Q4", "Transistor_FET:Si1308EDL", "Si1308EDL", SOT323,
      {"1": "EPD_GDR", "2": "EPD_RESE", "3": "EPD_SW"})
    C("R20", "Device:R", "1M", R04, {"1": "EPD_GDR", "2": "GND"})
    C("R21", "Device:R", "2R2", R08, {"1": "EPD_RESE", "2": "GND"})
    # pompe de charge : D3 cathode vers le noeud milieu, anode vers PREVGL (rail negatif)
    C("D3", "Diode:MBR0530", "B5819W", SOD123,
      {"1": "EPD_CPMID", "2": "EPD_PREVGL"})
    C("D4", "Diode:MBR0530", "B5819W", SOD123,
      {"1": "GND", "2": "EPD_CPMID"})
    C("D5", "Diode:MBR0530", "B5819W", SOD123,
      {"1": "EPD_PREVGH", "2": "EPD_SW"})
    C("C18", "Device:C", "1uF", C06, {"1": "EPD_SW", "2": "EPD_CPMID"})
    C("C19", "Device:C", "1uF", C06, {"1": "EPD_PREVGL", "2": "GND"})
    C("C20", "Device:C", "1uF", C06, {"1": "EPD_PREVGH", "2": "GND"})
    C("C21", "Device:C", "4.7uF", C08, {"1": "+3V3", "2": "GND"})
    C("C22", "Device:C", "1uF", C06, {"1": "+3V3", "2": "GND"})
    C("C23", "Device:C", "1uF", C06, {"1": "EPD_VDD", "2": "GND"})
    C("C24", "Device:C", "1uF", C06, {"1": "EPD_VSH", "2": "GND"})
    C("C25", "Device:C", "1uF", C06, {"1": "EPD_VSL", "2": "GND"})
    C("C26", "Device:C", "1uF", C06, {"1": "EPD_VCOM", "2": "GND"})
    C("C27", "Device:C", "1uF", C06, {"1": "EPD_VDHR", "2": "GND"})
    C("R22", "Device:R", "10k", R04, {"1": "+3V3", "2": "EPD_TSCL"})
    C("R23", "Device:R", "10k", R04, {"1": "+3V3", "2": "EPD_TSDA"})


# =====================================================================  IO

def io(n):
    """USB-C clavier, clavier interne, microSD, bus I2C.

    L'USB-C de charge (J6) reste à la carte : ses paires de données ne vont au
    même endroit que si la carte porte un pont USB-série.
    """
    C = n.C
    C("R24", "Device:R", "5k1 1%", R04, {"1": "CC1", "2": "GND"})
    C("R25", "Device:R", "5k1 1%", R04, {"1": "CC2", "2": "GND"})

    C("J7", "Connector:USB_C_Receptacle_USB2.0_16P", "USB-C CLAVIER (hote)", USBC,
      {"A1": "GND", "A4": "+5V", "A5": "KBD_CC1", "A6": "USB_DP", "A7": "USB_DN",
       "A9": "+5V", "B1": "GND", "B4": "+5V", "B5": "KBD_CC2", "B6": "USB_DP",
       "B7": "USB_DN", "B9": "+5V", "A12": "GND", "B12": "GND", "SH": "GND"})
    C("R26", "Device:R", "56k 1%", R06, {"1": "+5V", "2": "KBD_CC1"})
    C("R27", "Device:R", "56k 1%", R06, {"1": "+5V", "2": "KBD_CC2"})

    # Clavier de la variante intégrée, en dérivation sur la même paire que J7. Pas de
    # résistance série : l'étage USB de l'ESP32-S3 intègre l'adaptation et les rappels
    # de 15 kΩ côté hôte, en ajouter ici déséquilibrerait les deux branches.
    # UN SEUL des deux connecteurs à la fois — deux périphériques sur un bus hôte
    # n'ont pas de comportement défini.
    C("J13", "Connector_Generic:Conn_01x04", "CLAVIER INTERNE", PH4,
      {"1": "+5V", "2": "USB_DP", "3": "USB_DN", "4": "GND"})

    C("J8", "Connector:Micro_SD_Card_Det_Hirose_DM3AT", "microSD",
      "Connector_Card:microSD_HC_Molex_104031-0811",
      {"1": "SD_DAT2", "2": "SD_CS", "3": "SD_MOSI", "4": "+3V3_SD", "5": "SD_SCK",
       "6": "GND", "7": "SD_MISO", "8": "SD_DAT1", "9": "GND", "SH": "GND"})
    C("R28", "Device:R", "10k", R04, {"1": "+3V3_SD", "2": "SD_MISO"})
    C("R29", "Device:R", "10k", R04, {"1": "+3V3_SD", "2": "SD_DAT1"})
    C("R30", "Device:R", "10k", R04, {"1": "+3V3_SD", "2": "SD_DAT2"})

    # pull-ups du bus I2C (aucun autre composant ne les fournit)
    C("R31", "Device:R", "4k7", R04, {"1": "+3V3", "2": "I2C_SDA"})
    C("R32", "Device:R", "4k7", R04, {"1": "+3V3", "2": "I2C_SCL"})


def drapeaux(n):
    """Drapeaux d'alimentation pour l'ERC."""
    for i, net in enumerate(["GND", "VBAT", "VBUS", "VSYS", "+3V3_SD"]):
        n.C(f"#FLG{i:02d}", "power:PWR_FLAG", "PWR_FLAG", "", {"1": net})


# =====================================================================  references

# References JLCPCB, relevees dans la bibliotheque CDFER installee (voir README).
# Toutes Basic ou Preferred : exemptees de frais de chargeur en assemblage Economic.
PASSIFS = {
    ("R", "100R"):   ("C25076", "0402WGF1000TCE"),
    ("R", "150R"):   ("C22808", "0603WAF1500T5E"),
    ("R", "2R2"):    ("C17521", "0805W8F2R20T5E"),
    ("R", "4k7"):    ("C25900", "0402WGF4701TCE"),
    ("R", "5k1 1%"): ("C25905", "0402WGF5101TCE"),
    ("R", "7.5k 1%"): ("C23234", "0603WAF7501T5E"),
    ("R", "10k"):    ("C25744", "0402WGF1002TCE"),
    ("R", "12k"):    ("C25752", "0402WGF1202TCE"),
    ("R", "56k 1%"): ("C23206", "0603WAF5602T5E"),
    ("R", "100k"):   ("C25741", "0402WGF1003TCE"),
    ("R", "1M"):     ("C26083", "0402WGF1004TCE"),
    ("C", "100nF"):  ("C1525",  "CL05B104KO5NNNC"),
    ("C", "1uF"):    ("C15849", "CL10A105KB8NNNC"),
    ("C", "47nF"):   ("C1622",  "CL10B473KB8NNNC"),
    ("C", "4.7uF"):  ("C1779",  "CL21A475KAQNNNE"),
    ("C", "10uF"):   ("C15850", "CL21A106KAYNNNE"),
    ("C", "22uF"):   ("C45783", "CL21A226MAQNNNE"),
}

# Discrets et connecteurs. Les trois references laissees vides (SW*, J4, J8) attendent
# la reconciliation empreinte / piece reelle - voir README, "Ce qui reste a faire".
AUTRES = {
    # deux references JLCPCB pour le meme SMF5.0A : celle-ci est celle qu'on
    # a trouvee en stock.
    "D1": ("C19077497", "SMF5.0A"),
    "D3": ("C8598", "B5819W"), "D4": ("C8598", "B5819W"), "D5": ("C8598", "B5819W"),
    "Q1": ("C15127", "AO3401A"),
    "Q2": ("C8545", "2N7002"), "Q3": ("C8545", "2N7002"),
    "Q5": ("C8545", "2N7002"), "Q6": ("C8545", "2N7002"),
    "Q4": ("C7603347", "SI1308EDL"),
    "J1": ("C173752", "S2B-PH-K-S"), "J2": ("C173752", "S2B-PH-K-S"),
    "J3": ("C173752", "S2B-PH-K-S"),
    "J6": ("C165948", "TYPE-C-31-M-12"), "J7": ("C165948", "TYPE-C-31-M-12"),
    "J4": ("C262567", "AFC07-S24FCA-00"),
    "J13": ("C157926", "S4B-PH-K-S"),
    "J8": ("C585350", "1040310811"),
    "SW1": ("C231329", "B3U-1000P"), "SW2": ("C231329", "B3U-1000P"),
    "SW3": ("C231329", "B3U-1000P"),
    "U1": ("C2913202", "ESP32-S3-WROOM-1-N16R8"),
    "U5": ("C2846043", "CH343P"),
}


# Posés à la main, donc hors BOM et hors CPL : les trois JST-PH du bord (batterie,
# contact bouton, LED), le JST-PH clavier et le bouton QON. Chaque carte y ajoute
# ses propres non-montés — les headers traversants 2,54 mm, que JLCPCB n'assemble pas.
DNP_COMMUN = {"J1", "J2", "J3", "J13", "SW1"}


def remplacer(n, remplacements):
    """Applique les écarts d'une carte au tronc commun, après référencement.

    Une carte finie dans le GUI dérive de son générateur ; c'est ici qu'on
    rattrape l'écart plutôt que de laisser le générateur mentir sur elle.
    """
    for ref, champs in remplacements.items():
        for c in n.comps:
            if c[0] == ref:
                c[3] = champs.get("fp", c[3])
                c[5].update({k: v for k, v in champs.items() if k != "fp"})
                break
        else:
            raise SystemExit(f"remplacement : {ref} absent du tronc commun")


def referencer(n):
    for c in n.comps:
        k = (c[1].split(":")[-1], c[2])
        if k in PASSIFS:
            c[5]["LCSC"], c[5]["MPN"] = PASSIFS[k]
        if c[0] in AUTRES:
            c[5]["LCSC"], c[5]["MPN"] = AUTRES[c[0]]
    manquants = sorted({(c[1].split(":")[-1], c[2]) for c in n.comps
                        if c[1] in ("Device:R", "Device:C") and not c[5].get("LCSC")})
    if manquants:
        print("  !! passifs sans reference LCSC :", manquants)
    sans = [c[0] for c in n.comps
            if not c[5].get("LCSC") and not c[0].startswith("#FLG")]
    print("  sans reference LCSC :", ", ".join(sans) if sans else "aucun")


# =====================================================================  feuilles

# Répartition par feuille des composants que les deux cartes ont en commun ;
# chaque `board.py` complète avec les siens via FEUILLE_SUP.
FEUILLE = {}
for _r in ("C15", "C16", "C17"):
    FEUILLE[_r] = "mcu"
for _r in ("J4", "L4", "Q4", "R20", "R21", "R22", "R23", "D3", "D4", "D5"):
    FEUILLE[_r] = "display"
for _i in range(18, 28):
    FEUILLE[f"C{_i}"] = "display"
for _r in ("J7", "J8", "J13"):
    FEUILLE[_r] = "io"
for _i in range(24, 33):
    FEUILLE[f"R{_i}"] = "io"

SHEET_UUIDS = {
    "power":   "3a1c9e40-0001-4a00-9000-000000000001",
    "mcu":     "3a1c9e40-0002-4a00-9000-000000000002",
    "display": "3a1c9e40-0003-4a00-9000-000000000003",
    "io":      "3a1c9e40-0004-4a00-9000-000000000004",
}
ORDRE = ["power", "mcu", "display", "io"]
FICHIERS = {"power": "01-power.kicad_sch", "mcu": "02-mcu.kicad_sch",
            "display": "03-display.kicad_sch", "io": "04-io.kicad_sch"}


# =====================================================================  placement

GRID = 1.27
PAD_X = 5.08 + 33.0
PAD_Y = 5.08 + 12.7


def snap(v):
    return round(v / GRID) * GRID


def bbox(lib_id):
    """Encombrement des broches, en coordonnées schéma (Y inversé)."""
    _, pins = LIB.get(lib_id)
    xs = [p[0] for p in pins.values()]
    ys = [-p[1] for p in pins.values()]
    return min(xs), min(ys), max(xs), max(ys)


class Zone:
    """Rangement en étagères : on descend d'une ligne quand la zone déborde."""

    def __init__(self, x, y, width):
        self.x0, self.y0, self.w = x, y, width
        self.cx, self.cy, self.rowh = x, y, 0.0

    def place(self, lib_id):
        x0, y0, x1, y1 = bbox(lib_id)
        w = (x1 - x0) + 2 * PAD_X
        h = (y1 - y0) + 2 * PAD_Y
        if self.cx > self.x0 and self.cx + w > self.x0 + self.w:
            self.cx = self.x0
            self.cy += self.rowh
            self.rowh = 0.0
        ox = snap(self.cx + PAD_X - x0)
        oy = snap(self.cy + PAD_Y - y0)
        self.cx += w
        self.rowh = max(self.rowh, h)
        return ox, oy


def _wrap(t, n):
    mots, lignes, cur = t.split(), [], ""
    for m in mots:
        if len(cur) + len(m) + 1 > n:
            lignes.append(cur)
            cur = m
        else:
            cur = (cur + " " + m).strip()
    if cur:
        lignes.append(cur)
    return lignes[:3]


# =====================================================================  emission

def emettre(board, n, outdir):
    """Écrit la racine et les quatre feuilles du projet `board` dans `outdir`."""
    feuille = dict(FEUILLE)
    for nom, refs in board.FEUILLE_SUP.items():
        for r in refs:
            feuille[r] = nom

    par_feuille = {nom: [] for nom in ORDRE}
    for comp in n.comps:
        ref = comp[0]
        par_feuille["power" if ref.startswith("#FLG")
                    else feuille.get(ref, "power")].append(comp)

    for nom in ORDRE:
        titre = board.SHEETS[nom]
        z = Zone(25, 40, 1160)
        body, lib_ids = [], []
        inst_path = f"/{board.ROOT}/{SHEET_UUIDS[nom]}"
        for ref, lib_id, value, fp, pins, props in par_feuille[nom]:
            if lib_id not in lib_ids:
                lib_ids.append(lib_id)
            x, y = z.place(lib_id)
            body.append(symbol_instance(lib_id, ref, value, fp, x, y, pins,
                                        board.PROJECT, inst_path,
                                        extra_props=props, dnp=(ref in board.DNP)))
            seen = {}
            for num, net in pins.items():
                ax, ay, (dx, dy) = pin_abs(lib_id, x, y, num)
                key = (round(ax, 3), round(ay, 3))
                if key in seen:
                    if seen[key] != net:
                        raise SystemExit(f"{ref}: broches empilees en conflit")
                    continue
                seen[key] = net
                ex, ey = ax + dx * 5.08, ay + dy * 5.08
                body.append(wire(ax, ay, ex, ey))
                ang = 0 if dx > 0.5 else (180 if dx < -0.5 else (90 if dy < -0.5 else 270))
                body.append(label(net, ex, ey, ang, glob=True))
            for num in board.NC.get(ref, []):
                ax, ay, _ = pin_abs(lib_id, x, y, num)
                body.append(f'\t(no_connect\n\t\t(at {ax} {ay})\n\t\t(uuid "{uid()}")\n\t)\n')
        body.append(text(titre, 25, 30, size=2.5))
        emb = "".join("\t" + LIB.get(l)[0].replace("\n\t", "\n\t\t") + "\n" for l in lib_ids)
        out = HEADER.format(root=SHEET_UUIDS[nom], paper="A2", title=board.TITRE,
                            date=board.DATE, rev=board.REV, c1=titre[:90], c2="")
        out += "\t(lib_symbols\n" + emb + "\t)\n" + "".join(body)
        out += "\t(embedded_fonts no)\n)\n"
        open(os.path.join(outdir, FICHIERS[nom]), "w").write(out)
        print(f"  {FICHIERS[nom]:22} {len(par_feuille[nom]):3} composants")

    # --- feuille racine : uniquement les quatre feuilles filles
    root = HEADER.format(root=board.ROOT, paper="A4", title=board.TITRE,
                         date=board.DATE, rev=board.REV, c1=board.SOUS_TITRE,
                         c2="Connectivite inter-feuilles par etiquettes globales")
    root += "\t(lib_symbols\n\t)\n"
    for k, nom in enumerate(ORDRE):
        col, row = k % 2, k // 2
        sx, sy = 30.48 + col * 100.33, 45.72 + row * 66.04
        root += kisch.sheet(nom, FICHIERS[nom], sx, sy, 76.2, 40.64,
                            SHEET_UUIDS[nom], board.PROJECT, board.ROOT, page=k + 2)
        # description sous le cadre, apres la propriete Sheetfile
        for i, frag in enumerate(_wrap(board.SHEETS[nom], 46)):
            root += text(frag, sx, sy + 40.64 + 7.62 + i * 3.0, size=1.5)
    root += '\t(sheet_instances\n\t\t(path "/"\n\t\t\t(page "1")\n\t\t)\n\t)\n'
    root += "\t(embedded_fonts no)\n)\n"
    racine = os.path.join(outdir, board.PROJECT + ".kicad_sch")
    open(racine, "w").write(root)
    print(f"ecrit: {racine} (racine) + {len(ORDRE)} feuilles, {len(n.comps)} composants")
