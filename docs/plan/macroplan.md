# Macroplan — version details

Frequent releases. Each version is a usable artifact, not a checkpoint.
This file holds the `macroplan` source block and the scope checklists of the
releases still open. The user-facing requirements and engineering targets each
release feeds into are tracked in [`qfd.md`](../quality/qfd.md).

## Macro-plan

Macroplan source — paste into the macroplan app to render the week-by-week
view. `original` dates are the June 2026 baseline and never move; slips get
appended as `reestimates`, per-item actuals live in the Status block below.

```macroplan
title = "Typoena — macro plan"

[[feature]]
name = "v0.1 it writes, it pushes"
start = 2026-06-01
original = 2026-06-29
delivered = 2026-07-11
learning = "Shipped 12 days late. The long pole was hardware bring-up risk, not the editor: SD on a shared SPI bus (resolved by moving it to its own SPI3, ADR-012) and on-device git (gix killed, pivoted to libgit2 as an esp-idf CMake component, ADR-004). Splash landed as a vector wordmark, not the planned 1-bit bitmap — the asset-embed/blit path is deferred to v1.0."

[[feature]]
name = "v0.2 navigation"
start = 2026-06-29
original = 2026-07-20
delivered = 2026-07-11
learning = "Delivered 9 days early. Motions/modes, Ctrl-d/u, the UTF-8 buffer, and the absolute line-number gutter all landed 2026-07-11; the last gate, Spike 13's on-panel gutter refresh check, confirmed a single-line edit repaints only rows at/below it with no extra full refresh. Relative line numbering was dropped as an e-ink ghosting cost with no proportionate gain."

[[feature]]
name = "v0.2.5 international input"
start = 2026-07-20
original = 2026-08-03
delivered = 2026-07-11
learning = "Delivered 23 days early — ahead of its own start window. Dead-key accent composer in the keymap crate (US-International, à é ê ë ñ ç), editor buffer made UTF-8-correct, typed on the bench with no panic. The side-panel pending-accent marker was dropped by decision: at typing speed it is stale before the ~630 ms panel repaint, so it conveyed nothing. Bonus: physical Esc (HID 0x29) remapped to backtick/tilde so code fences + grave/tilde accents work on a 60% board without a Fn layer."

[[feature]]
name = "v0.3 editing"
start = 2026-08-03
original = 2026-08-24
delivered = 2026-07-11
learning = "Core complete 44 days early, host-tested and partially smoke-tested on the panel. Register + yank/paste (yy/p/P), snapshot undo/redo (u/Ctrl-r, bounded 100 groups in PSRAM), and keystroke-recorded `.` repeat all landed 2026-07-11; the d/c operator grammar + text objects were already done ahead of schedule. Firmware bumped to 0.3.0. On device dd/yy/Ctrl-r confirmed; the one bug found was a multi-line paste leaving its later lines below the fold (adjust_scroll only tracked the caret) — fixed with a reveal() that scrolls the block end into view."

[[feature]]
name = "v0.4 visual + ex"
start = 2026-08-24
original = 2026-09-07
delivered = 2026-07-11
learning = "Core complete 58 days early, host-tested. Visual (v) and VisualLine (V) selection with y/d/c landed 2026-07-11 (charwise vim-inclusive of the char under the caret; linewise spans whole lines and pastes like yy/dd), plus the recorded v/V→Visual reassignment: the read-only View mode moved to `gr` (go-read). Selection is drawn as reverse-video cells on the 1-bit panel with the caret punched back to normal video so the active end stands out; 18 new editor tests (83 total). The `:` command mechanism and :fmt were already done; `:e <path>` was deliberately deferred to v0.5 where its multi-file/buffer-lifecycle machinery (Spikes 11/14) lives, rather than half-building file-open here. Firmware bumped to 0.4.0. On-device smoke-test of Visual still pending (pure editor-core, low risk)."

[[feature]]
name = "v0.5 palette + multi-file"
start = 2026-09-07
original = 2026-09-28
delivered = 2026-07-12
learning = "Delivered 2026-07-12, well ahead of the 2026-09-28 baseline, and fully on-device confirmed. Four slices: the drained Effect queue + parked-buffer LRU foundation; the Cmd-P fuzzy file palette (Spike 11 — no ghosting on the transient panel); :enew + file delete (Spike 14 caught that add_all alone doesn't stage a deletion on this libgit2 — fixed with update_all, i.e. git add -A); and the git-tracked .typoena.toml prefs with a stay-open palette `>` command mode + :settings. Both directions of the prefs loop are proven on hardware — boot-read (byte-exact parse) and on-device palette edit (a device push flipped line_numbers on origin). Three decide-before-build calls: the idle auto-save is unformatted, and both the per-device auto_sync override and the `> auto sync` command are deferred to v0.7 where auto_sync gains behaviour. Amended 2026-07-12: a light/dark `theme` key and a set-ahead `> auto sync` preset command (2m/5m/10m/15m/30m) were added on top — the palette generalised so Enter rotates any pref to its next value (a bool is the two-option case); auto_sync is still read by nothing until v0.7. Descoped from v0.5 (not the four slices): explicit buffer close, the grey-Push-in-Local panel cue, and the multi-file push count."

[[feature]]
name = "v0.6 markdown"
start = 2026-09-28
original = 2026-10-12
delivered = 2026-07-12
learning = "Core complete 2026-07-12, ~92 days ahead of the 2026-10-12 baseline, host-tested (187 editor tests). The snippet feature was reshaped 2026-07-08→07-12 from a hard-coded table into a git-synced, Zed-compatible .typoena.snippets.json library: a forward-only tab-stop session ($1..$n/$0, ${n:label} stripped to $n) driven by two surfaces — inline Tab-expansion in Insert and a $ palette launcher — plus a quiet pause hint in the side panel. The Cmd-P palette generalised into a verb split: bare = files, > = a real command registry (toggles stay open, one-shots format/push close, the parameterised `new file` two-step), $ = snippets — retiring :e. Firmware bumped 0.5.0→0.6.0; the boot-read of the library was confirmed to build for xtensa (serde_json, the one new dep — cargo check passes). `just init` now seeds a curated 17-snippet catalog (three opt-in groups). On-device smoke-test still pending (pure editor-core + a mirror of the proven prefs boot-read, low risk). Known caveat: two symbols the catalog inserts (arrow →, neq ≠) are outside ISO-8859-15, so they store/sync correctly but need a display-layer glyph overlay (in flight) to draw on the panel; the other 15 render on the stock font."

[[feature]]
name = "v0.7 search + git"
start = 2026-10-12
original = 2026-11-02
delivered = 2026-07-14
learning = "Delivered 2026-07-14, ~16 weeks ahead of the 2026-11-02 baseline, and closed on-device across three bench runs in three days. `/` search shipped smartcase + accent-folded (a user decision that superseded the same-day plain-insensitive version; /ete finds été) with n/N, Enter-only jump, and an editor-global pattern. `:gl` pull landed fetch + fast-forward-only in all four shapes; the fast-forward is an O(changed) tree-diff apply (apply_tree_diff) built after run 2 crashed in libgit2's O(tree) checkout_tree — internal-DRAM exhaustion plus an esp-idf spi_master NULL-deref on its own failed-alloc path. Three memory/transport fixes rode along: file-list interning to one PSRAM blob (was 182 KB internal), a 64 KB DMA reserve, and TLS session resumption (third vendor delta), which cut the rejected-push reconcile cycle from 59 s to 24 s. Bonus: the first on-device rejected-push → reconcile → replay → push success, and the sd_bench dir-scaling run convicted FAT linear directory scans as the ~400 ms/loose-write residual (bounded, accepted). :sync was renamed :gp to pair with :gl."

[[feature]]
name = "v0.7.5 focus mode"
start = 2026-07-17
original = 2026-07-17
delivered = 2026-07-17
learning = "Delivered same-day — an unplanned insert after v0.7, specced/built/host-tested/on-device-verified in one session (firmware 0.7.5, 5 focus + 245 editor + 29 keymap tests). Silent 25-min block on a monotonic clock with no live countdown (e-ink can't show one cheaply); the rest card drops at the next typing pause, or a +2 min grace cap — proven on device when a continuous-typing block force-broke at 27 s (25 + 2). Resume/quit moved from a bare c / q+Esc to the Ctrl-C / Ctrl-Q chords after a bench run judged a single key too easy to fumble behind the full-screen curtain; the host also drops the rest of the key batch on exit so a bump can't reach the buffer. :focusdebug (25-second clock) made the same-day on-device check practical."

[[feature]]
name = "v0.7.7 OTA firmware update"
start = 2026-07-19
original = 2026-07-19
delivered = 2026-07-19
learning = "Delivered 2026-07-19 — an unplanned insert that RESOLVES the v1.x 'firmware auto-update' open question (raised 2026-07-14) well ahead of its pre-v1.0 deadline. `:update` GETs typoena.dev/firmware/latest.txt and, if newer, streams typoena-<ver>.bin into the inactive slot of an A/B layout (partitions-ota.csv: factory + ota_0 + ota_1 + otadata) via esp-idf OTA, then reboots into it. Proven on hardware across two back-to-back installs (0.7.7→0.7.8→0.7.9, exercising both slot directions). The load-bearing risk was device-side TLS, settled by git.apoena.dev's LE→ISRG Root X1 being in the esp-idf FULL CA bundle (validated on-device, twice). Release hosting was SPLIT after weighing one-platform: the installer stays on GitHub (its /releases/latest/download shortcut, no token), firmware releases live on Gitea git.apoena.dev — the host the device's TLS must trust; nginx on typoena.dev 302-redirects the .bin to the Gitea release asset so binaries never enter the site repo, and `just publish-firmware` cuts the release + writes latest.txt (commit-first for a reproducible tag). A/B rollback (CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE) is enforced on customer units by `just ship`. The 0.7.9 payload that proved it also shipped :about (a version splash), :update naming the running version, and the active filename in the side panel — firmware now 0.7.9."

[[feature]]
name = "v0.8 editor: palette + fonts + panel"
start = 2026-07-22
original = 2026-07-22
delivered = 2026-07-22
learning = "Delivered 2026-07-22 — an unplanned feature batch (like v0.7.5 / v0.7.7), numbered 0.8.0 and taking the roadmap's 0.8 slot, which pushed the planned battery+sleep to v0.10 and moved robustness ahead to v0.9. Headline UX: the command palette opens on Cmd+Shift+P; the writing font family is chosen live from the settings palette (alternate mono families baked into MonoFont atlases, grid-invariant — this lands v1.0's 'runtime-switchable fonts' early); the side panel is grouped into file / sync / vim tiers with friendlier filenames; :pub marks a note .pub.md; the boot splash is a lowercase wordmark. Reliability: the file-walk is pinned to Core1 so it can't starve the UI (fixed the type-to-ink lag), panel ghosting is cleared by scheduled full refreshes, and the crate is pinned to opt-level 2 after 's' miscompiled the wizard into a boot loop. Also: the fast-partial typing waveform (real Good Display GDEY0579T93 LUT, ~495 → ~265 ms) landed behind a default-off fast_partial pref, gated on a longevity + cold soak; a QC bring-up fixture for the carrier PCB; the first-boot wizard can now erase and dedicate a bring-your-own SD card on consent (v0.9's on-device provisioning); and the git-push 'publish' concept was renamed 'push'. Host-tested; fast-partial + QC verified at the bench."

[[feature]]
name = "v0.9 robustness"
start = 2026-11-02
original = 2026-11-30

[[feature]]
name = "v0.10 battery + sleep"
start = 2026-11-30
original = 2026-12-28

[[feature]]
name = "v1.0 polish"
start = 2026-12-28
original = 2027-01-25

[[milestone]]
name = "MVP ships"
week = 2026-06-29
requires = ["v0.1 it writes, it pushes"]
```

## Delivered

v0.1 through v0.8 are shipped; the `learning` field of each `[[feature]]` above
carries what the release cost and what it taught. Release contents, feature by
feature, are in [`../../CHANGELOG.md`](../../CHANGELOG.md); the behaviour they
left behind is in [`../reference/`](../reference/commands.md), not here.

The versions below are the ones still open. Marks: `[x]` done in core ·
`[~]` partially done · `[ ]` not started. An inline `(✓)` marks the done half
of a split item.

---

## v0.9 — Robustness — [~]

Crash-safe writes, interrupted-push recovery, SD removal handling, Wi-Fi
reconnect, and on-device provisioning (the first release usable by a non-author).
**On-device provisioning is DELIVERED early (✓)** — the zero-computer first-boot
wizard, verified on device, which also erases and dedicates a bring-your-own SD
card on consent. The rest is **not started.**

- [ ] Crash-safe writes (write to `.tmp`, fsync, rename) — NB FatFS `f_rename`
      refuses to overwrite, so it's unlink-then-rename + `*.tmp` boot-recovery
      (found in [Spike 3](../record/postmortems/2026-07-05-spike3-sd-cmd59.md), still
      open there)
- [ ] Recover from interrupted push (re-attempt on next save) — the splice
      journal + stranded-commit recovery (2026-07-13) already survive a
      power-pull mid-push; this item is the _retry_ half
- [ ] Reconnect-on-stale-connection in the libgit2 http/stream layer — the
      push keep-alive race is **avoided, not fixed** (repack keeps the marking
      phase at ~3.5 s ≪ the ~30 s idle window; a multi-pack or cold-cache
      state could still lose it — see the
      [real-repo-sync kaizen](../record/kaizen/real-repo-sync.md))
- [ ] `:gl` recovers from a divergence by rebasing — instead of refusing (the
      v0.7 ff-only behavior), the pull replants the device's local commit(s)
      onto origin's tip (`rebase_local_onto`: splice the `merge_base..HEAD`
      paths from the card onto origin's tree, last-writer-wins per note, no
      content merge) and ends `LocalAhead` for `:gs` to push. The branch ref
      moves **last**, after the merged tree is applied to the card, so a
      power-pull mid-rebase leaves HEAD at the old tip and the next `:gl`
      recomputes the identical commit idempotently. Snackbar `rebased <oid> -
      :gs to push`. Core done 2026-07-14 (`git_sync.rs`, full build green);
      **on-device verification pending** — needs a real divergence to
      reproduce (a stranded local commit plus a foreign push to the same
      branch)
- [ ] Eradicate the paint-during-sync DMA allocation failure: persistent
      internal DMA scratch in `Epd` (safety net + allocation-free repaints
      shipped; see the
      [editor-freeze postmortem](../record/postmortems/2026-07-11-editor-freeze-spi-dma-oom-during-sync.md))
- [ ] SD card removal / reinsert handling
- [ ] Wi-Fi reconnect with backoff
- [ ] On-device provisioning + settings screen: SSID, PAT rotation, default
      remote, commit author (replaces the v0.1 dev-only NVS-flashing path —
      first release usable by someone who is not the firmware author).
      **Expanded 2026-07-15 into the onboarding wizard** — first-boot setup on
      an unconfigured card (keyboard Wi-Fi, GitHub device flow via a QR on the
      panel, repo pick, size-gated shallow clone) with `:setup` re-entering the
      same wizard prefilled as the settings/reset screen. Spec:
      [v0.9-onboarding-wizard.md](v0.9-onboarding-wizard.md)

## v0.10 — Power: battery + sleep — [ ]

Bench current-draw measurement, the LiPo power chain (HW-373 + MT3608 per the
PCB migration) with a latching-button soft-power latch + load-sharing power
path (shutdown paints a sleeping-Typo off card so the persistent e-ink shows
the power state), per-sync Wi-Fi teardown, light/deep sleep, the `auto_sync`
runtime, and a battery indicator. **The power chain runs on the bench**; the
soft-power firmware and everything downstream of it is not started.
Detail: [v0.10-battery-and-sleep.md](v0.10-battery-and-sleep.md).

## v1.0 — Polish — [ ]

≤ 3 s boot, runtime-switchable fonts, enclosure files, and a user guide
(light/dark theme landed early in v0.5; **runtime-switchable writing fonts landed
early in v0.8**). **Not started.**

- [ ] Boot time ≤ 3 s to usable cursor — 4.26 s at v0.1 ship; **regressed to
      ~8.7 s on a real 1098-file card** once the v0.5 palette walk joined the
      boot path (4.3 s of readdir-over-SPI, already d_type-optimised), so the walk must
      go async/deferred, and the ~1.9 s cold-boot full refresh + ~0.74 s PSRAM
      memtest are the remaining levers (see
      [`notes/boot-time-budget.md`](../record/notes/boot-time-budget.md))
- [ ] Font selection (at least one serif + one mono) with adjustable font
      size, switchable at runtime and persisted across reboots
- [ ] Theme: light / dark (inverted e-ink), switchable at runtime and
      persisted across reboots. The invert itself is trivial (XOR at blit); the
      unproven part is the panel's behaviour on a predominantly-black frame —
      full-black waveforms stress it differently, and partial-refresh ghosting
      may accumulate faster, forcing a more frequent full refresh. Bench it for
      legibility, ghosting, and partial-refresh latency against the light theme
      before shipping.
- [ ] Enclosure design files in `hardware/`
- [ ] User guide

Quality carry-over: **graduate the fast-partial typing waveform** (custom `0x32`
LUT, ~495 → ~265 ms per keystroke) from the default-off `fast_partial` opt-in to
on-by-default — the last lever on H1, the one unmet v0.1 latency target. Landed on
`main` 2026-07-21 behind the flag; gated on a longevity + cold soak (`0x08` spends
the vendor drive margin). Target tracked in [`qfd.md`](../quality/qfd.md); bench data in
[`tradeoff-curves/epd-refresh-latency.md`](../record/tradeoff-curves/epd-refresh-latency.md). Note: the same
custom waveform is a candidate for the **≤ 3 s boot splash lever** — the boot's
~630 ms full-area partial measured ~300 ms on the custom LUT at the bench — if it
validates cold.

## v1.x — Stretch / nice-to-have

Post-1.0 ideas, not committed to any release:



**Firmware auto-update — RESOLVED + DELIVERED 2026-07-19** (shipped as **v0.7.7**,
well ahead of the pre-v1.0 deadline). Chose **OTA over Wi-Fi**: `:update` fetches a
version manifest (`typoena.dev/firmware/latest.txt`) and, if newer, streams
`typoena-<ver>.bin` into the inactive slot of an A/B layout (`partitions-ota.csv`:
factory + `ota_0` + `ota_1` + `otadata`) via esp-idf OTA and reboots into it.
`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE` gives rollback, enforced on customer units
by `just ship` (which refuses to flash without the rollback bootloader). Images are
hosted on the **Gitea** release for `typoena/typewriter` (git.apoena.dev — the
device validates its LE→ISRG Root X1 chain against the esp-idf FULL CA bundle);
nginx on typoena.dev 302-redirects the `.bin` there, so binaries never touch the
site repo. The **SD-drop alternative wasn't needed.** Not yet done: image signing
(the Gitea release + HTTPS is the trust boundary for now) — a v1.0/v1.x hardening
item. Verified on hardware across two back-to-back installs; see the v0.7.7 entry
above.
