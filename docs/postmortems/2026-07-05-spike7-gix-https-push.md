# Spike 7 (git push) — the ADR-004 kill-switch fired: gix can't push over HTTPS

> Date: 2026-07-05
> Status: **turned, not failed** — gix ruled out for the push path; pivoted to
> `libgit2` (`git2`) and proved the git mechanics on desktop. On-device build is
> the next gate.
>
> Context: Spike 7 in
> [`../v0.1-mvp-technical.md`](../v0.1-mvp-technical.md#hardware-bring-up-order),
> git impl [ADR-004](../adr.md#adr-004-git-implementation--gitoxide-gix), auth
> [ADR-005](../adr.md#adr-005-auth--https--github-personal-access-token).
> Spike program: [`../../spikes/spike7-git-push/`](../../spikes/spike7-git-push/).

## Summary

Spike 7 was written as the kill-switch for [ADR-004](../adr.md): *"the
smart-HTTP path is validated in spike 7 before we commit to integration; if it
fails on the device, we fall back to `libgit2-sys`."* It never needed a device
to fire. Before writing any gix code, gitoxide's own crate-status doc settles
the question: `gix` has send-pack/receive-pack **plumbing** (report-status,
sideband, delete-refs, atomic pushes) but supports push as a **workflow** only
over `file://` and `ssh://`. **Push over HTTP(S) is not implemented** — push is
still listed under "workflows that still need plumbing." (Clone/fetch, by
contrast, are robust over HTTP(S) — which is why Spike 6's TLS GET passed but
does not carry over to push.)

Because [ADR-005](../adr.md) fixes auth as **HTTPS + PAT**, `gix` cannot satisfy
the push path today. gix *can* push over `ssh://`, but that would (a) revisit
ADR-005 and (b) still die on device — gix's SSH transport spawns the external
`ssh` program, which does not exist on the ESP32. So the kill-switch condition
is met at the library level.

**Decision:** take the fallback the risk table already names — `libgit2` via the
[`git2`](https://docs.rs/git2) crate — keeping ADR-005 (HTTPS + PAT) intact.
Proved the full `add → commit → push` sequence on desktop
([`spikes/spike7-git-push`](../../spikes/spike7-git-push/)).

## Why not the alternatives

| Option | Verdict |
| ------ | ------- |
| **gix + HTTPS** (as ADR-004 intended) | Blocked — gix has no HTTP(S) push. |
| **gix + SSH push** | gix supports it, but revisits ADR-005 *and* gix's SSH transport shells out to an `ssh` binary absent on ESP32 → dead on device. |
| **gix-protocol send-pack + custom HTTPS transport** | Pure-Rust, no ADR change, but not smoke-test-sized: hand-wiring send-pack over an mbedtls HTTP transport is real work and unproven upstream. Reconsider only if the libgit2 cross-compile (below) turns out worse. |
| **libgit2 (`git2`)** ← chosen | The ADR's named fallback. Trivial on desktop; the risk becomes the on-device cross-compile. |

## What the desktop spike proves

Run live against a local `file://` bare remote (no credentials), exercising the
exact v0.1 `git` module contract:

- **first commit + push** from an unborn `HEAD` (fresh clone of an empty repo)
  → the commit lands in origin. Message is an ISO-8601 timestamp.
- **nothing to publish** → short-circuits when the staged tree matches `HEAD`.
- **divergence** → a second clone advances origin; the first clone's push is
  rejected, `pull --no-edit` merges cleanly (different files), the retry push
  succeeds, and origin ends with a correct two-parent merge commit.

Also confirmed 2026-07-05 against a **real GitHub repo** (`jcalixte/typoena-test`)
over HTTPS with a fine-grained PAT: `committed → push accepted by remote`, the
commit landed on GitHub. So the TLS handshake + PAT auth + smart-HTTP push all
work through libgit2's vendored stack (desktop links `openssl-sys` for TLS). The
one path still unexercised live is a **non-fast-forward rejection over HTTPS**
(the `push_update_reference` callback) — the `file://` transport surfaced that as
a `push()` error instead, and the GitHub push was a clean fast-forward.

Implementation notes that carry into the real module:

- **`git add --all` semantics.** libgit2's `index.add_all(["*"], DEFAULT)` stages
  new + modified + **deleted** paths, unlike a naive `git add .`. v0.5 file-delete
  needs removals to reach the next Publish's staged set — this is that behavior.
- **Push rejection is not always a `push()` error.** A non-fast-forward can come
  back as a transport `Err` (local transport did this) *or* silently via the
  `push_update_reference` callback with a status string while `push()` returns
  `Ok` (the HTTPS/GitHub path). The spike handles both and routes either to the
  pull-and-retry. The callback path is coded for but not yet exercised live.
- **PAT hygiene.** The token is handed only to libgit2's credential callback
  (`Cred::userpass_plaintext`) and never logged — matches ADR-005.

## What it does *not* prove — the next gate

The risk moved **with** the kill-switch, and arguably got harder. ADR-004 chose
gix *specifically to avoid* libgit2's C cross-compile to xtensa; falling back to
libgit2 re-introduces exactly that. The open question is now:

> Can `libgit2` (`git2` / `libgit2-sys`) cross-compile to
> `xtensa-esp32s3-espidf` and use esp-idf's **mbedtls** as its TLS backend?

`libgit2-sys` vendors libgit2 and, on desktop, pulled `openssl-sys` for TLS —
there is no openssl on esp-idf, so the device build will need libgit2 pointed at
mbedtls (its `MbedTLS` backend) via the esp-idf sysroot, which is unproven. This
is the on-device Spike 7 and it also depends on:

- **PSRAM** (`CONFIG_SPIRAM`) enabled — still off (only ~339 KB internal heap;
  see firmware README / Spike 6 note). libgit2's pack working set needs it.
- **A working SD card** (Spike 3, currently
  [paused on a CMD59-incompatible card](2026-07-05-spike3-sd-cmd59.md)) for the
  `/sd/repo` working copy.

So the full **SD → push** loop is still not testable on hardware; this spike
retired the *library/API* risk and replaced it with a *cross-compile* risk to
tackle once PSRAM + SD are unblocked.

## Follow-ups

- [ ] On-device Spike 7: cross-compile `git2`/`libgit2-sys` for
      `xtensa-esp32s3-espidf` with the **mbedtls** TLS backend; if it won't build,
      reconsider the gix-plumbing custom-transport route.
- [ ] Enable PSRAM (`CONFIG_SPIRAM`) — prerequisite for the git working set.
- [x] Run the desktop spike against a real GitHub test repo — **done 2026-07-05**
      (`jcalixte/typoena-test`, fine-grained PAT): HTTPS handshake + PAT auth +
      push confirmed. Still open: the `push_update_reference` rejection path over
      HTTPS (needs a non-fast-forward against a real remote to trigger it).
- [ ] Revise the `git` module section of the technical doc (it still describes
      gix crates/transport) once the device path is confirmed.

## Artifacts (this session)

- `spikes/spike7-git-push/` — the desktop spike crate (`src/main.rs`,
  `Cargo.toml`, `README.md`, `.env.example`).
- ADR-004 — outcome note appended (kill-switch fired → libgit2).
- `docs/v0.1-mvp-technical.md` — risk-table row updated (gix push → libgit2).
