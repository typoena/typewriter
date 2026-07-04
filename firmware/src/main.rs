mod epd;
mod usb_kbd;

use std::time::Instant;

use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{AnyIOPin, PinDriver, Pull};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::spi::config::{Config, DriverConfig};
use esp_idf_svc::hal::spi::{Dma, SpiBusDriver, SpiDriver};
use esp_idf_svc::hal::units::FromValueType;

use epd::Epd;

/// Injected by build.rs so serial output identifies the exact build.
const BUILD_TAG: &str = concat!("build ", env!("BUILD_TIME"), " @", env!("BUILD_GIT"));

/// FONT_10X20 laid out on the 792×272 panel: 10 px wide, 20 px tall.
const CW: i32 = 10;
const CH: i32 = 20;
const COLS: usize = (epd::WIDTH / 10) as usize; // 79 characters per line
const ROWS: usize = (epd::HEIGHT / 20) as usize; // 13 lines

/// Clear accumulated partial-refresh ghosting with a full refresh this often.
const FULL_REFRESH_EVERY: u32 = 20;

fn main() -> anyhow::Result<()> {
    // Required once before any esp-idf-svc call; some runtime patches
    // only link if this symbol is referenced. See esp-idf-template#71.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Typoena Spike 5 — partial refresh + keyboard, {BUILD_TAG}");

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    // GDEY0579T93 on S3-safe GPIOs (Spike 2 wiring):
    //   SCK 12 · DIN/MOSI 11 · CS 7 · DC 6 · RST 5 · BUSY 4
    let spi = SpiDriver::new(
        peripherals.spi2,
        pins.gpio12,
        pins.gpio11,
        None::<AnyIOPin>,
        &DriverConfig::new().dma(Dma::Auto(4096)),
    )?;
    let bus = SpiBusDriver::new(spi, &Config::new().baudrate(4.MHz().into()))?;
    let cs = PinDriver::output(pins.gpio7)?;
    let dc = PinDriver::output(pins.gpio6)?;
    let rst = PinDriver::output(pins.gpio5)?;
    let busy = PinDriver::input(pins.gpio4, Pull::Down)?;
    let mut epd = Epd::new(bus, dc, rst, cs, busy);

    log::info!("EPD reset + init…");
    epd.reset()?;
    epd.init()?;
    epd.clear_screen(0xFF)?; // white baseline; establishes the previous bank

    // Bring up the USB keyboard in the background; keys arrive via next_key().
    usb_kbd::start()?;

    // First render is full (establishes the on-screen baseline for partials).
    let mut text = String::new();
    epd.display_frame(render_frame(&text).bytes())?;

    let mut updates: u32 = 0;
    loop {
        // Drain all queued keystrokes (type-ahead absorbed during a refresh),
        // apply them, then do a single refresh for the batch.
        let mut keys = 0;
        while let Some(k) = usb_kbd::next_key() {
            apply_key(&mut text, k);
            keys += 1;
        }
        if keys == 0 {
            FreeRtos::delay_ms(8);
            continue;
        }

        let frame = render_frame(&text);
        updates += 1;
        let full = updates % FULL_REFRESH_EVERY == 0;

        let t0 = Instant::now();
        if full {
            epd.display_frame(frame.bytes())?;
        } else {
            epd.display_frame_partial(frame.bytes())?;
        }
        let ms = t0.elapsed().as_millis();
        log::info!(
            "{} refresh #{updates}: {ms} ms ({keys} key(s), {} chars)",
            if full { "FULL" } else { "partial" },
            text.chars().count(),
        );
    }
}

/// Apply a key event to the text buffer.
fn apply_key(text: &mut String, key: usb_kbd::Key) {
    match key {
        usb_kbd::Key::Char(c) => text.push(c),
        usb_kbd::Key::Enter => text.push('\n'),
        usb_kbd::Key::Backspace => {
            text.pop();
        }
    }
}

/// Render the text buffer into a frame: word-wrapped at the panel width,
/// scrolled to the last `ROWS` lines, with an underline caret at the end.
fn render_frame(text: &str) -> epd::Frame {
    let mut frame = epd::Frame::new_white();
    let style = MonoTextStyle::new(&FONT_10X20, BinaryColor::On); // black ink

    // Break into display lines on '\n' and at the column limit; tabs expand
    // to 4 spaces.
    let mut lines: Vec<String> = vec![String::new()];
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(String::new());
            continue;
        }
        let (glyph, count) = if ch == '\t' { (' ', 4) } else { (ch, 1) };
        for _ in 0..count {
            if lines.last().unwrap().chars().count() >= COLS {
                lines.push(String::new());
            }
            lines.last_mut().unwrap().push(glyph);
        }
    }

    // Scroll: show only the last ROWS lines.
    let start = lines.len().saturating_sub(ROWS);
    let shown = &lines[start..];
    for (row, line) in shown.iter().enumerate() {
        Text::with_baseline(line, Point::new(0, row as i32 * CH), style, Baseline::Top)
            .draw(&mut frame)
            .unwrap();
    }

    // Underline caret at the end of the last visible line.
    if let Some(last) = shown.last() {
        let col = last.chars().count().min(COLS - 1) as i32;
        let row = shown.len() as i32 - 1;
        Rectangle::new(
            Point::new(col * CW, row * CH + CH - 2),
            Size::new(CW as u32, 2),
        )
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(&mut frame)
        .unwrap();
    }

    frame
}
