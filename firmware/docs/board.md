# Bench board — ESP32-S3-DevKitC-1 v1.0 pinout

The bench board follows the **ESP32-S3-DevKitC-1 v1.0** pinout — an
ESP32-S3-WROOM-1 **N16R8** module (16 MB flash, 8 MB octal PSRAM). The v1.0
revision wires the on-board WS2812 RGB LED to **GPIO 48**; v1.1 moved it to
GPIO 38, so match assignments against this diagram, not the v1.1 one.

![ESP32-S3-DevKitC-1 v1.0 pinout](esp32-s3-devkitc-1-v1.0-pinout.jpg)

Source: [Espressif ESP32-S3-DevKitC-1 v1.0 user guide][devkitc-1-v1.0]. The
octal PSRAM consumes **GPIO 26–37**, so those are unavailable for peripherals.

[devkitc-1-v1.0]: https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32s3/esp32-s3-devkitc-1/user_guide_v1.0.html

Pin assignments in use (verified in the [bring-up spikes](bring-up-spikes.md)):

- **EPD (SPI2):** SCK 12 · DIN/MOSI 11 · CS 7 · DC 6 · RST 5 · BUSY 4, via the
  DESPI-C579 breakout.
- **SD card (dedicated SPI3, ADR-012):** SCK 14 · MOSI 15 · MISO 13 · CS 10.
