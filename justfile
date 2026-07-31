# Host-side workspace — the pure, off-device crates (see root Cargo.toml).
# Firmware recipes (build/flash/provision/…) live in firmware/justfile.

# list recipes
default:
    @just --list

# run every host crate's tests (editor, app, wizard, keymap, conf, display, hal)
test:
    cargo test --workspace

# clippy over the host workspace, all targets (libs, tests, examples).
# `unwrap_used` is warn-level workspace-wide — keep it at zero findings; the
# firmware package has its own `just -f firmware/justfile lint` (esp toolchain).
lint:
    cargo clippy --workspace --all-targets
