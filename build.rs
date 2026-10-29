use std::env;
use std::path::Path;
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

    let is_local_profile = env::var_os("OUT_DIR").is_some_and(|dir| {
        Path::new(&dir)
            .ancestors()
            .nth(3)
            .and_then(Path::file_name)
            .is_some_and(|name| name == "local")
    });

    // Skip automations if we are building a package (e.g., Arch PKGBUILD)
    // Arch's makepkg sets SOURCE_DATE_EPOCH.
    let is_packaging = env::var("SOURCE_DATE_EPOCH").is_ok();

    let sync_assets = env::var("FLUX_SYNC_ASSETS").is_ok();

    if is_local_profile && !is_packaging && sync_assets {
        // Only run formatting and asset syncing during local development release builds
        let _ = Command::new("cargo").args(["fmt"]).status();

        println!("cargo:warning=>>> Local profile build detected. Syncing assets via Makefile...");
        match Command::new("make").arg("install-assets").status() {
            Ok(s) if s.success() => println!("cargo:warning=>>> Assets synced successfully."),
            Ok(s) => panic!("make install-assets failed with status: {}", s),
            Err(e) => panic!("make install-assets failed to spawn: {}", e),
        }
    }

    println!("cargo:rerun-if-env-changed=FLUX_SYNC_ASSETS");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Makefile");
    println!("cargo:rerun-if-changed=scripts");
    println!("cargo:rerun-if-changed=flux.desktop.in");
    let mut git_files = vec![".git/HEAD".to_string(), ".git/packed-refs".to_string()];
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        if let Some(r) = head.strip_prefix("ref: ") {
            git_files.push(format!(".git/{}", r.trim()));
        }
    }
    for path in git_files.iter().filter(|p| Path::new(p.as_str()).exists()) {
        println!("cargo:rerun-if-changed={path}");
    }
}
