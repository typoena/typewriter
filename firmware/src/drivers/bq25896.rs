//! BQ25896 charger / power path — the register half, over I2C.
//!
//! The chip owns the whole power path on the mainboard: it selects between the
//! USB-C input and the cell, limits input current, runs the 1 µH buck to SYS
//! (which feeds the 3V3 buck-boost), and charges through its internal BATFET.
//! There is no discrete FET or diode in that path and no separate fuel gauge —
//! so this driver is also the only source of battery telemetry the firmware has.
//!
//! Register semantics and the field encodings below are from the datasheet
//! (SLUSC76C, §9.5 Register Map). Board-side values — the 150 Ω `ILIM`, the
//! fixed `TS` divider standing in for a cell with no thermistor, the hard-wired
//! `PSEL`/`OTG`/`/CE` levels — are in
//! `hardware/pcb/DESIGN-NOTES.md`.
//!
//! Two hazards live here and nowhere else:
//!
//! * **Ship mode is unreachable.** `REG09[5]` (`BATFET_DIS`) would give a true
//!   zero-draw off state, but the only thing that brings the chip back out of it
//!   is `/QON` or a fresh adapter — and `/QON` is `SW1`, a board-mounted button
//!   inside the case, not the user's power button. Setting it would leave a
//!   machine the writer cannot switch on. The off state is the ESP32's deep
//!   sleep instead (see [`super::power_esp`]).
//! * **The I2C watchdog must stay off.** Its default is 40 s, and it reloads
//!   every writable register with its power-on default when it expires — which
//!   it would, every time, since the MCU spends hours in deep sleep and nothing
//!   kicks it. [`Bq25896::configure`] disables it so the settings below survive.

use esp_idf_svc::hal::i2c::I2cDriver;
use esp_idf_svc::sys::EspError;

/// 7-bit address, fixed in silicon (datasheet §9.5.1).
const ADDR: u8 = 0x6B;

/// Bus timeout. Every transfer here is a 1- or 2-byte register access on a bus
/// with one device, so anything slower than this is a stuck bus, not traffic.
const TIMEOUT_MS: u32 = 50;

// Register map — only the ones this driver touches.
const REG00_INPUT_LIMIT: u8 = 0x00;
const REG02_ADC_CTRL: u8 = 0x02;
const REG04_CHARGE_CURRENT: u8 = 0x04;
const REG07_TIMERS: u8 = 0x07;
const REG0B_STATUS: u8 = 0x0B;
const REG0C_FAULT: u8 = 0x0C;
const REG0E_BATV: u8 = 0x0E;
const REG0F_SYSV: u8 = 0x0F;
const REG11_VBUSV: u8 = 0x11;
const REG12_ICHGR: u8 = 0x12;
const REG14_PART: u8 = 0x14;

/// `REG02[7]` — start one ADC conversion. Self-clearing: the chip drops it when
/// the conversion lands, which is how [`Bq25896::conversion_done`] reads.
const CONV_START: u8 = 1 << 7;
/// `REG02[6]` — continuous 1 Hz conversion. Deliberately never set: it keeps the
/// REGN LDO and the ADC alive, and the standby budget (~84 µA for the whole
/// machine) has no room for that. One-shots only.
const CONV_RATE: u8 = 1 << 6;
/// `REG02[4]` — input current optimizer. The charge port is a plain USB-C sink
/// (two 5.1 kΩ CC pull-downs, no negotiation), so the firmware cannot know what
/// the source can give. ICO finds out: it ramps the input up until VBUS sags
/// into VINDPM, then holds one step below.
const ICO_EN: u8 = 1 << 4;

/// Charge current, `REG04[6:0]`, 64 mA per step from 0.
const ICHG_STEP_MA: u16 = 64;
/// Input current limit, `REG00[5:0]`, 50 mA per step above a 100 mA offset.
const IINLIM_STEP_MA: u16 = 50;
const IINLIM_OFFSET_MA: u16 = 100;
/// `REG0E`/`REG0F` battery and system voltage: 20 mV per step above 2.304 V.
const V_STEP_MV: u16 = 20;
const V_OFFSET_MV: u16 = 2304;
/// `REG11` VBUS: 100 mV per step above 2.6 V.
const VBUS_STEP_MV: u16 = 100;
const VBUS_OFFSET_MV: u16 = 2600;
/// `REG12` measured charge current: 50 mA per step from 0.
const ICHGR_STEP_MA: u16 = 50;

/// What the charger is doing with the cell right now (`REG0B[4:3]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    NotCharging,
    PreCharge,
    FastCharge,
    Done,
}

impl ChargeState {
    /// Whether current is going *into* the cell. `Done` is not charging: the
    /// charger has terminated and the machine runs off the input.
    pub fn charging(self) -> bool {
        matches!(self, Self::PreCharge | Self::FastCharge)
    }
}

/// One ADC sweep, decoded.
#[derive(Debug, Clone, Copy)]
pub struct Telemetry {
    /// Cell terminal voltage. Not the resting voltage while current flows —
    /// see `power_esp`'s state-of-charge estimate for the correction.
    pub vbat_mv: u16,
    /// The SYS rail the 3V3 buck-boost runs off.
    pub vsys_mv: u16,
    /// Input voltage, or `None` with no source plugged in.
    pub vbus_mv: Option<u16>,
    /// Charge current into the cell.
    pub ichg_ma: u16,
    pub state: ChargeState,
    /// `REG0B[2]` — the input is present and within its good window.
    pub power_good: bool,
}

/// The charger, addressed over a bus this driver owns.
pub struct Bq25896<'d> {
    i2c: I2cDriver<'d>,
}

impl<'d> Bq25896<'d> {
    /// Claim the charger on `i2c`, or report that nothing answered at
    /// [`ADDR`] — which is what a bench rig without the mainboard looks like,
    /// and is not fatal anywhere: the caller degrades to a machine with no
    /// battery telemetry.
    pub fn new(i2c: I2cDriver<'d>) -> Result<Self, EspError> {
        let mut chip = Self { i2c };
        let part = chip.read(REG14_PART)?;
        // Logged, not asserted: `REG14[5:3]` is the part number and `[1:0]` the
        // revision, and a board that answers at 0x6B at all is the charger. A
        // number that surprises belongs in the bench log, not in a refusal to
        // charge.
        log::info!(
            "BQ25896 found at {ADDR:#04x} — REG14 {part:#04x} (part {}, rev {})",
            (part >> 3) & 0b111,
            part & 0b11
        );
        Ok(chip)
    }

    /// Program the settings that must not be left at their power-on defaults,
    /// and leave every other register alone.
    ///
    /// `ichg_ma` is the charge current into the cell; `iinlim_ma` is the ceiling
    /// [`ICO_EN`] optimises *down* from, not a promise the source can deliver
    /// it. What stays default and why:
    ///
    /// * `VREG` 4.208 V, `VRECHG`, `ITERM`, `IPRECHG` — the defaults are the
    ///   single-cell LiPo values; writing them back would only add a place for
    ///   them to drift out of step with the datasheet.
    /// * `/CE` is strapped low on the board, so the chip already charges at its
    ///   defaults before this runs — including on a first power-up that never
    ///   reaches firmware.
    /// * `BATFET_DIS` stays clear. See the module docs: setting it switches the
    ///   machine off in a way the user's button cannot undo.
    pub fn configure(&mut self, ichg_ma: u16, iinlim_ma: u16) -> Result<(), EspError> {
        // Watchdog off FIRST — every write below is only durable once it is.
        // `REG07[5:4]` = 00.
        self.modify(REG07_TIMERS, 0b0011_0000, 0)?;

        let ichg = (ichg_ma / ICHG_STEP_MA).min(0x7F) as u8;
        self.modify(REG04_CHARGE_CURRENT, 0x7F, ichg)?;

        let iinlim =
            (iinlim_ma.saturating_sub(IINLIM_OFFSET_MA) / IINLIM_STEP_MA).min(0x3F) as u8;
        self.modify(REG00_INPUT_LIMIT, 0x3F, iinlim)?;

        // ADC on demand only, and let ICO discover the real input ceiling.
        self.modify(REG02_ADC_CTRL, CONV_START | CONV_RATE | ICO_EN, ICO_EN)?;

        log::info!(
            "BQ25896 configured — charge {} mA, input ceiling {} mA (ICO on), I2C watchdog off",
            u16::from(ichg) * ICHG_STEP_MA,
            u16::from(iinlim) * IINLIM_STEP_MA + IINLIM_OFFSET_MA
        );
        Ok(())
    }

    /// Kick off one ADC sweep. Takes up to a second to land, so the caller
    /// starts it and comes back for the result rather than blocking the writing
    /// loop on the bus.
    pub fn start_conversion(&mut self) -> Result<(), EspError> {
        self.modify(REG02_ADC_CTRL, CONV_START, CONV_START)
    }

    /// Whether the sweep started by [`start_conversion`](Self::start_conversion)
    /// has finished — the chip clears `CONV_START` itself when it has.
    pub fn conversion_done(&mut self) -> Result<bool, EspError> {
        Ok(self.read(REG02_ADC_CTRL)? & CONV_START == 0)
    }

    /// Read the finished sweep plus the charge state that goes with it.
    pub fn telemetry(&mut self) -> Result<Telemetry, EspError> {
        let status = self.read(REG0B_STATUS)?;
        let vbat = self.read(REG0E_BATV)? & 0x7F;
        let vsys = self.read(REG0F_SYSV)? & 0x7F;
        let vbus_raw = self.read(REG11_VBUSV)?;
        let ichg = self.read(REG12_ICHGR)? & 0x7F;

        let state = match (status >> 3) & 0b11 {
            0b01 => ChargeState::PreCharge,
            0b10 => ChargeState::FastCharge,
            0b11 => ChargeState::Done,
            _ => ChargeState::NotCharging,
        };
        Ok(Telemetry {
            vbat_mv: V_OFFSET_MV + u16::from(vbat) * V_STEP_MV,
            vsys_mv: V_OFFSET_MV + u16::from(vsys) * V_STEP_MV,
            // `REG11[7]` is VBUS_GD; with no source attached the field reads
            // back as a floor value rather than nothing, so the flag — not the
            // number — is what says whether a cable is in.
            vbus_mv: (vbus_raw & 0x80 != 0)
                .then(|| VBUS_OFFSET_MV + u16::from(vbus_raw & 0x7F) * VBUS_STEP_MV),
            ichg_ma: u16::from(ichg) * ICHGR_STEP_MA,
            state,
            power_good: status & 0b100 != 0,
        })
    }

    /// The fault register (`REG0C`), which latches: reading it clears the
    /// latched bits, so a non-zero value is "something went wrong since the last
    /// read", not "something is wrong now".
    pub fn faults(&mut self) -> Result<u8, EspError> {
        self.read(REG0C_FAULT)
    }

    fn read(&mut self, reg: u8) -> Result<u8, EspError> {
        let mut buf = [0u8; 1];
        self.i2c.write_read(ADDR, &[reg], &mut buf, TIMEOUT_MS)?;
        Ok(buf[0])
    }

    fn write(&mut self, reg: u8, value: u8) -> Result<(), EspError> {
        self.i2c.write(ADDR, &[reg, value], TIMEOUT_MS)
    }

    /// Read-modify-write the bits in `mask`, leaving the rest of the register as
    /// found. Most of these registers mix unrelated fields, so a blind write
    /// would clobber a neighbour — `REG02` alone carries the ADC controls, the
    /// boost frequency and the D+/D− detection.
    fn modify(&mut self, reg: u8, mask: u8, value: u8) -> Result<(), EspError> {
        let current = self.read(reg)?;
        let next = (current & !mask) | (value & mask);
        if next != current {
            self.write(reg, next)?;
        }
        Ok(())
    }
}
