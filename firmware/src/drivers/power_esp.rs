//! Power: the rails, the button, the status LED and the cell, behind
//! [`app::Power`].
//!
//! The mainboard has no mechanical power switch. The 3V3 buck-boost is enabled
//! by hardware and never turns off, so the chip is always fed — **off is the
//! ESP32's deep sleep**, and the button is a plain GPIO the firmware reads. That
//! is what makes every one of the pieces below load-bearing:
//!
//! * `PWR_SENSE` is on GPIO 21 because only RTC GPIOs (0–21 on the S3) can wake
//!   from deep sleep. The press that switches the machine on is an `ext0`
//!   wake-up, and nothing else in the system can produce one.
//! * The two switched rails come up here, not in their own drivers: the µSD's
//!   3V3 and the keyboard's 5 V are both off by default (a pulldown on each
//!   enable), so a card mount or a USB enumeration before [`Rails::bring_up`]
//!   would find dead hardware.
//! * The LED is the only feedback a press gets before the panel catches up —
//!   e-paper needs the better part of a second, and a button that does nothing
//!   for that long reads as broken.
//!
//! Charge and battery numbers come from the BQ25896 over I2C ([`super::bq25896`]).
//! A board where nothing answers on the bus still runs: `status()` stays `None`,
//! the panel shows no battery row, and the button half is unaffected.
//!
//! `PMIC_INT` (GPIO 16) is left alone. The charger pulses it for a few hundred
//! microseconds on a state change, which a polled read cannot catch; the same
//! information is in `REG0B`, which this driver reads anyway. The pin has a
//! board pull-up, so leaving it unclaimed costs nothing.

use std::rc::Rc;
use std::time::Instant;

use app::state_of_charge;
use editor::Battery;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{Gpio21, Gpio38, Gpio40, Gpio41, Input, Output, PinDriver, Pull};
use esp_idf_svc::sys::EspError;

use super::bq25896::Bq25896;
use crate::infrastructure::storage_sd::Storage;

/// Charge current into the 3700 mAh cell: ~0.5 C, the standard rate for a LiPo
/// pouch, and half of what the 150 Ω `ILIM` on the board allows. Provisional —
/// the EEMB 103395's own sheet has not been read, so this is the conservative
/// generic figure until the bench says otherwise.
const ICHG_MA: u16 = 1856;

/// Ceiling the charger's input-current optimiser works down from. The charge
/// port negotiates nothing (CC pull-downs only), so this is not a claim about
/// the source — ICO and VINDPM find what it can actually give.
const IINLIM_CEILING_MA: u16 = 2000;

/// How long the button must be held to switch the machine off. Long enough that
/// a knock or a brush past it cannot reach it, short enough to hold without
/// wondering whether anything is happening.
const LONG_PRESS_MS: u128 = 2000;

/// Contact settling window. The button is a panel switch on a 10 cm pigtail, so
/// it bounces; anything shorter than this is not a press.
const DEBOUNCE_MS: u128 = 30;

/// How often the charger is asked for a fresh sweep. The cell moves on a scale
/// of minutes and each sweep costs an I2C round trip plus the chip's ADC, so
/// polling harder would only spend power to watch power.
const POLL_INTERVAL_MS: u128 = 20_000;

/// Below this the machine saves and switches itself off rather than wait for a
/// brownout in the middle of a write. Above the charger's own 3.0 V cutoff with
/// room for the sag an e-paper refresh causes.
const CRITICAL_MV: u16 = 3300;

/// Consecutive critical readings before the shutdown fires. A panel refresh
/// pulls hundreds of milliamps and drags the terminal voltage down with it, so a
/// single low sample is as likely to be a refresh as a flat cell.
const CRITICAL_SAMPLES: u8 = 3;

/// The two switched rails and the status LED — the outputs that must be driven
/// before anything else in `main` touches the hardware they feed.
///
/// Held for the life of the program: dropping a [`PinDriver`] returns the pin to
/// its reset state, which on both enables means the pulldown wins and the rail
/// dies underneath a running card mount.
pub struct Rails {
    sd_en: PinDriver<'static, Output>,
    kbd_en: PinDriver<'static, Output>,
    led: PinDriver<'static, Output>,
}

impl Rails {
    /// Raise the µSD 3V3 rail, the keyboard 5 V rail and the button LED.
    ///
    /// All three enables are active-high through an N-FET with a 100 kΩ
    /// pulldown, so everything here is off until this runs — and off again the
    /// moment the chip resets or sleeps, with no firmware involved. Returns
    /// after the card's rail has had time to settle, so the caller can mount
    /// straight away.
    pub fn bring_up(
        sd_en: Gpio40<'static>,
        kbd_en: Gpio41<'static>,
        led: Gpio38<'static>,
    ) -> Result<Self, EspError> {
        let mut rails = Self {
            sd_en: PinDriver::output(sd_en)?,
            kbd_en: PinDriver::output(kbd_en)?,
            led: PinDriver::output(led)?,
        };
        rails.sd_en.set_high()?;
        rails.kbd_en.set_high()?;
        rails.led.set_high()?;
        // The P-FET's gate has to swing through a 100 kΩ pull-up and the card
        // needs its own supply ramp before it will answer CMD0.
        FreeRtos::delay_ms(20);
        log::info!("rails up — uSD 3V3, keyboard 5V, button LED");
        Ok(rails)
    }

    fn set_led(&mut self, on: bool) {
        let r = if on { self.led.set_high() } else { self.led.set_low() };
        if let Err(e) = r {
            log::warn!("button LED write FAILED ({e})");
        }
    }

    /// Drop everything this struct switched, on the way into deep sleep.
    fn shed(&mut self) {
        for (what, r) in [
            ("button LED", self.led.set_low()),
            ("keyboard 5V", self.kbd_en.set_low()),
            ("uSD 3V3", self.sd_en.set_low()),
        ] {
            if let Err(e) = r {
                log::warn!("dropping {what} FAILED ({e}); sleeping anyway");
            }
        }
    }
}

/// Where the button is in a press, as the debouncer sees it.
enum Button {
    /// Not pressed, and free to start one.
    Idle,
    /// Pressed; `since` is when the level first read low.
    Down { since: Instant },
    /// Held past [`LONG_PRESS_MS`] and already reported. Nothing more happens
    /// until it is released — which, on a machine that is switching off, it
    /// never is.
    Reported,
    /// Held at boot. A wake-from-sleep press is still down when the firmware
    /// gets here, and a machine that read it as a fresh press would switch
    /// itself back off the moment it woke.
    WaitingForRelease,
}

/// The ADC's one-shot cycle: the sweep takes up to a second, which the writing
/// loop cannot wait on, so the poll starts one pass and collects the next.
enum Adc {
    Idle { next: Instant },
    Converting { started: Instant },
}

/// [`app::Power`] over the mainboard: `PWR_SENSE`, the BQ25896, the LED, and the
/// deep sleep that is this machine's off state.
pub struct EspPower {
    rails: Rails,
    button_pin: PinDriver<'static, Input>,
    /// The charger, or `None` when nothing answered on the bus at boot — a bench
    /// rig, or a mainboard with a cold solder joint on SDA. Everything else here
    /// works without it.
    charger: Option<Bq25896<'static>>,
    /// Kept so the card is flushed and released before its rail drops.
    storage: Rc<Storage>,
    button: Button,
    /// The last level the debouncer accepted, and when it changed.
    level_low: bool,
    level_since: Instant,
    adc: Adc,
    latest: Option<Battery>,
    /// Consecutive readings under [`CRITICAL_MV`].
    critical_run: u8,
    /// Whether the low-battery warning has already been given for this descent.
    /// Cleared when the charge recovers, so a session that plugs in and drains
    /// again is warned twice, not once.
    warned_low: bool,
    /// Set when a long press was reported. If [`poll`](app::Power::poll) is
    /// reached again the shutdown was refused (an unnamed dirty buffer), so the
    /// LED goes back on — the machine is still running and must not claim
    /// otherwise.
    off_reported: bool,
}

impl EspPower {
    /// Assemble the power driver over already-raised [`Rails`].
    ///
    /// `charger` is `None` on a board where nothing answered at the charger's
    /// address — a bench rig, or a cold joint on SDA — and everything except the
    /// battery numbers still works. A charger that *is* there is programmed here
    /// (see [`Bq25896::configure`]) rather than by the caller, so the settings
    /// and the driver that depends on them stay in one place.
    ///
    /// If the button reads pressed, that press is the one that woke the machine:
    /// it is waited out, not acted on.
    pub fn new(
        rails: Rails,
        button_pin: Gpio21<'static>,
        charger: Option<Bq25896<'static>>,
        storage: Rc<Storage>,
    ) -> Result<Self, EspError> {
        let mut charger = charger;
        if let Some(chip) = charger.as_mut() {
            if let Err(e) = chip.configure(ICHG_MA, IINLIM_CEILING_MA) {
                log::warn!("charger configure FAILED ({e}); it keeps its power-on defaults");
            }
        }
        // The contact shorts `PWR_SENSE` to ground through 10 kΩ, so the pin
        // needs the internal pull-up to read high when nothing is pressed.
        let button_pin = PinDriver::input(button_pin, Pull::Up)?;
        let held = button_pin.is_low();
        if held {
            log::info!("button still held at boot (the press that woke us) — waiting for release");
        }
        Ok(Self {
            rails,
            button_pin,
            charger,
            storage,
            button: if held { Button::WaitingForRelease } else { Button::Idle },
            level_low: held,
            level_since: Instant::now(),
            adc: Adc::Idle { next: Instant::now() },
            latest: None,
            critical_run: 0,
            warned_low: false,
            off_reported: false,
        })
    }

    /// The debounced button level: `true` while pressed. A level that has not
    /// held for [`DEBOUNCE_MS`] keeps the previous answer.
    fn debounced_low(&mut self) -> bool {
        let raw = self.button_pin.is_low();
        if raw != self.level_low {
            if self.level_since.elapsed().as_millis() >= DEBOUNCE_MS {
                self.level_low = raw;
                self.level_since = Instant::now();
            }
        } else {
            self.level_since = Instant::now();
        }
        self.level_low
    }

    /// Advance the button state machine. Returns the press event, if any.
    fn poll_button(&mut self) -> Option<app::PowerEvent> {
        let down = self.debounced_low();
        match (&self.button, down) {
            (Button::WaitingForRelease, true) => None,
            (Button::WaitingForRelease, false) => {
                self.button = Button::Idle;
                None
            }
            (Button::Idle, true) => {
                self.button = Button::Down { since: Instant::now() };
                None
            }
            (Button::Idle, false) => None,
            (Button::Down { since }, true) => {
                if since.elapsed().as_millis() >= LONG_PRESS_MS {
                    self.button = Button::Reported;
                    self.rails.set_led(false);
                    self.off_reported = true;
                    Some(app::PowerEvent::OffAsked)
                } else {
                    None
                }
            }
            (Button::Down { .. }, false) => {
                self.button = Button::Idle;
                Some(app::PowerEvent::StatusAsked)
            }
            (Button::Reported, true) => None,
            (Button::Reported, false) => {
                self.button = Button::Idle;
                None
            }
        }
    }

    /// Run the charger's one-shot ADC cycle, and fold a finished sweep into
    /// [`latest`](Self::latest). Returns a cell event when the reading crosses a
    /// threshold.
    fn poll_charger(&mut self) -> Option<app::PowerEvent> {
        let charger = self.charger.as_mut()?;
        match self.adc {
            Adc::Idle { next } if Instant::now() >= next => {
                if let Err(e) = charger.start_conversion() {
                    log::warn!("charger ADC start FAILED ({e}); retrying at the next poll");
                    self.adc = Adc::Idle { next: Instant::now() + ms(POLL_INTERVAL_MS) };
                    return None;
                }
                self.adc = Adc::Converting { started: Instant::now() };
                None
            }
            Adc::Idle { .. } => None,
            Adc::Converting { started } => {
                match charger.conversion_done() {
                    Ok(true) => {}
                    // A sweep is specified to land inside a second; past two the
                    // chip has been reset out from under us (a `/QON` press, a
                    // VBUS glitch) and the start bit is gone with our settings.
                    Ok(false) if started.elapsed().as_millis() < 2000 => return None,
                    Ok(false) => {
                        log::warn!("charger ADC never finished — reprogramming");
                        let _ = charger.configure(ICHG_MA, IINLIM_CEILING_MA);
                        self.adc = Adc::Idle { next: Instant::now() + ms(POLL_INTERVAL_MS) };
                        return None;
                    }
                    Err(e) => {
                        log::warn!("charger ADC poll FAILED ({e})");
                        self.adc = Adc::Idle { next: Instant::now() + ms(POLL_INTERVAL_MS) };
                        return None;
                    }
                }
                self.adc = Adc::Idle { next: Instant::now() + ms(POLL_INTERVAL_MS) };
                let telemetry = match charger.telemetry() {
                    Ok(t) => t,
                    Err(e) => {
                        log::warn!("charger telemetry read FAILED ({e})");
                        return None;
                    }
                };
                let faults = charger.faults().unwrap_or(0);
                if faults != 0 {
                    log::warn!("charger fault register (latched since last read): {faults:#04x}");
                }
                let charging = telemetry.state.charging();
                let percent = state_of_charge(telemetry.vbat_mv, telemetry.ichg_ma, charging);
                log::info!(
                    "battery {percent}% — VBAT {} mV, SYS {} mV, VBUS {}, charge {} mA, {:?}",
                    telemetry.vbat_mv,
                    telemetry.vsys_mv,
                    telemetry.vbus_mv.map_or_else(|| "none".into(), |v| format!("{v} mV")),
                    telemetry.ichg_ma,
                    telemetry.state,
                );
                self.latest = Some(Battery { percent, charging });
                self.cell_event(telemetry.vbat_mv, percent, telemetry.power_good)
            }
        }
    }

    /// Decide whether a fresh reading is worth interrupting the writer for.
    fn cell_event(&mut self, vbat_mv: u16, percent: u8, powered: bool) -> Option<app::PowerEvent> {
        // On a cable, nothing is urgent: the cell is going up, and a low reading
        // is the charge starting rather than the session ending.
        if powered {
            self.critical_run = 0;
            if percent >= Battery::LOW_PERCENT {
                self.warned_low = false;
            }
            return None;
        }
        if vbat_mv <= CRITICAL_MV {
            self.critical_run = self.critical_run.saturating_add(1);
            if self.critical_run >= CRITICAL_SAMPLES {
                return Some(app::PowerEvent::Critical);
            }
            return None;
        }
        self.critical_run = 0;
        if percent < Battery::LOW_PERCENT {
            if !self.warned_low {
                self.warned_low = true;
                return Some(app::PowerEvent::Low);
            }
        } else {
            self.warned_low = false;
        }
        None
    }
}

impl app::Power for EspPower {
    fn poll(&mut self) -> Option<app::PowerEvent> {
        // Being polled after a reported long press means the loop refused the
        // shutdown (see `Editor::request_power_off`) — the machine is still on,
        // so put the light back. The real shutdown never comes back here.
        if self.off_reported {
            self.off_reported = false;
            self.rails.set_led(true);
        }
        // The button first: a held button outranks anything the cell has to say,
        // and only one event leaves per pass.
        self.poll_button().or_else(|| self.poll_charger())
    }

    fn status(&self) -> Option<Battery> {
        self.latest
    }

    fn power_off(&mut self) -> ! {
        log::info!("shutting down: unmounting the card, shedding the rails, entering deep sleep");
        self.storage.unmount();
        self.rails.shed();

        // The press that switched the machine off is very likely still down.
        // Arming `ext0` on a pin that already reads low wakes the chip the
        // instant it sleeps, so wait the finger out — but not forever: a stuck
        // contact must still end in a sleep, not a spin.
        let mut waited = 0;
        while self.button_pin.is_low() && waited < RELEASE_WAIT_MS {
            FreeRtos::delay_ms(10);
            waited += 10;
        }
        if waited >= RELEASE_WAIT_MS {
            log::warn!("button still down after {RELEASE_WAIT_MS} ms — sleeping regardless");
        }

        // SAFETY: three esp-idf C entry points taking a plain pin number, and
        // nothing in this process runs after the last one.
        unsafe {
            // `PWR_SENSE` low is the wake-up, and it is the only one armed: this
            // machine has no other way back on, which is what makes the button
            // the power switch. The pull-up goes on *after* arming — enabling
            // ext0 reconfigures the pad, and it would undo a pull set before it,
            // leaving the wake input floating for the whole sleep.
            esp_idf_svc::sys::esp_sleep_enable_ext0_wakeup(PIN_PWR_SENSE, 0);
            esp_idf_svc::sys::rtc_gpio_pullup_en(PIN_PWR_SENSE);
            esp_idf_svc::sys::rtc_gpio_pulldown_dis(PIN_PWR_SENSE);
            esp_idf_svc::sys::esp_deep_sleep_start()
        }
    }
}

/// `PWR_SENSE` — the power button, and the only deep-sleep wake source. Named
/// as a raw number because arming `ext0` is a C call, not a `PinDriver`
/// operation. Must stay an RTC GPIO (0–21 on the S3); see the module docs.
const PIN_PWR_SENSE: i32 = 21;

/// How long the shutdown waits for the button to come back up before sleeping
/// anyway.
const RELEASE_WAIT_MS: u32 = 10_000;

fn ms(millis: u128) -> std::time::Duration {
    std::time::Duration::from_millis(millis as u64)
}
