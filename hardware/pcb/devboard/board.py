#!/usr/bin/env python3
"""Delta de la devboard : ce qu'elle porte et que la mainboard n'a pas.

Carte figée — elle décrit la carte fabriquée posée sur l'établi. `netlist.lock`
verrouille son netlist : toute dérive du tronc commun de `netlist.py` la fait
échouer plutôt que de la modifier en silence.
"""
import netlist
from netlist import USBC, HDR22

PROJECT = "typoena-devboard"
ROOT = "b87907f7-0d09-4e38-9be2-0dfe24e700f1"
TITRE = "Typoena devboard"
SOUS_TITRE = "BQ25896 + TPS63001 + TPS61023 - carte unique PCBA"
DATE = "2026-08-15"
REV = "A"

SHEETS = {
    "power": "Alimentation - BQ25896 (charge + power path NVDC), TPS63001 3V3, "
             "TPS61023 5V clavier, rail uSD commute, bouton",
    "mcu": "MCU - rangees ESP32-S3-DevKitC-1 (2 x 22, 22,86 mm), decouplage 3V3",
    "display": "Ecran GDEY0579T93 - etage de puissance du §9, FPC 24p, "
               "header DESPI de secours",
    "io": "IO - USB-C charge, USB-C clavier (hote), clavier interne JST-PH, "
          "microSD, extension",
}

# RST (J11.3) reste en l'air : la carte de développement a son bouton et son
# rappel, rien ici n'a de raison de la redémarrer.
J11_NC = ["3", "13", "14", "21"]
J12_NC = ["2", "3", "11", "12", "13", "14", "15"]
# Port de charge seul : le pont USB-série est sur la carte de développement, donc
# les paires de données ne vont plus nulle part. Les 5k1 de CC restent, ce sont
# elles qui déclarent la carte comme consommateur.
J6_NC = ["A6", "A7", "A8", "B6", "B7", "B8"]

NC = {"J11": J11_NC, "J12": J12_NC, "J6": J6_NC,
      "J4": ["1", "4", "19"], "J7": ["A8", "B8"], "J8": ["10"]}

# JLCPCB n'assemble pas les headers traversants 2,54 mm. Le reste des non-montés
# est commun aux deux cartes, voir netlist.DNP_COMMUN.
DNP = netlist.DNP_COMMUN | {"J5", "J11", "J12"}

FEUILLE_SUP = {"mcu": ["J11", "J12"], "display": ["J5"], "io": ["J6"]}


def construire(n):
    C = n.C

    # Rangées de la carte de développement ESP32-S3-DevKitC-1, 2 x 22 points au pas
    # de 2,54 mm, 22,86 mm entre rangées. Correspondance relevée sur le brochage
    # Espressif (docs/assets/esp32-s3-devkitc-1-v1.0-pinout.jpg) croisé avec le
    # brochage du module : chaque signal garde la GPIO qu'il avait sur la mainboard.
    #
    # Non câblées, et pourquoi : IO3/IO46/IO45 sont des broches de strapping, un
    # niveau imposé au démarrage empêcherait le boot ; IO35/IO36/IO37 servent la
    # PSRAM octale ; TXD0/RXD0 appartiennent au CP2102 de la carte de développement ;
    # IO0 n'a plus d'usage depuis que SW3 et l'auto-reset ont disparu.
    #
    # 5V0 reste en l'air : la carte est alimentée en 3V3 direct, et relier 5V0 au
    # boost clavier mettrait deux sources sur le régulateur interne de la devboard.
    C("J11", "Connector_Generic:Conn_01x22", "DEVKITC-1 rangée gauche", HDR22,
      {"1": "+3V3", "2": "+3V3", "4": "EPD_BUSY", "5": "EPD_RST",
       "6": "EPD_DC", "7": "EPD_CS", "8": "SD_MOSI", "9": "PMIC_INT",
       "10": "I2C_SDA", "11": "I2C_SCL", "12": "IO8", "15": "IO9", "16": "SD_CS",
       "17": "EPD_MOSI", "18": "EPD_SCK", "19": "SD_MISO", "20": "SD_SCK",
       "22": "GND"})
    C("J12", "Connector_Generic:Conn_01x22", "DEVKITC-1 rangée droite", HDR22,
      {"1": "GND", "4": "IO1", "5": "IO2", "6": "IO42", "7": "KBD_5V_EN",
       "8": "SD_PWR_EN", "9": "IO39", "10": "BTN_LED", "16": "IO48", "17": "IO47",
       "18": "PWR_SENSE", "19": "USB_DP", "20": "USB_DN", "21": "GND", "22": "GND"})

    # secours : header 8 points vers une DESPI-C579 si l'etage integre ne demarre pas
    C("J5", "Connector_Generic:Conn_01x08", "SECOURS DESPI-C579",
      "Connector_PinHeader_2.54mm:PinHeader_1x08_P2.54mm_Vertical",
      {"1": "+3V3", "2": "GND", "3": "EPD_MOSI", "4": "EPD_SCK", "5": "EPD_CS",
       "6": "EPD_DC", "7": "EPD_RST", "8": "EPD_BUSY"})

    C("J6", "Connector:USB_C_Receptacle_USB2.0_16P", "USB-C CHARGE + PROG", USBC,
      {"A1": "GND", "A4": "VBUS", "A5": "CC1", "A9": "VBUS", "B1": "GND",
       "B4": "VBUS", "B5": "CC2", "B9": "VBUS", "A12": "GND", "B12": "GND",
       "SH": "GND"})
