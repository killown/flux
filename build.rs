use std::env;
use std::process::Command;

fn main() {
    glib_build_tools::compile_resources(
        &["data"],
        "data/resources.gresource.xml",
        "flux.gresource",
    );

    // Skip if we are running under cargo-fuzz
    if std::env::var("CARGO_CFG_FUZZING").is_ok() {
        return;
    }

    // Retrieve git commit hash
    let commit_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=FLUX_GIT_HASH={}", commit_hash);

    let out_dir = env::var("OUT_DIR").unwrap_or_default();
    let is_local_profile =
        out_dir.contains("/target/local/") || out_dir.contains(r"\target\local\");

    // Skip automations if we are building a package (e.g., Arch PKGBUILD)
    // Arch's makepkg sets SOURCE_DATE_EPOCH.
    let is_packaging = env::var("SOURCE_DATE_EPOCH").is_ok();

    let sync_assets = env::var("FLUX_SYNC_ASSETS").is_ok();

    if is_local_profile && !is_packaging && sync_assets {
        // Only run formatting and asset syncing during local development release builds
        let _ = Command::new("cargo").args(["fmt"]).status();

        println!("cargo:warning=>>> Local profile build detected. Syncing assets via Makefile...");
        // Pin BIN_PATH to the exact profile this build script is running under,
        // so make install can never fall back to a stale target/release/flux-fm.
        match Command::new("make")
            .arg("install")
            .env("BIN_PATH", "target/local/flux-fm")
            .status()
        {
            Ok(s) if s.success() => println!("cargo:warning=>>> Assets synced successfully."),
            Ok(s) => panic!("make install failed with status: {}", s),
            Err(e) => panic!("make install failed to spawn: {}", e),
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Makefile");
    println!("cargo:rerun-if-changed=scripts/properties.py");
    println!("cargo:rerun-if-changed=flux.desktop");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        if let Some(r) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=.git/{}", r.trim());
        }
    }
}
