# Docs

> The design record for Typoena. Four trees, split by what you are doing:
> **reference** is what is true right now, **plan** is what is coming,
> **quality** is how requirements are weighed, and **record** is dated and
> append-only.
>
> Start with the [ADRs](adr.md) for the load-bearing choices, or
> [commands](reference/commands.md) for what the device actually answers to.
> Project overview: [`../README.md`](../README.md).

## Decisions

| Doc | What's in it |
| --- | --- |
| [`adr.md`](adr.md) | Architecture Decision Records — the load-bearing technical choices, rejected alternatives included. |

## Reference — true now, no dates

| Doc | What's in it |
| --- | --- |
| [`reference/commands.md`](reference/commands.md) | Every `:` command and `>` palette action, and what each one refuses to do. |
| [`reference/stack.md`](reference/stack.md) | Software stack layer by layer, with measured costs and the annotated repo layout. |
| [`reference/typoena-toml.md`](reference/typoena-toml.md) | `.typoena.toml` — the git-tracked editor preferences. |
| [`reference/typoena-snippets.md`](reference/typoena-snippets.md) | `.typoena.snippets.json` — the git-tracked, Zed-compatible snippet library. |
| [`reference/testing.md`](reference/testing.md) | Where Rust tests live and how to run them. |
| [`../hardware/bom.md`](../hardware/bom.md) | Bill of materials — every physical part by subsystem. |
| [`../hardware/wiring.md`](../hardware/wiring.md) | Every electrical connection, one table per subsystem. |

## Plan — what is coming

| Doc | What's in it |
| --- | --- |
| [`plan/macroplan.md`](plan/macroplan.md) | The delivery plan: macroplan source, per-release status, and the open checklists for v0.9, v1.0 and v1.x. |
| [`plan/v0.1-mvp-product.md`](plan/v0.1-mvp-product.md) | v0.1 product design — the founding spec, as shipped. |
| [`plan/v0.1-mvp-technical.md`](plan/v0.1-mvp-technical.md) | v0.1 technical design — architecture, boot sequence, module breakdown. |
| [`plan/v0.9-onboarding-wizard.md`](plan/v0.9-onboarding-wizard.md) | First-boot setup and `:setup` — the zero-computer provisioning flow. |
| [`plan/v0.10-battery-and-sleep.md`](plan/v0.10-battery-and-sleep.md) | Power switch, power path, battery, sleep. Design decided, not built. |

Delivered releases are summarised in [`plan/macroplan.md`](plan/macroplan.md)
and listed in full in [`../CHANGELOG.md`](../CHANGELOG.md). Their behaviour
lives in the reference pages above, not in per-release specs.

## Quality — how requirements are weighed

| Doc | What's in it |
| --- | --- |
| [`quality/qfd.md`](quality/qfd.md) | QFD hub — what-matters-now headlines, §6 the critical performance budget, §7 the tradeoffs and unresolved tensions. Start here. |
| [`quality/glossary.md`](quality/glossary.md) | Methodology vocabulary: the WHAT / Function / Characteristic / Metric / Target ontology. |
| [`quality/qfd-house-1.md`](quality/qfd-house-1.md) | House 1 (WHATs × HOWs) — §1 requirements, §2 characteristics, §3 reading, §4 roof. |
| [`quality/qfd-house-2.md`](quality/qfd-house-2.md) | House 2 (HOWs × components) — §5 cascade tree, component ranking, shared-pool budget. |
| [`quality/qfd-houses-3-4.md`](quality/qfd-houses-3-4.md) | Houses 3 & 4 under the pipeline reading — processes P1–P9 × controls Q1–Q8. |
| [`quality/qfd-perception.md`](quality/qfd-perception.md) | Competitive perception — five products scored 0–5 per WHAT. |
| [`quality/house-vs-product.md`](quality/house-vs-product.md) | Standing challenges: where the scored houses and the real product disagree. |

Device vocabulary (Save, Push, Tracked, Local) is in
[`../CONTEXT.md`](../CONTEXT.md), not here.

## Record — dated, append-only, never pruned

| Area | What's in it |
| --- | --- |
| [`record/postmortems/`](record/postmortems/README.md) | What broke, the root cause, and the decision that came out of it. |
| [`record/kaizen/`](record/kaizen/README.md) | Six-step improvement loops: a recurring cost measured, attacked, re-measured. |
| [`record/tradeoff-curves/`](record/tradeoff-curves/README.md) | Cost-vs-knob curves behind chosen defaults — energy, latency, memory. |
| [`record/notes/`](record/notes/README.md) | Longer-form essays: the arguments too big for an ADR, too durable for a commit message. |
| [`../firmware/docs/bring-up-spikes.md`](../firmware/docs/bring-up-spikes.md) | The chronological bench log of Spikes 1–7 that brought the firmware up on silicon. |
