#!/usr/bin/env python3
"""Delta de la mainboard : ce qu'elle porte et que la devboard n'a pas.

Le module ESP32-S3 soudé remplace les rangées DevKitC, et la carte porte son
propre pont USB-série — donc l'USB-C de charge voit aussi ses paires de données.
"""
import netlist
from netlist import C04, C06, R04, SOT23, USBC

PROJECT = "typoena-mainboard"
ROOT = "b1f0c5a2-7d34-4e19-9a6c-0f2e8d41c7b3"
TITRE = "Typoena mainboard"
SOUS_TITRE = "BQ25896 + TPS63001 + TPS61023 - carte unique PCBA"
DATE = "2026-08-15"
REV = "A"

SHEETS = {
    "power": "Alimentation - BQ25896 (charge + power path NVDC), TPS63001 3V3, "
             "TPS61023 5V clavier, rail uSD commute, bouton",
    "mcu": "MCU - ESP32-S3-WROOM-1-N16R8, strapping, decouplage",
    "display": "Ecran GDEY0579T93 - etage de puissance du §9, FPC 24p",
    "io": "IO - USB-C charge/prog, USB-C clavier (hote), clavier interne JST-PH, "
          "microSD, pont USB-serie CH343P",
}

# 15/16/26 = IO3/IO46/IO45, broches de strapping : deliberement PAS sorties, un
# niveau impose au demarrage empecherait le boot. 28/29/30 = IO35/IO36/IO37,
# occupees par la PSRAM octale, inutilisables.
# 35 = IO42 : plus aucune destination depuis le retrait de son point de test.
ESP_NC = ["15", "16", "26", "28", "29", "30", "35"]
CH_NC = ["10", "11", "14", "15", "16"]
# SBU1/SBU2 : l'USB 2.0 ne s'en sert pas.
J6_NC = ["A8", "B8"]

NC = {"U1": ESP_NC, "U5": CH_NC, "J6": J6_NC,
      "J4": ["1", "4", "19"], "J7": ["A8", "B8"], "J8": ["10"]}

# JLCPCB n'assemble pas le header traversant 2,54 mm J9. Le reste des non-montés
# est commun aux deux cartes, voir netlist.DNP_COMMUN.
DNP = netlist.DNP_COMMUN | {"J9"} | {f"TP{i}" for i in (1, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13)}

FEUILLE_SUP = {
    "mcu": ["U1", "SW2", "SW3", "R18", "R19", "C14"],
    "io": (["J6", "J9", "U5", "Q5", "Q6", "C28", "C29"]
           + [f"TP{i}" for i in (1, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13)]),
}


def construire(n):
    C = n.C

    # Chaque signal garde la GPIO qu'il a sur la devboard : le firmware est
    # commun aux deux cartes.
    esp = {"1": "GND", "2": "+3V3", "3": "EN", "4": "EPD_BUSY", "5": "EPD_RST",
           "6": "EPD_DC", "7": "EPD_CS", "8": "SD_MOSI", "9": "PMIC_INT",
           "10": "I2C_SDA", "11": "I2C_SCL", "13": "USB_DN", "14": "USB_DP",
           "18": "SD_CS", "19": "EPD_MOSI", "20": "EPD_SCK", "21": "SD_MISO",
           "22": "SD_SCK", "23": "PWR_SENSE", "25": "IO48", "27": "IO0",
           "31": "BTN_LED", "33": "SD_PWR_EN", "34": "KBD_5V_EN", "36": "UART_RX",
           "37": "UART_TX", "40": "GND", "41": "GND",
           # GPIO libres, sortis sur points de test : pads 39/38/12/17/32/24/25
           "39": "IO1", "38": "IO2", "12": "IO8", "17": "IO9", "32": "IO39",
           "24": "IO47"}
    C("U1", "RF_Module:ESP32-S3-WROOM-1", "ESP32-S3-WROOM-1-N16R8",
      "RF_Module:ESP32-S3-WROOM-1", esp, MPN="ESP32-S3-WROOM-1-N16R8")

    C("SW2", "Switch:SW_Push", "RESET", "Button_Switch_SMD:SW_SPST_B3U-1000P",
      {"1": "EN", "2": "GND"})
    C("SW3", "Switch:SW_Push", "BOOT", "Button_Switch_SMD:SW_SPST_B3U-1000P",
      {"1": "IO0", "2": "GND"})
    C("R18", "Device:R", "10k", R04, {"1": "+3V3", "2": "EN"})
    C("C14", "Device:C", "1uF", C06, {"1": "EN", "2": "GND"})
    C("R19", "Device:R", "10k", R04, {"1": "+3V3", "2": "IO0"})

    # L'USB-C de charge porte aussi le port de programmation : ses paires de
    # données descendent sur le CH343P, pas sur l'USB natif de l'ESP32-S3 (qui
    # reste câblé sur J7/J13, côté hôte clavier).
    C("J6", "Connector:USB_C_Receptacle_USB2.0_16P", "USB-C CHARGE + PROG", USBC,
      {"A1": "GND", "A4": "VBUS", "A5": "CC1", "A6": "USB_PROG_DP",
       "A7": "USB_PROG_DN", "A9": "VBUS", "B1": "GND", "B4": "VBUS", "B5": "CC2",
       "B6": "USB_PROG_DP", "B7": "USB_PROG_DN", "B9": "VBUS", "A12": "GND",
       "B12": "GND", "SH": "GND"})

    C("U5", "Interface_USB:CH343P", "CH343P",
      "Package_DFN_QFN:WCH_QFN-16-1EP_3x3mm_P0.5mm_EP1.8x1.8mm",
      {"1": "+3V3", "2": "GND", "3": "VBUS", "6": "CH343_V3", "7": "USB_PROG_DP",
       "8": "USB_PROG_DN", "9": "VBUS", "12": "PROG_DTR", "13": "PROG_RTS",
       "4": "UART_RX", "5": "UART_TX", "17": "GND"})
    C("C28", "Device:C", "1uF", C06, {"1": "CH343_V3", "2": "GND"})
    C("C29", "Device:C", "100nF", C04, {"1": "+3V3", "2": "GND"})

    # Auto-reset croise facon DevKitC : chaque transistor a sa source sur l'AUTRE
    # signal, si bien que EN/IO0 ne sont tires bas que lorsque DTR et RTS different.
    # Les deux asserter ensemble (ce que fait un terminal a l'ouverture du port) ne
    # doit PAS redemarrer la carte.
    C("Q5", "Transistor_FET:2N7002", "2N7002", SOT23,
      {"1": "PROG_DTR", "2": "PROG_RTS", "3": "IO0"})
    C("Q6", "Transistor_FET:2N7002", "2N7002", SOT23,
      {"1": "PROG_RTS", "2": "PROG_DTR", "3": "EN"})

    C("J9", "Connector_Generic:Conn_01x06", "UART PROG",
      "Connector_PinHeader_2.54mm:PinHeader_1x06_P2.54mm_Vertical",
      {"1": "+3V3", "2": "GND", "3": "UART_TX", "4": "UART_RX", "5": "EN",
       "6": "IO0"})

    # Points de test : trous traversants Ø1,0, non montés — l'empreinte se retire
    # d'elle-même du BOM et du CPL. Ils remplacent une barrette d'extension : chaque
    # net va où le routage l'arrange, sans contrainte de les mener côte à côte.
    #
    # Chacun porte son net dans `Value`, à basculer de F.Fab vers F.Silkscreen côté
    # PCB : sur une barrette la position encode l'identité, un trou isolé ne dit
    # rien. Sans cette sérigraphie ils sont inutilisables.
    #
    # Deux GND, à poser éloignés l'un de l'autre : un point de test de signal sans
    # retour à proximité ne sert ni à sonder ni à alimenter.
    # Deux tailles, selon l'usage. Les cinq premiers sont ceux ou l'on branche
    # quelque chose d'alimente — typiquement un peripherique I2C : percage Ø1,0,
    # le seul qui laisse passer une broche carree de 0,64 mm (0,9 de diagonale),
    # donc le seul qui garde l'option barrette + cavalier.
    # Les huit GPIO ne recevront qu'un fil soude une fois : Ø0,7 suffit pour du
    # 24 AWG et divise l'encombrement par deux.
    GROS = "TestPoint:TestPoint_THTPad_D2.0mm_Drill1.0mm"
    PETIT = "TestPoint:TestPoint_THTPad_D1.5mm_Drill0.7mm"
    for ref, net, fp in (("TP1", "GND", GROS), ("TP3", "+3V3", GROS),
                         ("TP4", "I2C_SDA", GROS), ("TP5", "I2C_SCL", GROS),
                         ("TP6", "IO1", PETIT), ("TP7", "IO2", PETIT),
                         ("TP8", "IO8", PETIT), ("TP9", "IO9", PETIT),
                         ("TP10", "IO39", PETIT),
                         ("TP12", "IO47", PETIT), ("TP13", "IO48", PETIT)):
        C(ref, "Connector:TestPoint", net, fp, {"1": net})
