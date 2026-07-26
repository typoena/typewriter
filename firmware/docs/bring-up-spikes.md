# Hardware bring-up — spike log

The chronological record of the bench spikes that brought the firmware up on
real silicon, kept as-verified (dates and measurements are from the bench
sessions). For what the crate is and how to build it, start at the
[firmware README](../README.md).

## Modal editor (vim modes) — verified 2026-07-05

The firmware became a small vim-style modal text editor. The
[`editor` crate](../../editor/src/lib.rs) owns the buffer, caret, motions, and
per-mode rendering; [`src/main.rs`](../src/main.rs) is the hardware loop that
drains keystrokes, redraws, and picks a refresh strategy;
[`src/drivers/keyboard_usb.rs`](../src/drivers/keyboard_usb.rs) decodes editing
chords and a dual-role Caps key. The buffer is pure ASCII, so a byte offset
doubles as the caret's character index (Tab expands to spaces on insert).

Modes (shown live in a small status strip below the text):

- **Insert** — the boot mode; keys type at the caret. `Ctrl+W` /
  `Ctrl+Backspace` delete the previous word, `Cmd+Backspace` deletes to the
  start of the line.
- **Normal** — motions `h j k l`, `w b e`, `0` `$`, `gg` `G`; edits `x`, `dd`,
  and the `d` / `c` (change) operators over motions and text objects — `ciw`,
  `daw`, `di(`, `ci"`, … (bracket pairs are nesting-aware); `i a A I o O` to
  enter insert; count prefixes like `3j`, `2dd`.
- **View** — read-only reading: `j` / `k` scroll, `space` pages, `gg` / `G`
  jump; edits are locked out.

**Caps Lock is dual-role**: tapped it is `Esc` (→ Normal); held it is `Ctrl`.
So Caps no longer types capitals — use Shift.

Rendering reuses the partial refresh from Spike 5: additive Insert typing stays
on the fast windowed path with a ~750 ms debounced caret, while caret moves,
deletes, mode switches, and View scrolling take a clean full-area partial
(~630 ms). Count prefixes collapse repeated motion into a single refresh, which
matters at this latency.

Known rough edges (deferred): no backspace auto-repeat (the keyboard is on
`SET_IDLE(0)` and only key-downs are tracked), non-sticky column on `j` / `k`,
the `$` / end-of-line block caret sits one cell past the last char, `iw` / `aw`
are whitespace-delimited (like vim's `iW` / `aW`), and `cw` isn't special-cased
to `ce`.

## Spike 6 — Wi-Fi + TLS: verified 2026-07-05

A separate binary — [`src/bin/wifi_tls.rs`](../src/bin/wifi_tls.rs), flashed
with `just flash-wifi` — kept apart from the editor firmware. It brings up the
station, syncs the clock over SNTP (mbedtls validates the server cert against
wall time, so the 1970 RTC has to be corrected first), then does an HTTPS GET
to `https://api.github.com/` with cert-chain validation against the esp-idf
certificate bundle (`esp_crt_bundle_attach`), and logs status, a body preview,
and free heap around the handshake (TLS heap pressure is a watched risk). A
validated GET was the gate for Spike 7 (git push over HTTPS + PAT).

Bench result (WPA2-PSK AP, 2.4 GHz): associate ~3 s → DHCP → SNTP first sync →
`esp-x509-crt-bundle: Certificate validated` → `HTTPS GET … → 200`, reading real
GitHub JSON. **TLS handshake cost ≈ 35 KB heap** (265 → 229 KB, recovered
after), clean and repeatable across reboots. Note: PSRAM was **not** enabled yet
(only ~339 KB internal heap) — TLS fits, but Spike 7's git working set needed
`CONFIG_SPIRAM` turned on first.

Credentials are build-time: copy [`.env.example`](../.env.example) to `.env`,
set `TW_WIFI_SSID` / `TW_WIFI_PASS`, and `just` loads them (dotenv) so
`build.rs` bakes them in. `.env` is gitignored; the SD bench (`sd_bench`) needs
none of it. `sdkconfig.defaults` gained the full certificate bundle and a bigger
main task stack for the mbedtls handshake.

## Spike 3 — SD card (FAT) on dedicated SPI3: verified 2026-07-11

A separate binary — [`src/bin/sd_bench.rs`](../src/bin/sd_bench.rs), flashed
with `just flash-bench` — is a thin on-device harness over the real SD storage
adapter ([`src/infrastructure/storage_sd.rs`](../src/infrastructure/storage_sd.rs),
`app::Storage`): it mounts the card, reports FAT usage, and round-trips an
atomic save/load (write `*.tmp` → fsync → unlink → rename → read-back). Per
ADR-012 the SD runs on its **own SPI3 host** —
**SCK 14 · MOSI 15 · MISO 13 · SD CS 10** — leaving the EPD alone on SPI2.
Verified on the dedicated SPI3 bus 2026-07-11 (same mount + round-trip result as
the initial shared-SPI2 bring-up).

Bench result (genuine 32 GB SDHC card): mounts at 10 MHz, `29806 MiB total`,
atomic round-trip byte-identical. Two findings baked into the code:

- **Card compatibility.** A 133 GB SDXC card failed init at `CMD59` (SPI-mode
  CRC); a genuine ≤32 GB card works. We keep CRC required and reject bad cards
  with a swap-the-card message rather than run over an unchecked bus. See the
  [Spike 3 postmortem](../../docs/postmortems/2026-07-05-spike3-sd-cmd59.md).
- **FatFS rename ≠ POSIX rename.** `f_rename` won't overwrite an existing
  target (returns `FR_EXIST`), so the atomic save unlinks the destination first.
  `storage_sd` pairs this with `*.tmp` boot-recovery (`recover` at mount): if a
  `*.tmp` is found _alongside_ the target the crash may have been mid-write, so
  it keeps the committed file and discards the tmp; it only promotes the tmp
  when the target was already unlinked. Long filenames
  (`CONFIG_FATFS_LFN_HEAP`) are required for the two-dot `*.md.tmp` name.

**Arbitration resolved (ADR-012):** the EPD driver holds an exclusive SPI2 lock
for its whole lifetime, and storage runs on its own thread, so a shared bus
would need an EPD rewrite plus a cross-thread mutex on the save path. Instead
the SD gets its own SPI3 — the EPD stays untouched, no arbitration. The atomic
save (unlink-then-rename + `*.tmp` boot-recovery) has since landed as the
`storage_sd` adapter behind `app::Storage`.

## Spike 5 — partial refresh + typing: verified 2026-07-04

`main.rs` wires the keyboard to the panel:
[`src/drivers/keyboard_usb.rs`](../src/drivers/keyboard_usb.rs) feeds decoded
key-downs (US layout, edge-detected) into a queue, and the main loop keeps a
wrapped, scrolling text buffer that it draws with a **partial refresh**
(`Epd::display_frame_partial`) per keystroke batch, plus a periodic full
refresh to clear ghosting. First spike where input and output run together.
Measured on the bench at 4 MHz SPI: partial refresh ~630 ms, full ~1870 ms —
the partial waveform (~490 ms, all 272 rows) dominates. Follow-up: windowed-Y
partial refresh (drive only the edited line's rows) to cut per-keystroke
latency.

## Spike 4 — USB host keyboard: verified 2026-07-04

[`src/drivers/keyboard_usb.rs`](../src/drivers/keyboard_usb.rs) drives the
ESP-IDF USB Host Library directly through the raw `esp-idf-sys` bindings (no
managed HID class driver), enumerates an attached keyboard, claims the
boot-keyboard interface, switches it to boot protocol, and polls the
interrupt-IN endpoint — decoding each 8-byte report into modifiers + keycodes.
Verified with a `19f5:3255` keyboard: keystrokes, modifiers, and rollover all
decode correctly.

Hardware: flash + serial over the CP2102 "UART" port (console = UART0,
independent of the USB PHY), keyboard on the native "USB" port. The keyboard
enumerated **bus-powered** — no external VBUS injection needed on this
DevKitC-1 v1.0 (keep a 5 V power cable only as a brownout fallback for
higher-power/RGB devices).

## Spike 2 — EPD: verified 2026-07-04

The GDEY0579T93 e-paper panel is driven through the thin dual-SSD1683 driver in
[`src/drivers/screen_epd.rs`](../src/drivers/screen_epd.rs) (ported from
GxEPD2's `GxEPD2_579_GDEY0579T93`). Verified on the bench rig over 4 MHz SPI:

- **2a — uniform fill:** clean full-panel white ↔ black refreshes, proving
  the wiring, both cascaded controllers, RAM addressing, and the full
  refresh waveform.
- **2b — graphics/text:** `epd::Frame` implements `embedded-graphics`'
  `DrawTarget`; a stroked circle straddling the master/slave seam (x = 396)
  renders round and continuous, and `FONT_10X20` text is legible — proving
  the split-and-mirror full-frame blit (`Epd::display_frame`).

Wiring: SCK 12 · DIN/MOSI 11 · CS 7 · DC 6 · RST 5 · BUSY 4, via the
DESPI-C579 breakout.

Every build is stamped by [`build.rs`](../build.rs) with UTC time and
`git describe --always --dirty`; the tag is logged on serial at boot and
drawn on the panel, so the running build is always identifiable during
diagnosis.

Bring-up note: the initial symptom was per-pixel noise on the panel — a
half-seated CS jumper, not firmware. If the panel shows speckle/banding,
reseat the jumpers (CS first) before debugging code.

## Spike 1 — Blink: verified 2026-07-04

GPIO 2 + on-board WS2812 toggled at 1 Hz with `blink N` on USB-serial, proving
toolchain, esp-idf link, and GPIO on real silicon. The blink code was replaced
by Spike 2 in `main.rs` (see git history: `e040a8d`).

**Pin choice:** GPIO 2 is a safe general-purpose pin on the ESP32-S3-DevKitC-1:
it's not tied to a strapping function at boot and not muxed to the USB or PSRAM
peripherals. The blink loop also drives the on-board addressable LED — WS2812
on GPIO 48 (GPIO 38 on DevKitC-1 v1.1 boards) — via the RMT peripheral, so both
a plain GPIO and the RMT path are exercised.

## Order

The bring-up order followed
[`docs/v0.1-mvp-technical.md`](../../docs/v0.1-mvp-technical.md#hardware-bring-up-order):
Wi-Fi/TLS (Spike 6), then git push (Spike 7), then SD (Spike 3) — all verified.
