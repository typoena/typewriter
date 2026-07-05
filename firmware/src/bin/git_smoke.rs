//! Spike 7 — Path 2 Gate D: the `git2` safe API on device.
//!
//! Gates A/B/C proved libgit2 compiles, links, and runs via hand externs. This
//! replaces those with the real integration path: the `git2` crate's safe Rust
//! API, bound to our esp-idf-built libgit2 through `libgit2-sys` in system mode
//! (LIBGIT2_NO_VENDOR=1 + the fake pkg-config in firmware/pkgconfig/). If this
//! links and runs, the desktop spike's add/commit/push code transfers to device.
//!
//! Two checks, both in-memory (no filesystem or network needed yet):
//!   1. git2::Version — proves the safe wrapper reaches the linked library.
//!   2. Oid::hash_object — computes a blob SHA1 through libgit2's ODB, which
//!      runs the mbedTLS hash backend. The expected hash is known, so a correct
//!      value proves the whole path (git2 -> libgit2 -> mbedtls) end to end.

use esp_idf_svc::sys;
use git2::{ObjectType, Oid};

fn main() {
    sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Typoena — Spike 7 Path 2 Gate D (git2 safe API on device)");

    let (major, minor, patch) = git2::Version::get().libgit2_version();
    log::info!("git2 crate is talking to libgit2 {major}.{minor}.{patch}");

    // `git hash-object` of the 5 bytes "hello" is a fixed, well-known value.
    match Oid::hash_object(ObjectType::Blob, b"hello") {
        Ok(oid) => {
            log::info!("sha1(blob \"hello\") = {oid}");
            if oid.to_string() == "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0" {
                log::info!("hash matches `git hash-object` — mbedTLS SHA1 backend correct");
            } else {
                log::warn!("hash MISMATCH — mbedTLS SHA1 backend produced the wrong digest");
            }
        }
        Err(e) => log::error!("Oid::hash_object failed: {e}"),
    }

    log::info!("✅ git2 safe API linked and ran on device");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
