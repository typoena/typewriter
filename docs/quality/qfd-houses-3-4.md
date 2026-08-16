# Houses 3 & 4 — processes × controls (pipeline reading)

Third and fourth houses of the QFD cascade (hub: [`qfd.md`](qfd.md)),
drawn under a deliberate reinterpretation: a solo-built device has no
factory, so "process" means the toolchain + release pipeline (P1–P9,
firmware build through GitHub-App administration) and "production
controls" means the verification practices (Q1–Q8, host tests through
the end-to-end install-chain check). The literal manufacturing reading
would be scaffolding; the pipeline reading is where this project's real
production risk lives. Row importance carries down from
[House 2](qfd-house-2.md)'s derived component Σ.

## House 3 — components × processes (pipeline reading)

Components (rows, importance = the derived House-2 Σ) × the processes
that produce them. No factory: "process" is the toolchain + release
pipeline P1–P9. **P1 firmware build carries 52.4 % of the process weight;
P4 bench assembly is #2 (21.4 %) with only manual controls**; the
CS-jumper and SDXC lessons were both paid there. Catalogue + first-cut
caveat: [the narrative below](#houses-34--the-cascade-to-process-and-controls).

![House 3 — components x processes P1-P9 (pipeline reading)](diagrams/house-3.tikz)

## House 4 — processes × controls

Processes (rows, importance = House-3 rel-%) × the verification
practices that guard them, Q1–Q8. **Q2 on-device verification #1, Q3
build gates #2**: the hardware-verify-everything habit is where the
arithmetic says control effort belongs. Q6's checksum chain ranks #8 by
breadth while being the *sole* control on the public install path.
Reading: [the narrative below](#houses-34--the-cascade-to-process-and-controls).

![House 4 — processes x controls Q1-Q8](diagrams/house-4.tikz)

---

## Houses 3–4 — the cascade to process and controls

Classical QFD carries the cascade two houses further: components deploy
into the **process** that produces them (House 3), and the process deploys
into the **controls** that keep it honest (House 4). This project has no
factory, but it does have a production system: the toolchain and release
pipeline (P1–P9) and the verification practices that guard it (Q1–Q8).
Both houses (drawn at the top of this page) are scored under that reading. **First cut, scored
2026-07-16**: the P/Q catalogues and cells are asserted from the
documented pipeline (justfile, installer DESIGN, release chain, the
hardware-verification record), single-rater, not measured: re-score when
the pipeline changes shape.

Row importance carries down the cascade as in [House 2](qfd-house-2.md): House 3 rows carry
each component's derived Σ (C11/C15 parenthesised and excluded, as
everywhere), House 4 rows carry each process's House-3 relative weight.
Basements show relative weight + rank (raw Σ grows geometrically down the
cascade and stops being readable).

**The processes**: P1 firmware build (`just build`: cargo for xtensa +
ESP-IDF), P2 the libgit2 esp-idf CMake component build (vendored deltas;
needs its own fingerprint handling), P3 flash at manufacturing (devices
ship pre-flashed, [installer DESIGN](../../installer/DESIGN.md)), P4 bench
hardware assembly (panel, SPI3 SD, PSU, case), P5/P6 the two peer card
provisioning paths (wizard / installer), P7 the installer release cut
(`installer-v*` tag → universal binary + `.sha256`), P8 site deploy
(Coolify auto-deploy), P9 GitHub App + org administration (client_id,
scopes, token-expiry policy, the one "process" no repo builds).

The scored house is drawn as **House 3** at the top of this page.

**The controls**: Q1 host test suites (editor 237 / keymap 29 / wizard 39),
Q2 on-device verification runs (the hardware-verified stamps throughout
this file), Q3 build gates (`just build` / `build-light`), Q4 bench
instrumentation + telemetry (`sd_bench`, refresh log, `log_push_heap`,
boot timestamps), Q5 card safety guards (ambiguity refusal, dirty-guard,
`dot_clean`, token-never-derived), Q6 the checksum + quarantine chain on
the public install path, Q7 acceptance tests (1 h soak, cold-boot clock,
the owed power-pull), Q8 the end-to-end install-chain check (mirror →
release → typoena.dev, device-flow e2e).

The scored house is drawn as **House 4** at the top of this page.

**Reading the pair.** P1 carries **52.4 %** of the process weight: the
firmware build produces almost every high-Σ component, which is why Q2/Q3
(on-device verification + build gates) rank #1/#2 among controls: the
project's habit of hardware-verifying every slice is exactly where the
arithmetic says the control effort belongs. Two flags worth keeping:
**P4 bench assembly is #2 (21.4 %) with only manual controls**: nothing
automated guards the wiring that C5/C10/C16 depend on (the CS-jumper and
SDXC lessons were both paid here), so hardware changes deserve the same
verify-on-device discipline as code; and **Q6's rank #8 understates it**:
the checksum chain is the *only* control on the public install path, so
its breadth-based rank reads low exactly the way H8's once did in House 1
(narrow voter base, absolute stakes for its one voter).


[ADR-001]: adr.md#adr-001-language-and-runtime--rust-on-esp-idf-rs-std
[ADR-002]: adr.md#adr-002-ui-strategy--custom-widgets-on-embedded-graphics-not-ratatui
[ADR-003]: adr.md#adr-003-display-medium--e-ink-gdey0579t93-panel
[ADR-004]: adr.md#adr-004-git-implementation--gitoxide-gix
[ADR-005]: adr.md#adr-005-auth--https--github-personal-access-token
[ADR-006]: adr.md#adr-006-concurrency--stdthread--channels-no-async-runtime
[ADR-007]: adr.md#adr-007-storage-split--fat-on-sd-for-working-copy-littlefs-on-flash-for-config
[ADR-008]: adr.md#adr-008-mvp-power--wall-powered-battery-deferred-to-v08
[ADR-009]: adr.md#adr-009-keyboard-transport--usb-host-tinyusb
[ADR-010]: adr.md#adr-010-push-ux--atomic-ctrl-g-auto-timestamp-commit-message-no-user-prompt
[ADR-011]: adr.md#adr-011-credential-provisioning--how-the-pat-reaches-the-device-and-is-protected-at-rest
[ADR-012]: adr.md#adr-012-sd-on-its-own-spi3-host-not-shared-with-the-epd-on-spi2
