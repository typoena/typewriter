# Editor setup — Zed / rust-analyzer

The repo-level `.zed/settings.json` configures `rust-analyzer` for the firmware
crate:

- `cargo.target` is pinned to `xtensa-esp32s3-espidf` with
  `allTargets = false`, so RA doesn't try to also check the crate for the
  host target (which can't build `esp-idf-sys`).
- `binary.path` is pinned to the **rustup-managed** rust-analyzer
  (`stable` toolchain), not Zed's bundled one, so everyone runs the same
  RA. (It was originally a workaround for the lockfile issue below, but
  that turned out to depend on the `esp` toolchain version, not the RA
  binary.)

## The `esp` toolchain must be ≥ 1.96

Currently 1.97.0.0. On esp 1.95.0.0 rust-analyzer fails to load the workspace
with "Failed to read Cargo metadata … unexpected argument `--lockfile-path`".
cargo removed that flag in 1.95 in favor of the `CARGO_RESOLVER_LOCKFILE_PATH`
env var, and rust-analyzer picks flag-vs-env-var by semver-comparing the
workspace toolchain to `1.95.0` — `1.95.0-nightly` (what esp 1.95.0.0 reports)
sorts _below_ it, so RA passes the removed flag
([rust-analyzer#21761][ra-21761]). Note `espup update` can silently keep a
stale default version; pin it explicitly:

```sh
espup install --toolchain-version 1.97.0.0
```

[ra-21761]: https://github.com/rust-lang/rust-analyzer/issues/21761

If a contributor on a different machine has issues, regenerate the path:

```sh
rustup component add rust-analyzer --toolchain stable
rustup which rust-analyzer --toolchain stable
# put the printed path into .zed/settings.json under lsp.rust-analyzer.binary.path
```

## Environment Zed must be launched in

Two things rust-analyzer needs from the **environment Zed was launched in**:

- `LIBCLANG_PATH` — required by `bindgen` inside `esp-idf-sys`.
- The Xtensa GCC on `PATH` — required by `embuild` during `cargo check`.

Both are set by `~/export-esp.sh`. The pragmatic workflow:

```sh
. ~/export-esp.sh
zed /Users/julien/jclab/typewriter   # or: open from this shell
```

If Zed is launched from Finder/Dock instead, rust-analyzer will report
`bindgen` errors on the first `esp-idf-sys` check. Close Zed, source the
env in a terminal, and relaunch from there.

## Toolchain pins

[`rust-toolchain.toml`](../rust-toolchain.toml) pins the channel to `esp`
(installed by `espup install`). Cargo.toml currently includes git
`[patch.crates-io]` overrides for `esp-idf-sys` / `esp-idf-hal` / `esp-idf-svc`
(template default). These follow master and may need pinning to released
versions if a master commit breaks the build.
