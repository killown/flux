use std::env;
use std::process::Command;

fn main() {
    // Skip if we are running under cargo-fuzz
    if std::env::var("CARGO_CFG_FUZZING").is_ok() {
        return;
    }

    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    // Skip automations if we are building a package (e.g., Arch PKGBUILD)
    // Arch's makepkg sets SOURCE_DATE_EPOCH.
    let is_packaging = env::var("SOURCE_DATE_EPOCH").is_ok();

    if profile == "release" && !is_packaging {
        // Only run formatting and asset syncing during local development release builds
        let _ = Command::new("cargo").args(["fmt"]).status();

        println!("cargo:warning=>>> Release build detected. Syncing assets via Makefile...");
        let status = Command::new("make").arg("install").status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:warning=>>> Assets synced successfully.");
            } else {
                println!("cargo:warning=>>> Makefile failed with status: {}", s);
            }
        }
    }

    println!("cargo:rerun-if-changed=Makefile");
    println!("cargo:rerun-if-changed=scripts/properties.py");
    println!("cargo:rerun-if-changed=flux.desktop");
}
