//! Thin SSD1683 driver for the GDEY0579T93 (792×272) e-paper panel.
//!
//! This panel is a *dual-controller* device: 792×272 exceeds one SSD1683's
//! 400×300 limit, so it is driven as a **master** (command offset `0x00`) +
//! **slave** (`0x80`) pair, with the framebuffer split between them. The
//! command sequences and RAM-window math are ported faithfully from GxEPD2's
//! `GxEPD2_579_GDEY0579T93` (Jean-Marc Zingg), itself based on the Good
//! Display factory demo. See `docs/v0.1-mvp-technical.md` (Spike 2) and
//! ADR-003.
//!
//! Capabilities: hardware reset, init, uniform fill, full-frame blit via an
//! `embedded-graphics` `DrawTarget` (`Frame`), full refresh (`display_frame`),
//! and partial refresh (`display_frame_partial`) — Spikes 2 and 5.

use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{Input, Output, PinDriver};
use esp_idf_svc::hal::spi::{SpiBusDriver, SpiDriver};
use esp_idf_svc::sys::EspError;

// Panel geometry and the drawable `Frame` live in the `display` crate (so the
// editor can render off the xtensa target); re-exported so `epd::HEIGHT` etc.
// keep resolving.
pub use display::{FB_BYTES, FB_BYTES_W, HEIGHT, WIDTH};

/// Each controller drives one half. SSD1683 X is byte-addressed; 396 px
/// rounds up to 50 bytes (400 px) of RAM width, full panel height (272 rows).
const CTRL_BYTES_W: usize = 50;
const CTRL_BYTES: usize = CTRL_BYTES_W * HEIGHT as usize; // 50 * 272 = 13600

/// Max bytes per SPI transfer; matches the DMA size configured in `main`.
const SPI_CHUNK: usize = 4096;

/// Temperature override written to `0x1A` before each partial; `None` = leave
/// init's `[0x64, 0x00]`, no per-partial write.
///
/// CLOSED 2026-07-17: hot/cold sweeps never moved BUSY time — the OTP partial
/// LUT is temperature-independent here, not a latency lever. Scaffolding kept so
/// the closed result stays next to the driver. Log:
/// docs/tradeoff-curves/epd-refresh-latency.md.
const PARTIAL_TEMP: Option<[u8; 2]> = None;

/// Settle after each RAM-window set (8 per partial refresh). The GxEPD2 port's
/// `delay_ms(2)` rounded up to a whole 10 ms FreeRTOS tick (~40 ms/refresh);
/// the controller latches the window when the SPI transaction completes, so 0
/// is safe (verified 2026-07-17, −70 ms windowed). Raise to 1 tick only if band
/// corruption ever appears. Log: docs/tradeoff-curves/epd-refresh-latency.md.
const RAM_SETTLE_MS: u32 = 0;

/// Fast partial-refresh waveform — the per-keystroke typing-latency lever.
/// Loaded into `0x32` by [`Epd::update_part_fast`] instead of the factory OTP
/// partial (~495 ms BUSY floor → ~265 ms windowed, validated on hardware
/// 2026-07-21). Still gated behind the `fast_partial` pref: longevity soak and
/// cold-temperature check outstanding — the speed spends the vendor's drive
/// margin, which shrinks when cold. Full narrative and FR sweep:
/// docs/tradeoff-curves/epd-refresh-latency.md.
///
/// Provenance: Good Display's own `LUT_DATA_part` for this panel, preserved
/// verbatim in `reference/gdey0579t93-fp-lut/Display_EPD_W21.c`.
///
/// Layout (233 bytes): `[0..227)` is the `0x32` phase table — 7-byte phase rows,
/// byte[0] = frame count, bytes[1..3] = packed 2-bit level codes, byte[5] =
/// repeat; FR/XON at `[224..227)`. The trailing 6 are drive config fanned out to
/// their own registers by `update_part_fast`: EOPT (`0x3F`), VGH (`0x03`),
/// VSH1/VSH2/VSL (`0x04`), VCOM (`0x2C`).
// The weakest tail phase of each group is zeroed (TRIMMED) — a measured ~2%
// near-noop, kept only because it's harmless. The FR byte is the real lever.
const FAST_PARTIAL_LUT: [u8; 233] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x18, 0x01, 0x00, 0x00, 0x01, 0x00, // g1 main drive
    0x01, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, // g1 follow-up
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // g1 tail — TRIMMED (was 0x01,0x01,0x00…)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x58, 0x41, 0x00, 0x00, 0x01, 0x00, // g2 main drive
    0x01, 0x41, 0x00, 0x00, 0x00, 0x01, 0x00, // g2 follow-up drive
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // g2 tail — TRIMMED (was 0x01,0x01,0x00…)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x98, 0x81, 0x00, 0x00, 0x01, 0x00, // g3 main drive
    0x01, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00, // g3 follow-up drive
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // g3 tail — TRIMMED (was 0x01,0x01,0x00…)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x18, 0x41, 0x00, 0x00, 0x01, 0x00, // g4 main drive
    0x01, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, // g4 follow-up
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // g4 tail — TRIMMED (was 0x01,0x01,0x00…)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // FR, XON — FR scales the waveform clock: vendor 0x04 ≈ 420 ms, 0x08 ≈ 265 ms
    // windowed, still solid black. Non-monotonic; don't raise past 0x08 blind.
    0x08, 0x00, 0x00,
    // EOPT, VGH, VSH1, VSH2, VSL, VCOM
    0x06, 0x17, 0x41, 0xA8, 0x32, 0x00,
];

/// `0x22` (Display Update Control 2) value for the fast partial: enable clock +
/// analog, DISPLAY in Mode 2 with the LUT *already written* via `0x32`, power
/// down — crucially **without** the load-temp / load-LUT-from-OTP bits of the
/// factory partial's `0xFF`, which would overwrite [`FAST_PARTIAL_LUT`].
const FAST_PART_UPDATE: u8 = 0xCF;

/// [`FAST_PART_UPDATE`] minus the disable-analog (`0x02`) and disable-clock
/// (`0x01`) bits: the charge pump and oscillator stay powered after the refresh.
const FAST_PART_UPDATE_HOT: u8 = 0xCC;

/// Keep the ±15 V charge pump energized between fast partials (`0xCC` instead
/// of `0xCF`) so burst keystrokes skip the booster soft-start.
///
/// Bench 2026-07-21: no win — the first partial after a power-down was no slower
/// than mid-burst, so the ~240 ms floor is waveform BUSY, not pump ramp. Safe but
/// pointless, and holding the rails costs idle draw. Left as a toggle for a
/// future A/B. Log: docs/tradeoff-curves/epd-refresh-latency.md.
const FAST_PART_KEEP_HOT: bool = false;

pub struct Epd<'d> {
    spi: SpiBusDriver<'d, SpiDriver<'d>>,
    dc: PinDriver<'d, Output>,
    rst: PinDriver<'d, Output>,
    cs: PinDriver<'d, Output>,
    busy: PinDriver<'d, Input>,
    /// A refresh kicked off by `display_frame_async` whose waveform may still
    /// be running. Every public display call waits it out (`wait_ready`)
    /// before sending further controller traffic.
    refresh_pending: bool,
    /// The custom fast-partial recipe ([`FAST_PARTIAL_LUT`] + drive voltages)
    /// is resident in the controllers. Load-LUT activations do NOT displace it
    /// (scrolls doubled on the weakened waveform, 2026-07-25), so every
    /// non-fast refresh runs [`evict_fast_recipe`](Self::evict_fast_recipe)
    /// before its RAM writes.
    fast_lut_loaded: bool,
}

/// Caller-contract guard for the display entry points: a wrong-size
/// framebuffer or row window returns `ESP_ERR_INVALID_SIZE` instead of
/// panicking (a panic reboots the device).
fn contract(ok: bool) -> Result<(), EspError> {
    if ok {
        Ok(())
    } else {
        Err(EspError::from_infallible::<{ esp_idf_svc::sys::ESP_ERR_INVALID_SIZE }>())
    }
}

impl<'d> Epd<'d> {
    pub fn new(
        spi: SpiBusDriver<'d, SpiDriver<'d>>,
        dc: PinDriver<'d, Output>,
        rst: PinDriver<'d, Output>,
        cs: PinDriver<'d, Output>,
        busy: PinDriver<'d, Input>,
    ) -> Self {
        Self { spi, dc, rst, cs, busy, refresh_pending: false, fast_lut_loaded: false }
    }

    /// Wait out a refresh started by `display_frame_async`, if one is still
    /// running. Safe to call anytime; a no-op when nothing is pending.
    pub fn wait_ready(&mut self) -> Result<(), EspError> {
        if self.refresh_pending {
            self.wait_while_busy(2500)?; // full_refresh_time ≈ 2200 ms
            self.refresh_pending = false;
        }
        Ok(())
    }

    // SPI framing: DC low = command, DC high = data.
    fn cmd(&mut self, c: u8) -> Result<(), EspError> {
        self.dc.set_low()?;
        self.cs.set_low()?;
        self.spi.write(&[c])?;
        self.cs.set_high()?;
        Ok(())
    }

    fn data(&mut self, bytes: &[u8]) -> Result<(), EspError> {
        self.dc.set_high()?;
        self.cs.set_low()?;
        for chunk in bytes.chunks(SPI_CHUNK) {
            self.spi.write(chunk)?;
        }
        self.cs.set_high()?;
        Ok(())
    }

    /// BUSY is active-HIGH on this panel (GxEPD2 constructs with `HIGH`).
    fn wait_while_busy(&mut self, timeout_ms: u32) -> Result<(), EspError> {
        let mut waited = 0;
        while self.busy.is_high() {
            FreeRtos::delay_ms(1);
            waited += 1;
            if waited >= timeout_ms {
                log::warn!("EPD BUSY still high after {timeout_ms} ms — continuing");
                break;
            }
        }
        Ok(())
    }

    /// Hardware reset (RST is active-low). ~20 ms pulses per GxEPD2 default.
    pub fn reset(&mut self) -> Result<(), EspError> {
        self.rst.set_high()?;
        FreeRtos::delay_ms(20);
        self.rst.set_low()?;
        FreeRtos::delay_ms(20);
        self.rst.set_high()?;
        FreeRtos::delay_ms(20);
        self.wait_while_busy(100)?;
        Ok(())
    }

    /// Port of GxEPD2 `_InitDisplay` (B/W mode). The `0x20` master
    /// activations load the temperature value and LUT.
    pub fn init(&mut self) -> Result<(), EspError> {
        self.cmd(0x12)?; // SWRESET
        FreeRtos::delay_ms(10);
        self.wait_while_busy(100)?;
        self.cmd(0x18)?; // temperature sensor control
        self.data(&[0x80])?; // internal sensor
        self.cmd(0x22)?; // display update control 2
        self.data(&[0xB1])?; // enable clock, load temp, load LUT (B/W), disable clock
        self.cmd(0x20)?; // master activation
        FreeRtos::delay_ms(10);
        self.wait_while_busy(100)?;
        self.cmd(0x1A)?; // write to temperature register
        self.data(&[0x64, 0x00])?;
        self.cmd(0x22)?;
        self.data(&[0x91])?; // load temp, load LUT (B/W), disable clock
        self.cmd(0x20)?;
        FreeRtos::delay_ms(10);
        self.wait_while_busy(100)?;
        self.fast_lut_loaded = false; // reset + OTP reload evicted any custom recipe
        Ok(())
    }

    /// Port of GxEPD2 `_setPartialRamArea`. `target` is `0x00` (master) or
    /// `0x80` (slave); `mode` selects X/Y increment/decrement (0x00–0x03).
    fn set_ram_area(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        mode: u8,
        target: u8,
    ) -> Result<(), EspError> {
        self.cmd(0x11 | target)?; // data entry mode
        self.data(&[mode])?;
        let xl = (x / 8) as u8;
        let xh = ((x + w - 1) / 8) as u8;
        let ys = [(y % 256) as u8, (y / 256) as u8];
        let ye = [((y + h - 1) % 256) as u8, ((y + h - 1) / 256) as u8];
        match mode {
            0x03 => {
                // X increment, Y increment
                self.cmd(0x44 | target)?;
                self.data(&[xl, xh])?;
                self.cmd(0x45 | target)?;
                self.data(&[ys[0], ys[1], ye[0], ye[1]])?;
                self.cmd(0x4E | target)?;
                self.data(&[xl])?;
                self.cmd(0x4F | target)?;
                self.data(&[ys[0], ys[1]])?;
            }
            0x02 => {
                // X decrement, Y increment
                self.cmd(0x44 | target)?;
                self.data(&[xh, xl])?;
                self.cmd(0x45 | target)?;
                self.data(&[ys[0], ys[1], ye[0], ye[1]])?;
                self.cmd(0x4E | target)?;
                self.data(&[xh])?;
                self.cmd(0x4F | target)?;
                self.data(&[ys[0], ys[1]])?;
            }
            0x01 => {
                // X increment, Y decrement
                self.cmd(0x44 | target)?;
                self.data(&[xl, xh])?;
                self.cmd(0x45 | target)?;
                self.data(&[ye[0], ye[1], ys[0], ys[1]])?;
                self.cmd(0x4E | target)?;
                self.data(&[xl])?;
                self.cmd(0x4F | target)?;
                self.data(&[ye[0], ye[1]])?;
            }
            _ => {
                // 0x00: X decrement, Y decrement
                self.cmd(0x44 | target)?;
                self.data(&[xh, xl])?;
                self.cmd(0x45 | target)?;
                self.data(&[ye[0], ye[1], ys[0], ys[1]])?;
                self.cmd(0x4E | target)?;
                self.data(&[xh])?;
                self.cmd(0x4F | target)?;
                self.data(&[ye[0], ye[1]])?;
            }
        }
        // Always-false at the current 0, kept so re-tuning the constant
        // re-enables the delay (see its doc comment).
        #[allow(clippy::absurd_extreme_comparisons)]
        if RAM_SETTLE_MS > 0 {
            FreeRtos::delay_ms(RAM_SETTLE_MS);
        }
        Ok(())
    }

    /// Fill one RAM bank (`0x24` current or `0x26` previous) on both
    /// controllers with a constant byte. One clean full-coverage window per
    /// controller (slave = left half `0x80`, master = right half `0x00`) —
    /// simpler and more complete than GxEPD2's overlapping-window fill, which
    /// only matters for a constant value anyway.
    fn write_buffer(&mut self, command: u8, value: u8) -> Result<(), EspError> {
        let buf = vec![value; CTRL_BYTES];
        for target in [0x80u8, 0x00u8] {
            self.set_ram_area(0, 0, 400, HEIGHT, 0x03, target)?;
            self.cmd(command | target)?;
            self.data(&buf)?;
        }
        Ok(())
    }

    /// Port of GxEPD2 `refresh(false)` → `_Update_Full` (fast full update).
    fn update_full(&mut self) -> Result<(), EspError> {
        self.kick_update_full()?;
        self.wait_while_busy(2500)?; // full_refresh_time ≈ 2200 ms
        Ok(())
    }

    /// The command half of `update_full`: starts the full-refresh waveform
    /// (~2.2 s) and returns while it runs. The caller owns the eventual BUSY
    /// wait before any further controller traffic.
    ///
    /// `0x21 ← 0x40` bypasses the RED/"previous" bank as 0. Do NOT run this
    /// kick with the RED bank participating (`0x21 ← 0x00`): both bank
    /// orderings came out photo-negative and corrupted subsequent partials
    /// (2026-07-25, v4/v5 in docs/tradeoff-curves/epd-refresh-latency.md).
    fn kick_update_full(&mut self) -> Result<(), EspError> {
        self.set_ram_area(0, 0, WIDTH / 2, HEIGHT, 0x03, 0x80)?; // slave
        self.set_ram_area(0, 0, WIDTH / 2, HEIGHT, 0x03, 0x00)?; // master
        self.cmd(0x21)?; // display update control 1
        self.data(&[0x40, 0x10])?; // bypass RED as 0, cascade
        self.cmd(0x1A)?; // temperature register (fast full update)
        self.data(&[0x64, 0x00])?;
        self.cmd(0x22)?;
        self.data(&[0xD7])?; // fast full update
        self.cmd(0x20)?; // master activation
        Ok(())
    }

    /// Port of GxEPD2 `_Update_Part` — the partial-update waveform: no full
    /// flashing; only pixels that differ between the "previous" (`0x26`) and
    /// "current" (`0x24`) banks transition. `y0`/`h` restrict the RAM window
    /// (and thus the SPI transfer) to a band of rows; the *waveform* still
    /// drives the whole panel.
    ///
    /// Do NOT try to restrict the gate scan to the band (`0x01` MUX + `0x0F`
    /// scan start): BUSY time doesn't scale with MUX, and writing `0x01` risks
    /// clobbering the write-only OTP gate config — refuted on hardware, see
    /// docs/postmortems/2026-07-16-gate-scan-spike-refuted.md.
    fn update_part(&mut self, y0: u16, h: u16) -> Result<(), EspError> {
        self.set_ram_area(0, y0, WIDTH / 2, h, 0x03, 0x80)?; // slave
        self.set_ram_area(0, y0, WIDTH / 2, h, 0x03, 0x00)?; // master
        self.cmd(0x3C)?; // border waveform control
        self.data(&[0x80])?; // VCOM
        self.cmd(0x21)?; // display update control 1
        self.data(&[0x00, 0x10])?; // RED normal, cascade
        if let Some(temp) = PARTIAL_TEMP {
            // Closed experiment — see PARTIAL_TEMP. The 0xFF kick below reloads
            // temp+LUT, so this takes effect on this activation.
            self.cmd(0x1A)?; // write to temperature register
            self.data(&temp)?;
        }
        self.cmd(0x22)?; // display update control 2
        self.data(&[0xFF])?; // partial update (incl. load-temp + load-LUT)
        self.cmd(0x20)?; // master activation
        self.wait_while_busy(2000)?; // partial is well under the full ~2.2 s
        Ok(())
    }

    /// [`update_part`](Self::update_part)'s fast twin: loads [`FAST_PARTIAL_LUT`]
    /// via `0x32` and triggers with [`FAST_PART_UPDATE`] so the panel displays
    /// with *that* LUT instead of reloading the ~540 ms OTP one. Reached only
    /// from the `fast_partial`-gated windowed-additive path.
    fn update_part_fast(&mut self, y0: u16, h: u16) -> Result<(), EspError> {
        self.set_ram_area(0, y0, WIDTH / 2, h, 0x03, 0x80)?; // slave
        self.set_ram_area(0, y0, WIDTH / 2, h, 0x03, 0x00)?; // master
        // The whole recipe goes to BOTH controllers — each half has its own
        // waveform SRAM and charge pump; a master-only write would leave the left
        // half on the OTP waveform and the two halves would ghost differently.
        // Mirrors Good Display's `Epaper_Partial` (0x32, 0x3F, 0x03, 0x04, 0x2C, 0x37).
        const LUT: usize = 227;
        for target in [0x80u8, 0x00u8] {
            self.cmd(0x32 | target)?; // LUT register (waveform phases + FR/XON)
            self.data(&FAST_PARTIAL_LUT[..LUT])?;
            self.cmd(0x3F | target)?; // EOPT — LUT end option
            self.data(&[FAST_PARTIAL_LUT[LUT]])?;
            self.cmd(0x03 | target)?; // VGH — gate driving voltage
            self.data(&[FAST_PARTIAL_LUT[LUT + 1]])?;
            self.cmd(0x04 | target)?; // VSH1, VSH2, VSL — source driving voltage
            self.data(&FAST_PARTIAL_LUT[LUT + 2..LUT + 5])?;
            self.cmd(0x2C | target)?; // VCOM
            self.data(&[FAST_PARTIAL_LUT[LUT + 5]])?;
            self.cmd(0x37 | target)?; // display option — required (its omission broke the 2026-07-19 attempt)
            self.data(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00])?;
        }
        // Border kept at the known-good 0x80 (the vendor recipe uses 0xC0).
        self.cmd(0x3C)?; // border waveform control
        self.data(&[0x80])?;
        self.cmd(0x21)?; // display update control 1
        self.data(&[0x00, 0x10])?; // RED normal, cascade
        self.cmd(0x22)?; // display update control 2
        let trigger = if FAST_PART_KEEP_HOT { FAST_PART_UPDATE_HOT } else { FAST_PART_UPDATE };
        self.data(&[trigger])?;
        self.cmd(0x20)?; // master activation
        self.wait_while_busy(2000)?;
        self.fast_lut_loaded = true; // resident until update_part evicts it (or a re-init)
        Ok(())
    }

    /// Evict a resident fast-partial recipe by reloading the factory OTP
    /// waveform, if one is loaded; a no-op otherwise. Factory partials run on
    /// the weakened custom waveform without this (doubled Ctrl-U/D scrolls,
    /// 2026-07-25). Must run BEFORE the refresh's RAM-bank writes: its master
    /// activation between the `0x24` band write and the display made the
    /// partial drive toward the *previous* frame — a backspaced char stayed on
    /// the panel and became undrivable once the resync rewrote both banks
    /// (2026-07-26).
    fn evict_fast_recipe(&mut self) -> Result<(), EspError> {
        if self.fast_lut_loaded {
            self.reload_otp_lut()?;
            self.fast_lut_loaded = false;
        }
        Ok(())
    }

    /// Reload the factory OTP waveform, replacing a resident custom recipe — the
    /// same bare load-LUT activation `init()` ends with (RAM banks untouched, so
    /// the differential "previous image" survives; ~15 ms). If scroll-doubling
    /// ever persists past this, the remaining suspect is the custom drive
    /// voltages (`0x3F`/`0x03`/`0x04`/`0x2C`), which this may not restore.
    fn reload_otp_lut(&mut self) -> Result<(), EspError> {
        self.cmd(0x22)?; // display update control 2
        self.data(&[0x91])?; // load temp, load LUT (B/W), disable clock
        self.cmd(0x20)?; // master activation
        FreeRtos::delay_ms(10);
        self.wait_while_busy(100)?;
        Ok(())
    }

    /// Fill the whole panel with one value and full-refresh.
    /// `0xFF` = white, `0x00` = black. Port of GxEPD2 `clearScreen`.
    pub fn clear_screen(&mut self, value: u8) -> Result<(), EspError> {
        self.wait_ready()?;
        self.write_buffer(0x26, value)?; // previous
        self.write_buffer(0x24, value)?; // current
        self.update_full()?;
        Ok(())
    }

    /// Blit rows `y0..y0+h` of a 792×272 framebuffer into one RAM bank on both
    /// controllers. Port of GxEPD2 `_writeFromImage`, windowed in Y: slave gets
    /// panel bytes 0..=49 of each row in X-increment mode; the master's sources
    /// are wired mirrored, so it gets bytes 49..=98 in bitmap order while the
    /// address counter walks RAM 49..=0 (mode 0x02). The seam byte 49
    /// (px 392..399) lands on both; the 4 columns past each controller's 396
    /// sources aren't wired. Pass `(0, HEIGHT)` for a full-frame blit.
    fn write_frame_bank(&mut self, command: u8, fb: &[u8], y0: u16, h: u16) -> Result<(), EspError> {
        let rows = || fb.chunks_exact(FB_BYTES_W).skip(y0 as usize).take(h as usize);

        let mut buf = Vec::with_capacity(CTRL_BYTES_W * h as usize);
        for row in rows() {
            buf.extend_from_slice(row.get(..CTRL_BYTES_W).unwrap_or_default());
        }
        self.set_ram_area(0, y0, WIDTH / 2, h, 0x03, 0x80)?; // slave
        self.cmd(command | 0x80)?;
        self.data(&buf)?;

        buf.clear();
        for row in rows() {
            buf.extend_from_slice(row.get(FB_BYTES_W - CTRL_BYTES_W..).unwrap_or_default());
        }
        self.set_ram_area(0, y0, WIDTH / 2, h, 0x02, 0x00)?; // master
        self.cmd(command)?;
        self.data(&buf)?;
        Ok(())
    }

    /// Show a full 792×272 framebuffer (`FB_BYTES` long) with a full
    /// refresh. Writes both RAM banks so the next differential update has a
    /// consistent "previous" image.
    pub fn display_frame(&mut self, fb: &[u8]) -> Result<(), EspError> {
        contract(fb.len() == FB_BYTES)?;
        self.wait_ready()?;
        // A resident fast recipe survives 0xD7's load-LUT (same finding as the
        // factory-partial eviction), so a full while it is loaded would run the
        // flash on the typing waveform.
        self.evict_fast_recipe()?;
        self.write_frame_bank(0x26, fb, 0, HEIGHT)?; // previous
        self.write_frame_bank(0x24, fb, 0, HEIGHT)?; // current
        self.update_full()?;
        Ok(())
    }

    /// [`display_frame`](Self::display_frame)'s *laundering* variant: a software
    /// power-cycle (panel reset + re-init) followed by the plain full refresh,
    /// ~1.9 s. Restores the boot state that partials overwrite — the
    /// sensor-loaded temperature band and the `0x32` waveform SRAM — which is
    /// what lets the boot full scrub fast-partial residue while a mid-session
    /// full can't. Narrative: docs/tradeoff-curves/epd-refresh-latency.md.
    pub fn display_frame_clean(&mut self, fb: &[u8]) -> Result<(), EspError> {
        contract(fb.len() == FB_BYTES)?;
        self.wait_ready()?;
        self.reset()?;
        self.init()?;
        self.write_frame_bank(0x26, fb, 0, HEIGHT)?; // previous
        self.write_frame_bank(0x24, fb, 0, HEIGHT)?; // current
        self.update_full()?;
        Ok(())
    }

    /// `display_frame` minus the wait: writes both RAM banks, starts the
    /// full-refresh waveform (~2.2 s), and returns immediately so the caller
    /// can do other work (SD mount, note load) while the panel paints itself.
    /// Every public display call waits out the pending refresh (`wait_ready`)
    /// before its own controller traffic, so nothing can collide with it.
    pub fn display_frame_async(&mut self, fb: &[u8]) -> Result<(), EspError> {
        contract(fb.len() == FB_BYTES)?;
        self.wait_ready()?;
        self.evict_fast_recipe()?; // same hazard as display_frame
        self.write_frame_bank(0x26, fb, 0, HEIGHT)?; // previous
        self.write_frame_bank(0x24, fb, 0, HEIGHT)?; // current
        self.kick_update_full()?;
        self.refresh_pending = true;
        Ok(())
    }

    /// Partial-refresh only rows `y0..y0+h` of the panel from a full
    /// framebuffer — the fast per-keystroke path (pass `(0, HEIGHT)` for the
    /// whole panel). Requires the banks to already hold the on-screen image
    /// for those rows — true after any `display_frame`, `clear_screen`, or a
    /// prior partial covering them. Writes the new rows to `0x24`, runs the
    /// partial waveform over just that band, then re-writes the band to BOTH
    /// banks (GxEPD2's `writeImageAgain`): the controller ping-pongs its RAM
    /// buffers on a Mode-2 display, so post-refresh the bank addressed as
    /// `0x24` is the stale one — syncing only `0x26` left content flapping
    /// while typing fast, see
    /// docs/postmortems/2026-07-16-partial-refresh-bank-toggle.md.
    /// `fb` is always the full frame; only the given rows are used.
    pub fn display_frame_partial_window(
        &mut self,
        fb: &[u8],
        y0: u16,
        h: u16,
    ) -> Result<(), EspError> {
        self.partial_window(fb, y0, h, false)
    }

    /// The experimental fast-waveform twin of
    /// [`display_frame_partial_window`](Self::display_frame_partial_window): same
    /// windowed sequence, but the transition runs the short custom LUT
    /// ([`update_part_fast`](Self::update_part_fast)) instead of the OTP partial.
    /// The render engine calls this only for the per-keystroke additive repaint and
    /// only when the `fast_partial` pref is on. See [`FAST_PARTIAL_LUT`].
    pub fn display_frame_partial_window_fast(
        &mut self,
        fb: &[u8],
        y0: u16,
        h: u16,
    ) -> Result<(), EspError> {
        self.partial_window(fb, y0, h, true)
    }

    /// Shared body of the two windowed-partial methods above. `fast` selects the
    /// custom-LUT waveform; everything else — the RAM-bank writes and the
    /// post-refresh resync — is identical and kept here in one place.
    fn partial_window(&mut self, fb: &[u8], y0: u16, h: u16, fast: bool) -> Result<(), EspError> {
        contract(fb.len() == FB_BYTES)?;
        contract(h > 0 && u32::from(y0) + u32::from(h) <= u32::from(HEIGHT))?;
        self.wait_ready()?;
        if !fast {
            self.evict_fast_recipe()?; // before the bank write — see its doc
        }
        self.write_frame_bank(0x24, fb, y0, h)?; // current = new
        if fast {
            self.update_part_fast(y0, h)?; // transition previous -> current
        } else {
            self.update_part(y0, h)?;
        }
        self.write_frame_bank(0x26, fb, y0, h)?; // resync both banks…
        self.write_frame_bank(0x24, fb, y0, h)?; // …post ping-pong
        Ok(())
    }
}

/// The [`hal::Screen`] port: the render engine (`app::Panel`) drives the panel
/// through this contract rather than the concrete `Epd`, so it no longer names
/// esp-idf. Both methods forward to the inherent driver methods above; the
/// associated error is esp-idf's `EspError`, kept off the layers above by the
/// trait's associated type.
impl hal::Screen for Epd<'_> {
    type Error = EspError;

    fn display_frame(&mut self, fb: &[u8]) -> Result<(), Self::Error> {
        Epd::display_frame(self, fb)
    }

    fn display_frame_partial_window(
        &mut self,
        fb: &[u8],
        y0: u16,
        h: u16,
    ) -> Result<(), Self::Error> {
        Epd::display_frame_partial_window(self, fb, y0, h)
    }

    fn display_frame_partial_window_fast(
        &mut self,
        fb: &[u8],
        y0: u16,
        h: u16,
    ) -> Result<(), Self::Error> {
        Epd::display_frame_partial_window_fast(self, fb, y0, h)
    }

    fn display_frame_clean(&mut self, fb: &[u8]) -> Result<(), Self::Error> {
        Epd::display_frame_clean(self, fb)
    }
}
