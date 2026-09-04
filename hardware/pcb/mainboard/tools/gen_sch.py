#!/usr/bin/env python3
"""Génère le schéma de la mainboard.

Connectivité par étiquettes (voir kisch.py). Le schéma est plat : la découpe en
feuilles hiérarchiques se fait ensuite dans le GUI, elle ne change pas le netlist.
"""
import kisch
from kisch import LIB, uid, wire, label, text, symbol_instance, pin_abs, HEADER

PROJECT = "typoena-mainboard"
ROOT = "b1f0c5a2-7d34-4e19-9a6c-0f2e8d41c7b3"
OUT = ("/home/emmanuel/Documents/Developpement/esp32/typewriter/"
       "hardware/pcb/mainboard/typoena-mainboard.kicad_sch")

# empreintes courantes
R04 = "Resistor_SMD:R_0402_1005Metric"
C04 = "Capacitor_SMD:C_0402_1005Metric"
C06 = "Capacitor_SMD:C_0603_1608Metric"
C08 = "Capacitor_SMD:C_0805_2012Metric"
R08 = "Resistor_SMD:R_0805_2012Metric"
SOT23 = "Package_TO_SOT_SMD:SOT-23"
SOT323 = "Package_TO_SOT_SMD:SOT-323_SC-70"
SOD123 = "Diode_SMD:D_SOD-123"
PH2 = "Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal"

comps = []
notes = []
_zone = {}


def C(ref, lib_id, value, fp, pins, **props):
    comps.append([ref, lib_id, value, fp, pins, props])


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


def zone(name, x, y, width, cols=None, dx=None, dy=None):
    _zone[name] = dict(x0=x, y0=y, w=width, cx=x, cy=y, rowh=0.0)


def place(name, lib_id):
    """Rangement en étagères : on descend d'une ligne quand la zone déborde."""
    z = _zone[name]
    x0, y0, x1, y1 = bbox(lib_id)
    w = (x1 - x0) + 2 * PAD_X
    h = (y1 - y0) + 2 * PAD_Y
    if z["cx"] > z["x0"] and z["cx"] + w > z["x0"] + z["w"]:
        z["cx"] = z["x0"]
        z["cy"] += z["rowh"]
        z["rowh"] = 0.0
    ox = snap(z["cx"] + PAD_X - x0)
    oy = snap(z["cy"] + PAD_Y - y0)
    z["cx"] += w
    z["rowh"] = max(z["rowh"], h)
    return ox, oy


# =====================================================================  ALIMENTATION

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
C("R1", "Device:R", "150R", R04, {"1": "PMIC_ILIM", "2": "GND"})
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
  {"1": "FB_5V", "2": "KBD_5V_EN", "3": "VSYS", "4": "GND", "5": "SW_5V", "6": "+5V"},
  LCSC="C919459", MPN="TPS61023DRLR")
# meme reference que L1 : une ligne de BOM en moins (3 $ de frais de chargeur)
C("L3", "Device:L", "1uH 4.15A", "Inductor_SMD:L_APV_ANR4030",
  {"1": "VSYS", "2": "SW_5V"}, LCSC="C42193", MPN="SWPA4030S1R0NT")
C("C10", "Device:C", "10uF", C08, {"1": "VSYS", "2": "GND"})
C("C11", "Device:C", "22uF", C08, {"1": "+5V", "2": "GND"})
C("C12", "Device:C", "22uF", C08, {"1": "+5V", "2": "GND"})
C("R10", "Device:R", "56k 1%", R04, {"1": "+5V", "2": "FB_5V"})
C("R11", "Device:R", "7.5k 1%", R04, {"1": "FB_5V", "2": "GND"})
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

# =====================================================================  MCU

esp = {"1": "GND", "2": "+3V3", "3": "EN", "4": "EPD_BUSY", "5": "EPD_RST",
       "6": "EPD_DC", "7": "EPD_CS", "8": "SD_MOSI", "9": "PMIC_INT",
       "10": "I2C_SDA", "11": "I2C_SCL", "13": "USB_DM", "14": "USB_DP",
       "18": "SD_CS", "19": "EPD_MOSI", "20": "EPD_SCK", "21": "SD_MISO",
       "22": "SD_SCK", "23": "PWR_SENSE", "25": "IO48", "27": "IO0",
       "31": "BTN_LED", "33": "SD_PWR_EN", "34": "KBD_5V_EN", "36": "UART_RX",
       "37": "UART_TX", "40": "GND", "41": "GND",
       # sorties sur J10 (extension) : pads 39/38/12/17/32/35/24
       "39": "IO1", "38": "IO2", "12": "IO8", "17": "IO9", "32": "IO39",
       "35": "IO42", "24": "IO47"}
C("U1", "RF_Module:ESP32-S3-WROOM-1", "ESP32-S3-WROOM-1-N16R8",
  "RF_Module:ESP32-S3-WROOM-1", esp, MPN="ESP32-S3-WROOM-1-N16R8")
# broches volontairement non connectées : 12/17/39 = IO8/IO9/IO1 (ADC1 de reserve),
# 15/16/26 = IO3/IO46/IO45 (strapping), 24/32/35 = IO47/IO39/IO42 (reserve),
# 28/29/30 = IO35/IO36/IO37 (PSRAM octal, inutilisables)
# 15/16/26 = IO3/IO46/IO45, broches de strapping : deliberement PAS sorties, un
# niveau impose au demarrage empecherait le boot. 28/29/30 = IO35/IO36/IO37, PSRAM.
ESP_NC = ["15", "16", "26", "28", "29", "30"]

C("SW2", "Switch:SW_Push", "RESET", "Button_Switch_SMD:SW_SPST_B3U-1000P",
  {"1": "EN", "2": "GND"})
C("SW3", "Switch:SW_Push", "BOOT", "Button_Switch_SMD:SW_SPST_B3U-1000P",
  {"1": "IO0", "2": "GND"})
C("R18", "Device:R", "10k", R04, {"1": "+3V3", "2": "EN"})
C("C14", "Device:C", "1uF", C06, {"1": "EN", "2": "GND"})
C("R19", "Device:R", "10k", R04, {"1": "+3V3", "2": "IO0"})
C("C15", "Device:C", "100nF", C04, {"1": "+3V3", "2": "GND"})
C("C16", "Device:C", "10uF", C08, {"1": "+3V3", "2": "GND"})
C("C17", "Device:C", "22uF", C08, {"1": "+3V3", "2": "GND"})
# =====================================================================  ECRAN

# Variante `_MountingPin` du symbole : elle seule expose la pastille mécanique MP
# que porte l'empreinte. Sans elle, MP n'a aucun net — donc ni chevelu, ni
# violation DRC, et le connecteur ne tient que par ses 24 pastilles de 0,3 mm.
# Les 24 broches numérotées ont la même géométrie que Conn_01x24.
C("J4", "Connector_Generic_MountingPin:Conn_01x24_MountingPin", "FPC 24p 0.5mm - PANNEAU",
  "Connector_FFC-FPC:Jushuo_AFC07-S24FCA-00_1x24-1MP_P0.50_Horizontal",
  {"2": "EPD_GDR", "3": "EPD_RESE", "5": "EPD_VDHR", "6": "EPD_TSCL",
   "7": "EPD_TSDA", "8": "GND", "9": "EPD_BUSY", "10": "EPD_RST", "11": "EPD_DC",
   "12": "EPD_CS", "13": "EPD_SCK", "14": "EPD_MOSI", "15": "+3V3", "16": "+3V3",
   "17": "GND", "18": "EPD_VDD", "20": "EPD_VSH", "21": "EPD_PREVGH",
   "22": "EPD_VSL", "23": "EPD_PREVGL", "24": "EPD_VCOM", "MP": "GND"})
EPD_NC = ["1", "4", "19"]

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
# secours : header 8 points vers une DESPI-C579 si l'etage integre ne demarre pas
C("J5", "Connector_Generic:Conn_01x08", "SECOURS DESPI-C579",
  "Connector_PinHeader_2.54mm:PinHeader_1x08_P2.54mm_Vertical",
  {"1": "+3V3", "2": "GND", "3": "EPD_MOSI", "4": "EPD_SCK", "5": "EPD_CS",
   "6": "EPD_DC", "7": "EPD_RST", "8": "EPD_BUSY"})

# =====================================================================  IO

C("J6", "Connector:USB_C_Receptacle_USB2.0_16P", "USB-C CHARGE + PROG",
  "Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12",
  {"A1": "GND", "A4": "VBUS", "A5": "CC1", "A6": "USB_PROG_DP", "A7": "USB_PROG_DM",
   "A9": "VBUS", "B1": "GND", "B4": "VBUS", "B5": "CC2", "B6": "USB_PROG_DP",
   "B7": "USB_PROG_DM", "B9": "VBUS", "A12": "GND", "B12": "GND", "SH": "GND"})
J6_NC = ["A8", "B8"]
C("R24", "Device:R", "5k1 1%", R04, {"1": "CC1", "2": "GND"})
C("R25", "Device:R", "5k1 1%", R04, {"1": "CC2", "2": "GND"})

C("J7", "Connector:USB_C_Receptacle_USB2.0_16P", "USB-C CLAVIER (hote)",
  "Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12",
  {"A1": "GND", "A4": "+5V", "A5": "KBD_CC1", "A6": "USB_DP", "A7": "USB_DM",
   "A9": "+5V", "B1": "GND", "B4": "+5V", "B5": "KBD_CC2", "B6": "USB_DP",
   "B7": "USB_DM", "B9": "+5V", "A12": "GND", "B12": "GND", "SH": "GND"})
C("R26", "Device:R", "56k 1%", R04, {"1": "+5V", "2": "KBD_CC1"})
C("R27", "Device:R", "56k 1%", R04, {"1": "+5V", "2": "KBD_CC2"})

C("J8", "Connector:Micro_SD_Card_Det_Hirose_DM3AT", "microSD",
  "Connector_Card:microSD_HC_Molex_104031-0811",
  {"1": "SD_DAT2", "2": "SD_CS", "3": "SD_MOSI", "4": "+3V3_SD", "5": "SD_SCK",
   "6": "GND", "7": "SD_MISO", "8": "SD_DAT1", "9": "GND",    "SH": "GND"})
C("R28", "Device:R", "10k", R04, {"1": "+3V3_SD", "2": "SD_MISO"})
C("R29", "Device:R", "10k", R04, {"1": "+3V3_SD", "2": "SD_DAT1"})
C("R30", "Device:R", "10k", R04, {"1": "+3V3_SD", "2": "SD_DAT2"})

C("U5", "Interface_USB:CH343P", "CH343P", "Package_DFN_QFN:WCH_QFN-16-1EP_3x3mm_P0.5mm_EP1.8x1.8mm",
  {"1": "+3V3", "2": "GND", "3": "VBUS", "6": "CH343_V3", "7": "USB_PROG_DP",
   "8": "USB_PROG_DM", "9": "VBUS", "12": "PROG_DTR", "13": "PROG_RTS",
   "4": "UART_RX", "5": "UART_TX", "17": "GND"})
CH_NC = ["10", "11", "14", "15", "16"]
C("C28", "Device:C", "1uF", C06, {"1": "CH343_V3", "2": "GND"})
C("C29", "Device:C", "100nF", C04, {"1": "+3V3", "2": "GND"})
# auto-reset : le montage classique deux transistors (DTR/RTS -> EN/IO0)
# Auto-reset croise facon DevKitC : chaque transistor a sa source sur l'AUTRE
# signal, si bien que EN/IO0 ne sont tires bas que lorsque DTR et RTS different.
# Les deux asserter ensemble (ce que fait un terminal a l'ouverture du port) ne
# doit PAS redemarrer la carte.
C("Q5", "Transistor_FET:2N7002", "2N7002", SOT23,
  {"1": "PROG_DTR", "2": "PROG_RTS", "3": "IO0"})
C("Q6", "Transistor_FET:2N7002", "2N7002", SOT23,
  {"1": "PROG_RTS", "2": "PROG_DTR", "3": "EN"})
# pull-ups du bus I2C (aucun autre composant ne les fournit)
C("R31", "Device:R", "4k7", R04, {"1": "+3V3", "2": "I2C_SDA"})
C("R32", "Device:R", "4k7", R04, {"1": "+3V3", "2": "I2C_SCL"})
# Extension : les 4 premieres broches suivent le brochage Qwiic (GND/3V3/SDA/SCL),
# de sorte qu'un peripherique I2C se soude dessus sans decodage. Non monte : ce sont
# des trous metallises, ils ne coutent ni ligne de BOM ni pose.
C("J10", "Connector_Generic:Conn_01x12", "EXTENSION (non monte)",
  "Connector_PinHeader_2.54mm:PinHeader_1x12_P2.54mm_Vertical",
  {"1": "GND", "2": "+3V3", "3": "I2C_SDA", "4": "I2C_SCL",
   "5": "IO1", "6": "IO2", "7": "IO8", "8": "IO9", "9": "IO39", "10": "IO42",
   "11": "IO47", "12": "IO48"})
C("J9", "Connector_Generic:Conn_01x06", "UART PROG",
  "Connector_PinHeader_2.54mm:PinHeader_1x06_P2.54mm_Vertical",
  {"1": "+3V3", "2": "GND", "3": "UART_TX", "4": "UART_RX", "5": "EN", "6": "IO0"})

# drapeaux d'alimentation pour l'ERC
for i, net in enumerate(["GND", "VBAT", "VBUS", "VSYS", "+3V3_SD"]):
    C(f"#FLG{i:02d}", "power:PWR_FLAG", "PWR_FLAG", "", {"1": net})


# References JLCPCB, relevees dans la bibliotheque CDFER installee (voir README).
# Toutes Basic ou Preferred : exemptees de frais de chargeur en assemblage Economic.
PASSIFS = {
    ("R", "100R"):  ("C25076", "0402WGF1000TCE"),
    ("R", "150R"):  ("C25082", "0402WGF1500TCE"),
    ("R", "2R2"):   ("C17521", "0805W8F2R20T5E"),
    ("R", "4k7"):   ("C25900", "0402WGF4701TCE"),
    ("R", "5k1 1%"):("C25905", "0402WGF5101TCE"),
    ("R", "7.5k 1%"):("C25918", "0402WGF7501TCE"),
    ("R", "10k"):   ("C25744", "0402WGF1002TCE"),
    ("R", "12k"):   ("C25752", "0402WGF1202TCE"),
    ("R", "56k 1%"):("C25796", "0402WGF5602TCE"),
    ("R", "100k"):  ("C25741", "0402WGF1003TCE"),
    ("R", "1M"):    ("C26083", "0402WGF1004TCE"),
    ("C", "100nF"): ("C1525",  "CL05B104KO5NNNC"),
    ("C", "1uF"):   ("C15849", "CL10A105KB8NNNC"),
    ("C", "47nF"):  ("C1622",  "CL10B473KB8NNNC"),
    ("C", "4.7uF"): ("C1779",  "CL21A475KAQNNNE"),
    ("C", "10uF"):  ("C15850", "CL21A106KAYNNNE"),
    ("C", "22uF"):  ("C45783", "CL21A226MAQNNNE"),
}
for _c in comps:
    _k = (_c[1].split(":")[-1], _c[2])
    if _k in PASSIFS:
        _c[5]["LCSC"], _c[5]["MPN"] = PASSIFS[_k]
manquants = sorted({(c[1].split(":")[-1], c[2]) for c in comps
                    if c[1] in ("Device:R", "Device:C") and not c[5].get("LCSC")})
if manquants:
    print("  !! passifs sans reference LCSC :", manquants)


# Discrets et connecteurs. Les trois references laissees vides (SW*, J4, J8) attendent
# la reconciliation empreinte / piece reelle - voir README, "Ce qui reste a faire".
AUTRES = {
    "U1": ("C2913202", "ESP32-S3-WROOM-1-N16R8"),
    "U5": ("C2846043", "CH343P"),
    "D1": ("C284108",  "SMF5.0A"),
    "D3": ("C8598", "B5819W"), "D4": ("C8598", "B5819W"), "D5": ("C8598", "B5819W"),
    "Q1": ("C15127", "AO3401A"),
    "Q2": ("C8545", "2N7002"), "Q3": ("C8545", "2N7002"),
    "Q5": ("C8545", "2N7002"), "Q6": ("C8545", "2N7002"),
    "Q4": ("C7603347", "SI1308EDL"),
    "J1": ("C173752", "S2B-PH-K-S"), "J2": ("C173752", "S2B-PH-K-S"),
    "J3": ("C173752", "S2B-PH-K-S"),
    "J6": ("C165948", "TYPE-C-31-M-12"), "J7": ("C165948", "TYPE-C-31-M-12"),
    "J4": ("C262567", "AFC07-S24FCA-00"),
    "J8": ("C585350", "1040310811"),
    "SW1": ("C231329", "B3U-1000P"), "SW2": ("C231329", "B3U-1000P"),
    "SW3": ("C231329", "B3U-1000P"),
}
# J5 (secours DESPI) et J9 (UART) sont des headers traversants 2,54 mm : JLCPCB ne les
# assemble pas. Marques non-montes pour qu'ils sortent du BOM et du CPL.
DNP = {"J5", "J9", "J10"}
for _c in comps:
    if _c[0] in AUTRES:
        _c[5]["LCSC"], _c[5]["MPN"] = AUTRES[_c[0]]
_sans = [c[0] for c in comps if not c[5].get("LCSC") and not c[0].startswith("#FLG")]
print("  sans reference LCSC :", ", ".join(_sans) if _sans else "aucun")

NC = {"U1": ESP_NC, "J4": EPD_NC, "J6": J6_NC, "J7": ["A8", "B8"], "J8": ["10"], "U5": CH_NC}

# =====================================================================  emission
zone_of = {}
for name in ("pwr", "mcu", "epd", "io", "flags"):
    pass
# on ré-associe chaque composant à sa zone dans l'ordre de déclaration
order = []
for ref, *_ in comps:
    if ref.startswith("#FLG"):
        order.append("flags")
    elif ref in ("U1", "SW2", "SW3") or ref in ("R18", "R19", "C14", "C15", "C16", "C17"):
        order.append("mcu")
    elif ref in ("J4", "J5", "L4", "Q4", "R20", "R21", "R22", "R23", "D3", "D4", "D5") \
            or (ref.startswith("C") and ref[1:].isdigit() and 18 <= int(ref[1:]) <= 27
                and ref != "C30"):
        order.append("epd")
    elif ref in ("J6", "J7", "J8", "J9", "J10", "U5", "Q5", "Q6") \
            or (ref.startswith("R") and ref[1:].isdigit() and 24 <= int(ref[1:]) <= 32) \
            or ref in ("C28", "C29"):
        order.append("io")
    else:
        order.append("pwr")

SHEETS = [
    ("power",   "01-power.kicad_sch",   "Alimentation - BQ25896 (charge + power path NVDC), TPS63001 3V3, TPS61023 5V clavier, rail uSD commute, bouton"),
    ("mcu",     "02-mcu.kicad_sch",     "MCU - ESP32-S3-WROOM-1-N16R8, strapping, decouplage"),
    ("display", "03-display.kicad_sch", "Ecran GDEY0579T93 - etage de puissance du §9, FPC 24p, header DESPI de secours"),
    ("io",      "04-io.kicad_sch",      "IO - USB-C charge/prog, USB-C clavier (hote), microSD, pont USB-serie CH343P, extension"),
]
ZONE2SHEET = {"pwr": "power", "mcu": "mcu", "epd": "display", "io": "io",
              "flags": "power"}
SHEET_UUIDS = {
    "power":   "3a1c9e40-0001-4a00-9000-000000000001",
    "mcu":     "3a1c9e40-0002-4a00-9000-000000000002",
    "display": "3a1c9e40-0003-4a00-9000-000000000003",
    "io":      "3a1c9e40-0004-4a00-9000-000000000004",
}

# repartition des composants par feuille, dans l'ordre de declaration
par_feuille = {n: [] for n, _, _ in SHEETS}
for comp, zname in zip(comps, order):
    par_feuille[ZONE2SHEET[zname]].append(comp)

def _wrap(t, n):
    mots, lignes, cur = t.split(), [], ""
    for m in mots:
        if len(cur) + len(m) + 1 > n:
            lignes.append(cur); cur = m
        else:
            cur = (cur + " " + m).strip()
    if cur: lignes.append(cur)
    return lignes[:3]


import os
OUTDIR = os.path.dirname(OUT)

for name, fname, titre in SHEETS:
    zone(name, 25, 40, 1160)
    body, lib_ids = [], []
    inst_path = f"/{ROOT}/{SHEET_UUIDS[name]}"
    for ref, lib_id, value, fp, pins, props in par_feuille[name]:
        if lib_id not in lib_ids:
            lib_ids.append(lib_id)
        x, y = place(name, lib_id)
        body.append(symbol_instance(lib_id, ref, value, fp, x, y, pins, PROJECT,
                                    inst_path, extra_props=props, dnp=(ref in DNP)))
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
        for num in NC.get(ref, []):
            ax, ay, _ = pin_abs(lib_id, x, y, num)
            body.append(f'\t(no_connect\n\t\t(at {ax} {ay})\n\t\t(uuid "{uid()}")\n\t)\n')
    body.append(text(titre, 25, 30, size=2.5))
    emb = "".join("\t" + LIB.get(l)[0].replace("\n\t", "\n\t\t") + "\n" for l in lib_ids)
    out = HEADER.format(root=SHEET_UUIDS[name], paper="A2", title="Typoena mainboard",
                        date="2026-08-15", rev="A", c1=titre[:90], c2="")
    out += "\t(lib_symbols\n" + emb + "\t)\n" + "".join(body)
    out += "\t(embedded_fonts no)\n)\n"
    open(os.path.join(OUTDIR, fname), "w").write(out)
    print(f"  {fname:22} {len(par_feuille[name]):3} composants")

# --- feuille racine : uniquement les quatre feuilles filles
root = HEADER.format(root=ROOT, paper="A4", title="Typoena mainboard",
                     date="2026-08-15", rev="A",
                     c1="BQ25896 + TPS63001 + TPS61023 - carte unique PCBA",
                     c2="Connectivite inter-feuilles par etiquettes globales")
root += "\t(lib_symbols\n\t)\n"
for k, (name, fname, titre) in enumerate(SHEETS):
    col, row = k % 2, k // 2
    _sx, _sy = 30.48 + col * 100.33, 45.72 + row * 66.04
    root += kisch.sheet(name, fname, _sx, _sy, 76.2, 40.64,
                        SHEET_UUIDS[name], PROJECT, ROOT, page=k + 2)
    # description sous le cadre, apres la propriete Sheetfile
    for _i, _frag in enumerate(_wrap(titre, 46)):
        root += text(_frag, _sx, _sy + 40.64 + 7.62 + _i * 3.0, size=1.5)
root += '\t(sheet_instances\n\t\t(path "/"\n\t\t\t(page "1")\n\t\t)\n\t)\n'
root += "\t(embedded_fonts no)\n)\n"
open(OUT, "w").write(root)
print(f"ecrit: {OUT} (racine) + {len(SHEETS)} feuilles, {len(comps)} composants")
