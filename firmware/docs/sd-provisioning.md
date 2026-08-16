# Provisioning an SD card

Typoena reads its config and its notes repo from the SD card — it never
cold-clones the ~566 MB repo over Wi-Fi + mbedTLS (the
[git-sync sizing decision](../../docs/record/notes/git-sync-images-and-repo-size.md)).
Instead a Mac prepares the card over a reader, and the device only ever takes
the `open` + fast-forward path. The [`justfile`](../justfile) has three entry
points, each ejecting the card when done:

```sh
just init ~/code/notes    # full prep of a fresh card: notes repo + config
just load ~/code/notes    # (re)copy just the notes repo → /sd/repo
just provision            # (re)write just the config (rotate PAT, switch Wi-Fi)
```

`init` is the once-per-card command; `load` and `provision` each refresh one
half without touching the other. Add a `/Volumes/<name>` as the last argument if
more than one removable card is mounted — auto-detect refuses on ambiguity,
since a wrong guess would let `rsync --delete` wipe the wrong disk's `repo/`.

## Config with little to type

`typoena.conf` (Wi-Fi + PAT + git identity) needs **no `.env`**. Each value runs
a ladder — `.env` if present, else derived from tools already on the machine,
else an interactive prompt with the derived value as the default:

| Value                                | Derived from                                              |
| ------------------------------------ | --------------------------------------------------------- |
| `TW_REMOTE_URL`                      | the source repo's `origin` (or the card's existing clone) |
| `TW_AUTHOR_NAME` / `TW_AUTHOR_EMAIL` | `git config user.name` / `user.email`                     |
| `TW_GH_USER`                         | `gh api user`                                             |
| `TW_WIFI_SSID`                       | the Mac's active Wi-Fi network                            |
| `TW_WIFI_PASS`                       | the System keychain for that SSID (else prompt)           |
| `TW_TOKEN`                           | **never derived** — sign in with GitHub, or type a token  |

So a first run is usually: `just init ~/code/notes`, press Enter through the
auto-filled defaults, approve the macOS Keychain dialog for the Wi-Fi password
(or type it), and sign in with GitHub (or paste a fine-grained PAT) once.
Reading a saved Wi-Fi password triggers a macOS authorization dialog (login
password / Touch ID → Allow) — that's macOS guarding a System-keychain secret,
not something the recipe can suppress. Keeping [`.env`](../.env.example)
populated stays a valid override and skips all prompts.

## Secrets on the card

FAT has no file permissions, so **physical custody of the card is the only
control** over the plaintext `TW_TOKEN`. The device-flow user token carries
only the Typoena app's grants; a pasted fine-grained PAT should be scoped with
`contents:write` on just the notes repo, so a lost card is a one-token revoke.
The token is never derived from `gh auth token` (a broad token on removable media
would defeat the point) and never echoed — the recipes report each value only as
`set` / `MISSING`.
