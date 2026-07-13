use std::process::Command;

fn main() {
    embuild::espidf::sysenv::output();

    // Stamp the binary so serial output and on-panel text identify the
    // exact build (bring-up lesson: know which build you're diagnosing).
    let git = Command::new("git")
        .args(["describe", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let time = Command::new("date")
        .args(["-u", "+%m-%d %H:%M"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=BUILD_GIT={git}");
    println!("cargo:rustc-env=BUILD_TIME={time}Z");

    // Wi-Fi credentials for the network spikes (6/7) and, later, the runtime.
    // Read at build time and emitted as compile-time env so a binary can pull
    // them in with env!(). Empty when unset: the network spike checks at
    // runtime and prints a clear message, so the *editor* build never has to
    // carry Wi-Fi creds. Source them from firmware/.env (loaded by `just`).
    //
    // The TW_REMOTE_URL / TW_GH_USER / TW_PAT / TW_AUTHOR_* vars back Spike 7's
    // on-device push (src/bin/git_push.rs). env!() embeds a value only in a
    // binary that references it, so the editor binary carries none of these.
    // NOTE: TW_PAT ends up in the git_push image — fine for the bench spike, but
    // a product must not bake the PAT into flash (ADR-005).
    for var in [
        "TW_WIFI_SSID",
        "TW_WIFI_PASS",
        "TW_REMOTE_URL",
        "TW_GH_USER",
        "TW_PAT",
        "TW_AUTHOR_NAME",
        "TW_AUTHOR_EMAIL",
    ] {
        let val = std::env::var(var).unwrap_or_default();
        println!("cargo:rustc-env={var}={val}");
        println!("cargo:rerun-if-env-changed={var}");
    }

    // A git-feature build with an empty publish config can only ever fail at
    // runtime (git_sync's publish_cycle guard), so refuse it here instead.
    // env!() bakes the values at compile time and only `just` dotenv-loads
    // firmware/.env — a plain `cargo build --features git` in a bare shell
    // silently produced a firmware whose `:sync` could never work (bit the
    // 2026-07-13 flash). TW_WIFI_PASS may be legitimately empty (open network)
    // and TW_AUTHOR_* have runtime defaults, so only the four required vars
    // are checked.
    if std::env::var("CARGO_FEATURE_GIT").is_ok() {
        let missing: Vec<&str> = ["TW_WIFI_SSID", "TW_REMOTE_URL", "TW_GH_USER", "TW_PAT"]
            .into_iter()
            .filter(|v| std::env::var(v).map_or(true, |val| val.is_empty()))
            .collect();
        if !missing.is_empty() {
            panic!(
                "git-feature build without publish config: {} unset/empty. \
                 Build through `just build` (dotenv-loads firmware/.env), or \
                 source firmware/.env into this shell. For a no-git editor \
                 build use `just build-light` (drops --features git).",
                missing.join(", ")
            );
        }
    }

    // Pointing rerun-if-changed at a file that never exists forces this
    // script to rerun on every build, keeping BUILD_TIME fresh.
    println!("cargo:rerun-if-changed=.force-build-stamp");
}
