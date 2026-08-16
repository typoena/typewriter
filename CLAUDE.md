# Typoena (typewriter repo)

## Who writes here

Everything in this repo — firmware, docs, commit messages — is written by an
Opus model **newer than 4.8**. Check the running model before authoring; if it
is Sonnet, Haiku, or an older Opus, say so and stop rather than write.

The rule follows the work, not the session: a subagent or workflow that edits
files here needs the same model, so pass the override explicitly instead of
letting a cheaper tier inherit the task. Read-only work — searching, locating,
summarising — has no such constraint.

## Flashing the device

Never flash the device (`just flash`, `flash-only`, `flash-ota`, `espflash`, …) —
Julien flashes on his side. Building (`just build`, `cargo build`) is fine and
encouraged to verify changes compile.

## Comment rules

Comments stay lean. A comment earns its place only when it carries something the
code can't:

- **Magic-byte / hardware semantics** — what a register value or command sequence
  means (`0xCF` = display without reloading the OTP LUT), why a write goes to both
  controllers, active-high BUSY, etc.
- **Hazards refuted on hardware** — "do NOT do X" warnings that no test can
  encode (gate-scan restriction, RED-participating full kick, `opt-level=s`).
  One or two lines stating the hazard + a pointer to the postmortem/tradeoff doc.
- **Bench findings with no testable home** — a closed experiment whose scaffolding
  remains (`PARTIAL_TEMP`), a measured constant (`RAM_SETTLE_MS = 0`). State the
  finding in a line or two, link the log.
- **Contracts and invariants** — recovery decision tables, idempotence under
  power-pull, why an API must be called in an order.

Everything else — experiment history, dated sagas, provenance stories, v1→vN
iteration trails — lives in `docs/` (tradeoff-curves, postmortems, notes); the
comment keeps only the conclusion plus the pointer. One home per rationale: never
tell the same story in two comments — pick the natural site (usually the field or
const), and have the other point to it.

## Documentation describes the present

Code and docs state what **is**, never what *was*. Git history is where the past
lives, and it's the tool we actually use to reach it — so a doc never needs to
carry the story forward itself.

Delete on sight, rather than rewrite:

- obituaries for removed things ("the earlier two-board split has been removed…",
  "superseded by…", "recover it from git history if needed")
- migration trails and `v1 → vN` framing ("was `⟨TBD⟩` / now", "what changed
  versus the perfboard build", "this used to be an AMS1117")
- status prose that only made sense against a former state ("still in place as a
  reference", "not yet migrated")

Keep the **rationale for what exists now**, stated in the present: why this part,
this value, this pinout, this hazard. When the reason is genuinely a road already
travelled — a measured tradeoff, a postmortem — keep the conclusion and link the
doc in `docs/`, don't retell the trip.

Applies to a rename or a deletion too: after it, no file should mention the old
name. Not a redirect, not a note — gone.

## KiCad MCP (hardware/pcb)

PCB work needs the `kicad` MCP server. If its `mcp__kicad__*` tools are absent,
the container was rebuilt: everything lives on a persistent volume, but the
registration is stored in `/home/jean/.claude.json`, which sits at the ephemeral
home root. Re-register with:

```sh
~/.local/share/com.jean.desktop/tools/kicad9/restore-after-rebuild.sh
```

Idempotent — it verifies the volume is intact, then re-runs `claude mcp add`.
The tools only load at session start, so **the session must be restarted
afterwards**; say so rather than retrying the script.

If it reports a missing file, the volume itself is damaged — see the
`kicad-headless-mcp` memory note for how the toolchain was built.

## docs/ folder

Do not proactively read, scan, or `ls` the `docs/` folder. It's large and mostly
historical spec / QFD / postmortem material already summarized in memory.

Only open `docs/` when the current task is explicitly to **create, read, update, or
delete** a file in there (or when a task plainly requires one specific spec — open
that named file directly, don't browse the folder).
