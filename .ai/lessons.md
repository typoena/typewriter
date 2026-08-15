# Lessons

## "on save" means the device, not the host tooling (2026-08-08)

An editor-behavior ask in this repo ("on save, …", "lint on save") is about the
Typoena device's editor — the `editor` crate's core plus its `app`/`firmware`
adapters. It is not about Zed settings, Claude Code hooks, or repo tooling.
Ask before touching `.zed/` or `.claude/` for a behavior request.

## A research conversation is not a change request (2026-08-15)

Session `16ecf0f4` (2026-08-09) was twelve questions about buying a 40% Planck
— "where can I buy one of these", "and the keys?", "1u?". It ended in
`6d148d4`, editing `typoena-case-kb.scad`, `README-kb.md` and `hardware/justfile`.
The research was the ask and was answered; the model edit nobody requested.

When research turns up a real model defect, name it and offer the fix. Land it
when asked.

## Read the parameter block, not the whole model (2026-08-15)

`typoena-case.scad` is 34.5 KB and `README.md` 17.7 KB — ~13k tokens to read
both in full, for a change that moves one constant. The 84 parameters live in a
banner-separated block at lines 30–285. Read that plus the single module that
consumes the constant.

## Storage-layer guarantee ≠ user-visible behavior (2026-08-08)

Claimed "firmware already appends the trailing newline" from
`storage_sd.rs:atomic_write`, and it was wrong from the writer's seat: the byte
landed on the file, but the buffer never gained the empty last row, so nothing
changed on screen and the buffer stayed a byte out of step with its file.
Before saying a feature exists, check the layer the user actually observes
(buffer / screen), not just the deepest layer that touches the bytes.
