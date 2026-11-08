# Host-side workspace — the pure, off-device crates (see root Cargo.toml).
# Firmware recipes (build/flash/provision/…) come from firmware/justfile as the
# `fw` module: `just fw::ship`, `just fw::publish-firmware`.

mod fw 'firmware/justfile'

# list recipes
default:
    @just --list --list-submodules

# run every host crate's tests (editor, app, wizard, keymap, conf, display, hal)
test:
    cargo test --workspace

# clippy over the host workspace, all targets (libs, tests, examples).
# `unwrap_used` is warn-level workspace-wide — keep it at zero findings; the
# firmware package has its own `just fw::lint` (esp toolchain).
lint:
    cargo clippy --workspace --all-targets
