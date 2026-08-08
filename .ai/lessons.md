# Lessons

## "on save" means the device, not the host tooling (2026-08-08)

An editor-behavior ask in this repo ("on save, …", "lint on save") is about the
Typoena device's editor — the `editor` crate's core plus its `app`/`firmware`
adapters. It is not about Zed settings, Claude Code hooks, or repo tooling.
Ask before touching `.zed/` or `.claude/` for a behavior request.

## Storage-layer guarantee ≠ user-visible behavior (2026-08-08)

Claimed "firmware already appends the trailing newline" from
`storage_sd.rs:atomic_write`, and it was wrong from the writer's seat: the byte
landed on the file, but the buffer never gained the empty last row, so nothing
changed on screen and the buffer stayed a byte out of step with its file.
Before saying a feature exists, check the layer the user actually observes
(buffer / screen), not just the deepest layer that touches the bytes.
