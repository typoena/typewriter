# Docs

> The design record for Typoena — the decisions, specs, and bench write-ups
> behind the writing appliance. Start with the [ADRs](adr.md) for the
> load-bearing choices, or the [v0.1 specs](v0.1-mvp-product.md) for what the
> first release actually does. Building one? The [BOM](bom.md) lists every
> physical part with its reference.
>
> Project overview: [`../README.md`](../README.md).

## Decisions & specs

| Doc                                              | What's in it                                                                                                         |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| [`adr.md`](adr.md)                               | Architecture Decision Records — the load-bearing technical choices and why.                                          |
| [`stack.md`](stack.md)                           | Software stack — layer-by-layer choices with measured costs, and the annotated repo layout.                          |
| [`v0.1-mvp-product.md`](v0.1-mvp-product.md)     | v0.1 product design — boot, type one file, save, push.                                                               |
| [`v0.1-mvp-technical.md`](v0.1-mvp-technical.md) | v0.1 technical design — single Rust binary on `esp-idf-rs`, modules, threads, bring-up order.                        |
| [`macroplan.md`](macroplan.md)                   | Version-by-version plan; each release is a usable artifact, not a checkpoint.                                        |
| [`typoena-toml.md`](typoena-toml.md)             | `.typoena.toml` reference — the git-tracked editor preferences (auto-save, format-on-save, line numbers, auto-sync). |
| [`typoena-snippets.md`](typoena-snippets.md)     | `.typoena.snippets.json` reference — the git-tracked, Zed-compatible snippet library.                                |
| [`bom.md`](bom.md)                               | Whole-project bill of materials — every physical part by subsystem, with its reference.                              |
| [`wiring.md`](wiring.md)                         | Every electrical connection, one table per subsystem — the companion to the BOM.                                     |

## Release specs

One page per release still carrying live scope. Delivered releases are
summarised in [`macroplan.md`](macroplan.md) and [`../CHANGELOG.md`](../CHANGELOG.md).

| Doc                                                        | What's in it                                                                        |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| [`v0.5-palette-and-multi-file.md`](v0.5-palette-and-multi-file.md) | File palette, multi-buffer lifecycle, and the prefs loop.                     |
| [`v0.6-markdown.md`](v0.6-markdown.md)                     | Markdown render affordances and the snippet engine.                                 |
| [`v0.7-search-and-git.md`](v0.7-search-and-git.md)         | `/` search, `:gl` pull, and the memory/transport fixes that rode along.             |
| [`v0.7.5-focus-mode.md`](v0.7.5-focus-mode.md)             | The silent focus block and the full-screen rest card.                               |
| [`v0.7.6-reboot.md`](v0.7.6-reboot.md)                     | `:reboot` — a software restart that can paint before it resets.                     |
| [`v0.7.7-inbox-notes.md`](v0.7.7-inbox-notes.md)           | `:inbox` / `:oldest` — dated fleeting notes.                                        |
| [`v0.9-onboarding-wizard.md`](v0.9-onboarding-wizard.md)   | First-boot setup and `:setup` — the zero-computer provisioning flow.                |
| [`v0.9-robustness.md`](v0.9-robustness.md)                 | Crash-safe writes, interrupted-push recovery, card removal. Not started.            |
| [`v0.10-battery-and-sleep.md`](v0.10-battery-and-sleep.md) | Power switch, power path, battery, sleep. Design decided, not built.                |
| [`v1.0-polish.md`](v1.0-polish.md)                         | Boot time, fonts, light/dark theme, enclosure files, user guide. Not started.       |
| [`v1.x-stretch.md`](v1.x-stretch.md)                       | Post-1.0 ideas, not committed to any release.                                       |

## Conventions

| Doc                        | What's in it                                                                                                            |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| [`testing.md`](testing.md) | Where Rust tests live — unit tests in-file vs the `editor` crate's `src/tests/` behavioural submodule; how to run them. |

## Quality method

| Doc                                                | What's in it                                                                                                                            |
| -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| [`qfd.md`](qfd.md)                                 | Quality Function Deployment **hub** — what-matters-now headlines, the page index, and the keep-honest rules. Start here.                |
| [`qfd-house-1.md`](qfd-house-1.md)                 | House 1 (WHATs × HOWs, 16×16) with §1 requirements, §2 characteristics, §3 reading, §4 roof.                                           |
| [`qfd-perception.md`](qfd-perception.md)           | Competitive perception — five products scored 0–5 per WHAT, measured benchmarks, caveats.                                              |
| [`qfd-house-2.md`](qfd-house-2.md)                 | House 2 (HOWs × components) — §5 cascade tree, component ranking, shared-pool budget matrix.                                           |
| [`qfd-houses-3-4.md`](qfd-houses-3-4.md)           | Houses 3 & 4 under the pipeline reading — processes P1–P9 × controls Q1–Q8.                                                            |
| [`qfd-budget.md`](qfd-budget.md)                   | §6 critical performance budget — ranked targets, verdicts, and the named fallback per row.                                            |
| [`qfd-tradeoffs.md`](qfd-tradeoffs.md)             | §7 tradeoffs T1–T15 and the tensions left deliberately unresolved, each with its trigger.                                             |
| [`house-vs-product.md`](house-vs-product.md)       | Standing challenges between the scored houses and the real product — open disputes with evidence and resolution triggers.               |

## Bench work

| Area                                            | What's in it                                                                                                                                  |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| [`postmortems/`](postmortems/README.md)         | Bring-up debugging write-ups: what broke, the root cause, and the decisions that came out of it.                                              |
| [`notes/`](notes/README.md)                     | Longer-form essays on the thinking behind specific choices — e.g. where the ~16 s cold [`:gs`](notes/sync-latency.md) goes.                   |
| [`tradeoff-curves/`](tradeoff-curves/README.md) | Cost-vs-knob curves behind chosen defaults — energy, latency, memory.                                                                         |
| [`kaizen/`](kaizen/README.md)                   | Six-step kaizen write-ups — the problem→analysis→fix story behind an improvement, e.g. the real-repo [sync brick](kaizen/real-repo-sync.md).  |
| [`../firmware/docs/bring-up-spikes.md`](../firmware/docs/bring-up-spikes.md) | The chronological bench log of Spikes 1–7 that brought the firmware up on real silicon.                  |
