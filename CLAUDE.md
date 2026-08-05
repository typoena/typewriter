# Typoena (typewriter repo)

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
