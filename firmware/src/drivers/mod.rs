//! Drivers — the esp-idf implementations of the hardware ports.
//!
//! Each module owns one concrete device and the port impl that exposes it: the
//! SSD1683 panel ([`screen_epd`], `hal::Screen`), the USB-host keyboard
//! ([`keyboard_usb`], `hal::Keyboard`), Wi-Fi bring-up ([`wifi_esp`]), and the
//! esp clock/system adapters ([`clock_esp`] `app::Clock`, [`system_esp`]
//! `app::System`). The layers above name only the traits (see the `hal` and
//! `app` crates); these are injected at composition in `main.rs`. Mirrors the
//! `drivers/` tier of the C `../typing-machine` reference.

pub mod clock_esp;
pub mod keyboard_usb;
pub mod screen_epd;
pub mod system_esp;
pub mod wifi_esp;
