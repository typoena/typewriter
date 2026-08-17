# libgit2 POSIX shims on esp-idf

`firmware/components/libgit2/` replaces the POSIX surface libgit2 expects but
esp-idf and FATFS do not provide. The bring-up story is in
`docs/record/postmortems/2026-07-05-spike7-gix-https-push.md`; this is the
standing shim surface.

## Absent flags

`O_BINARY` is defined to 0 — esp-idf draws no text/binary distinction, so the
flag has nothing to select. `O_CLOEXEC` is defined to 0 — there is no `exec()`
on esp-idf, so close-on-exec is a no-op.

## Replaced calls

`p_open`, `p_creat` and `p_rename` are defined here; `posix.c`'s originals are
compiled under `libgit2_unused_*` names (see the component `CMakeLists.txt`), so
these are the definitions every other translation unit links against. Their
rationale — forcing `S_IWUSR` so FATFS never sets `AM_RDO`, and drop-then-rename
replace semantics — is stated at each function.

`getuid` and friends return one implicit root user. `utimes` only checks the path
exists; a failed `stat` has already set `errno` (`ENOENT` for a missing object).

`gai_strerror` is provided because lwip implements `getaddrinfo` without it.

## mmap emulation

`esp_map.c` reads the requested range into a heap buffer. A file shorter than the
requested length stops the read loop and leaves the tail zero-filled, which is
what a real mapping past end-of-file gives.

## TLS stream

`esp_mbedtls_stream.c` strips the trailing newline mbedTLS puts on its
verification-result string. Session resumption moves the saved session shallowly
— ownership transfers to the new holder, so the old one must not be freed.
