# Typoena firmware

Rust crate targeting `xtensa-esp32s3-espidf` — the on-device firmware: a
vim-style modal editor with git push/pull, OTA updates, and a first-boot
wizard.

One crate of the [Typoena project](../README.md) — start there for the
vision, the hardware, and the design record (macroplan, QFD, ADRs, doc
index). Firmware-specific design:
[`v0.1-mvp-technical.md`](../docs/v0.1-mvp-technical.md) — module split,
threads, and the hardware bring-up order.

Technical pages:

- [Build details](docs/build-details.md) — esp-idf components, libgit2 and
  the `full` feature
- [Editor setup](docs/editor-setup.md) — Zed / rust-analyzer, `esp` toolchain
  version requirement, launch environment
- [Provisioning an SD card](docs/sd-provisioning.md) — `just init` / `load` /
  `provision`, the config ladder, secrets on the card
- [Bench board & pinout](docs/board.md) — DevKitC-1 v1.0, pin assignments
- [Bring-up spike log](docs/bring-up-spikes.md) — Spikes 1–6 as verified on
  the bench
- [Bench QC firmware](docs/bench-qc.md) — go/no-go fixture for the
  hand-soldered carrier PCB

## Quick commands

A [`justfile`](https://github.com/casey/just) wraps the common commands and
sources the espup env itself — run `just` in this directory for the list
(`build`, `flash`, `monitor`, `info`, `ports`).

## Build

Once per shell session, source the espup env (sets `LIBCLANG_PATH` and adds
the Xtensa GCC to `PATH`):

```sh
. ~/export-esp.sh
```

Then from this directory:

```sh
just build   # the product firmware: editor + git pushing + OTA + wizard
```

The first build is slow (esp-idf + libgit2 + mbedTLS); after that it's
incremental — see [build details](docs/build-details.md). The fast iteration
loop is host-side: `cargo test -p app -p editor`.

The panel's fast/partial waveform comes from Good Display's vendor reference
driver, kept verbatim in
[`reference/gdey0579t93-fp-lut/`](reference/gdey0579t93-fp-lut/README.md) — it is
the source of truth for the custom `0x32` LUT, which cannot be derived from the
panel's OTP.

## Flash

With the board connected over USB, `just flash` (or `cargo run --release`)
triggers `espflash flash --monitor` via the runner configured in
[`.cargo/config.toml`](.cargo/config.toml).
