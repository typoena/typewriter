//! Infrastructure — adapters that fulfil the `app` ports over libraries and
//! protocols.
//!
//! SD/FAT storage ([`storage_sd`], `app::Storage`), the background palette file
//! index ([`file_index`], `app::FileIndex`), the git publish transport plus its
//! `app::SyncService` ([`sync_git`], `git` build) or the light-build no-op
//! ([`sync_null`]), and the onboarding wizard's hardware I/O ([`wizard_io`],
//! `git` build). Mirrors the `infrastructure/` tier of the C `../typing-machine`
//! reference.

pub mod file_index;
pub mod storage_sd;

// The git transport is feature-gated (it pulls libgit2); the light build swaps
// in the inert no-op so the run loop's `SyncService` port is always satisfied.
#[cfg(feature = "git")]
pub mod sync_git;
#[cfg(not(feature = "git"))]
pub mod sync_null;

// The onboarding wizard's end state is a clone, so it only exists in the git
// build (see the `wizard` optional dep in Cargo.toml).
#[cfg(feature = "git")]
pub mod wizard_io;
