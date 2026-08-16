# Ex commands and palette actions

> Every `:` command the editor answers, and the `>` palette actions that mirror
> them. Source of truth is `execute_command` in
> [`../../editor/src/lib.rs`](../../editor/src/lib.rs); this page is the reader's
> copy. Vocabulary (Save, Push, Tracked, Local): [`../../CONTEXT.md`](../../CONTEXT.md).

There is no `:q` family. An always-on writing appliance has nothing to quit to,
so `:wq` and `:x` just save and the quit half is dropped. An unrecognised
command is silently ignored.

## Files

| Command | What it does |
| --- | --- |
| `:w` `:wq` `:x` | Save the active buffer. Runs `:fmt` first when `format_on_save` is set. |
| `:enew <path>` | Create a file at `<path>`. Bare `:enew` prints the usage notice. |
| `:delete` `:d` | Delete the active file, behind a y/n confirm. |
| `:fmt` | Markdown normalizer — table alignment, blank-line collapse, trailing-whitespace strip. |
| `:pub` `:publish` | Rename `<name>.md` to `<name>.pub.md` and retarget every `[title](…)` link pointing at the old name, card-wide. Refuses on an unnamed scratch, a Local file, a non-`.md` file, an already-published file, or when the `.pub.md` name is taken — it never silently clobbers. |

There is no `:e` — bare `Cmd-P` opens files, `> new file` creates them.

## Fleeting notes

| Command | What it does |
| --- | --- |
| `:inbox` `:in` | Open today's fleeting note, creating it if new. Lives in the git-tracked `_inbox/` as `YYYY-MM-DD.md` (ISO order, so a listing sorts chronologically), prefilled with a `# DD/MM/YYYY` heading. Switches to the note if it is already open or already on the card. Refuses when the clock is unset — a clear notice beats a note dated `1970-01-01`. |
| `:oldest` `:old` | Open the oldest note in `_inbox/` for cleanup. The palette list is path-sorted and the names are ISO dates, so the first `_inbox/` entry is the oldest — no dates parsed. Needs no clock, so it works offline at any time. |

## Sync

| Command | What it does |
| --- | --- |
| `:gs` | Push: fmt → save → commit → push. Also bound to a bare `gs` in Normal mode, and to `> push`. Refused on a Local buffer. `gs` and not `gp` because vim binds `gp` to paste-after, and a paste habit must never fire a push. |
| `:gl` | Pull: fetch, then fast-forward. |

`:gl` reads the ref advertisement first, so "up to date" and "local ahead"
answer without entering pack negotiation. The fast-forward is an `O(changed)`
tree-diff apply, not a `checkout_tree` — libgit2's SAFE checkout walks the whole
working directory, which exhausts internal DRAM on a real card. Any file about
to be clobbered whose content no longer hashes to the old blob aborts the pull
before the first write, so edits made behind git's back are never lost. The ref
moves last, so a half-applied working copy self-heals: the next `:gl` re-applies
identically.

On divergence `:gl` rebases the device's local commits onto origin
(last-writer-wins per note, no content merge) and ends local-ahead for `:gs` to
push. Snackbars: `pulled <oid>` · `up to date` · `ahead - :gs to push` ·
`rebased <oid> - :gs to push` · `pull: <reason>`.

Latency breakdown for a cold push:
[`../record/notes/sync-latency.md`](../record/notes/sync-latency.md).

## Device

| Command | What it does |
| --- | --- |
| `:setup` | Reboot into the onboarding wizard, prefilled from the card, behind a y/n confirm. Refuses up front while anything is unsaved — the reboot would lose it. Spec: [`../plan/v0.9-onboarding-wizard.md`](../plan/v0.9-onboarding-wizard.md). |
| `:reboot` | Restart the device, behind a y/n confirm. Auto-saves named dirty buffers on confirm and paints a "restarting…" screen before resetting. An *unnamed* dirty scratch has nowhere to save to, so it blocks with a notice instead of prompting. |
| `:update` | Over-the-air firmware update: fetch a newer image into the inactive A/B slot and reboot into it. Refuses while the buffer is dirty — the post-install reboot would lose the edit. |
| `:about` | Full-screen splash with the running firmware version. Read-only: every key but `Enter`/`q`/`Esc` is swallowed. |
| `:settings` | Open the `>` command palette. |

A hardware RST button cannot show an offboarding screen — no firmware runs in
that window, so the boot splash is the earliest possible feedback. `:reboot`
exists so an intentional restart *looks* intentional.

## Focus

| Command | What it does |
| --- | --- |
| `:focus` | Toggle a focus session: a silent block, then a full-screen rest card. No visible countdown at any point — e-ink cannot show one cheaply. `Ctrl-C` continues, `Ctrl-Q` quits. |
| `:focusdebug` | Flip the time-base to 25 **seconds** per block so the whole cycle is testable in seconds. Independent of whether a session is running. |

## Palette

`Cmd-P` opens files, `Cmd-Shift-P` opens commands, and `$` opens snippets.

| Action | Notes |
| --- | --- |
| `new file...` | Two-step: prompts for the name. |
| `add local link...` | Pick a file, insert a link to it. |
| `follow link` | Open the link under the caret. |
| `format` `push` `reboot` | Mirror `:fmt`, `:gs`, `:reboot`. One-shots close the palette. |
| `setup...` `update firmware` | Mirror `:setup`, `:update`. |

Preference toggles stay open so several can be flipped in one visit; Enter
rotates a preference to its next value. The preferences themselves are in
[`typoena-toml.md`](typoena-toml.md), the snippet library in
[`typoena-snippets.md`](typoena-snippets.md).
