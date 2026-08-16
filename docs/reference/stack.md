# Software stack

**Language: Rust on `esp-idf-rs` (std).** Every stack decision — language, UI
strategy, display, git lib, auth, concurrency, storage, power, keyboard
transport — has an ADR in [`adr.md`](../adr.md), including the rejected
alternatives (Ratatui, Gleam + Shore on AtomVM, C/Arduino — ADR-001/002). How
each decision is weighted against the user-facing requirements lives in
[`qfd.md`](../quality/qfd.md); the ontology those docs use is defined in
[`quality/glossary.md`](../quality/glossary.md). Where a default traces to a cost curve rather
than a discrete pick — energy, latency, or memory bending against an interval
or size — the curve and its knee live in
[`tradeoff-curves/README.md`](../record/tradeoff-curves/README.md). The whole `unsafe`
surface is thin FFI into ESP-IDF and libgit2, concentrated in
`firmware/src/drivers/` and `firmware/src/infrastructure/`; every block carries
its own `SAFETY:` justification.

| Layer         | Choice                                                                                              | Notes                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HAL / runtime | `esp-idf-svc`, `esp-idf-hal`                                                                        | std build: heap, threads, VFS, mbedtls, Wi-Fi stack.                                                                                                                                                                                                                                                                                                                                                                                          |
| Display       | Custom SSD1683 driver (`firmware/src/drivers/screen_epd.rs`) + `embedded-graphics`                  | Dual-controller 792×272 panel; dirty-rect partial refresh (~630 ms measured). Panel geometry + the in-memory `Frame` live in the shared `display/` crate.                                                                                                                                                                                                                                                                                     |
| UI layer      | Custom thin widget layer                                                                            | Ratatui's API _shape_ without its char-grid terminal model ([ADR-002](../adr.md#adr-002-ui-strategy--custom-widgets-on-embedded-graphics-not-ratatui)).                                                                                                                                                                                                                                                                                          |
| Editor core   | Custom, in-tree (`editor/` crate)                                                                   | Modal (Normal / Insert / Visual / VisualLine / View / Command / Palette), motions, operators + text objects; UTF-8 buffer fed by the dead-key composer; smartcase + accent-folded `/` search. Host-built and host-tested off the xtensa target.                                                                                                                                                                                               |
| USB host      | `esp-idf` TinyUSB bindings                                                                          | Boot-protocol HID; verified on hardware (Spike 4).                                                                                                                                                                                                                                                                                                                                                                                            |
| Git           | **libgit2 via `git2`**, built as an esp-idf component with mbedTLS (`firmware/components/libgit2/`) | `gix` was the original pick but can't push over HTTPS — the [ADR-004](../adr.md#adr-004-git-implementation--gitoxide-gix) kill-switch fired ([postmortem](../record/postmortems/2026-07-05-spike7-gix-https-push.md)). `:gs` push + `:gl` pull verified on device; ~16 s cold-`:gs` [latency breakdown](../record/notes/sync-latency.md); how the real notes repo went from a 611 s brick to a 24 s sync: [kaizen](../record/kaizen/real-repo-sync.md).                     |
| TLS           | `mbedtls` via `esp-idf`                                                                             | GitHub HTTPS with the chain checked against embedded roots; ≈35 KB heap measured during handshake (Spike 6).                                                                                                                                                                                                                                                                                                                                  |
| Auth          | HTTPS + GitHub PAT                                                                                  | Provisioned to `/sd/typoena.conf` by the host installer or the on-device wizard; at-rest protection is [ADR-011](../adr.md#adr-011-credential-provisioning--how-the-pat-reaches-the-device-and-is-protected-at-rest).                                                                                                                                                                                                                            |
| Filesystem    | FAT on SD (`esp_vfs_fat`)                                                                           | Working copy lives here (`/sd/repo` + `/sd/local`); editor prefs are a git-tracked [`.typoena.toml`](typoena-toml.md) in the repo.                                                                                                                                                                                                                                                                                                            |

## Repo layout

```
/firmware       the device crate (esp-idf target) — main.rs composes editor +
                display + drivers + infrastructure; on-device spike/bench bins
                in src/bin
  /src/drivers          esp-idf implementations of the hal ports: EPD
                        (screen_epd.rs), USB keyboard, Wi-Fi, clock, system
  /src/infrastructure   SD storage (atomic save + crash recovery), net, OTA,
                        file index, wizard I/O driver
  /components/libgit2   libgit2 as an esp-idf CMake component (mbedTLS);
                        source vendored as a git submodule
  build.rs              build stamp (UTC + git describe), baked env defaults
/app            application runtime — render loop, ports, run loop; host-built
                and host-tested (`cargo test -p app`)
/editor         modal editor core — buffer, modes, motions, palette, search,
                render; host-tested
/display        panel geometry + in-memory Frame (embedded-graphics
                DrawTarget), shared by the EPD driver and the editor
/hal            hardware abstraction layer — pure trait "ports" for the
                devices the run loop drives; no esp-idf types
/conf           `typoena.conf` schema — the single source of truth for the
                seven TW_* values (parse, render, remote-URL shorthand)
/keymap         pure HID boot-keyboard decode — host-testable, fuzzable
/wizard         on-device onboarding wizard — pure step/field state machine,
                host-testable; firmware executes its Effects
/installer      macOS setup tool (ratatui) — flash + provision a card without
                a dev environment
/spikes         desktop spikes (spike7 git push proof, pre-device)
/hardware       the physical build — bom.md, wiring.md, the parametric
                OpenSCAD case (case/) + renders, and the KiCad PCBs (pcb/)
/docs           the design record — index: README.md
  adr.md          the load-bearing decisions
  /reference      true now: commands, stack, prefs, snippets, testing
  /plan           macroplan + the specs for releases not yet delivered
  /quality        the QFD cascade + its methodology glossary
  /record         dated and append-only: postmortems, kaizen,
                  tradeoff-curves, notes
CONTEXT.md      project glossary — Tracked / Local / Save / Push, and the
                principles that fall out of them
```
