# Build details — esp-idf, libgit2, the `full` feature

The first build is slow (the esp-idf C sources are checked out and built
under `.embuild/`; the build also compiles libgit2 + mbedTLS). Subsequent builds
are incremental — libgit2 is a fingerprint-cached esp-idf component, so editing
Rust never recompiles it. Editor and render-engine logic is host-tested
off-device (`cargo test -p app -p editor`), which is the fast iteration loop.

## The `full` feature — why libgit2 stays behind a switch

Pushing (`:gs`/`:gl` → git) and `:update` (OTA) drag in libgit2 + mbedTLS
(compiled as an esp-idf component) and the `git2` crate — expensive to build.
The `firmware` bin sets `required-features = ["full"]`, so the product firmware
always has them. `full` is nonetheless **off by default**, for one reason: a
bare `cargo build` and the standalone bench bins build WITHOUT libgit2.

| Target                            | `full`                     | libgit2 component          | `git2` crate |
| --------------------------------- | -------------------------- | -------------------------- | ------------ |
| `firmware` (`just build`/`flash`) | always (required-features) | compiled                   | linked       |
| bench bins (`just build-bench`, …) | off                       | not compiled (empty no-op) | not linked   |

Two independent switches gate libgit2, and the `full` recipes flip them together:

1. **`full` Cargo feature** (`--features full`) — pulls the `git2`/`libgit2-sys`
   crates and the `net`/`ota`/`wizard_io` modules. The `firmware` bin requires
   it; the bench recipes omit it.
2. **`LIBGIT2_SRC` env** — the [libgit2 component](../components/libgit2/CMakeLists.txt)
   only compiles its sources when this points at the vendored tree; unset, it
   registers an _empty_ component. Only the full recipes set it.
