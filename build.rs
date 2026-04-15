use std::env;
use std::process::Command;

fn main() {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    if profile == "release" {
        // Automatically fix formatting before continuing
        let fmt_status = Command::new("cargo").args(["fmt"]).status();

        if let Ok(s) = fmt_status {
            if !s.success() {
                println!("cargo:warning=>>> cargo fmt failed to run.");
            }
        }

        println!("cargo:warning=>>> Release build detected. Syncing assets via Makefile...");
        let status = Command::new("make").arg("install").status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=>>> Assets synced successfully.");
            }
            Ok(s) => {
                println!("cargo:warning=>>> Makefile failed with status: {}", s);
            }
            Err(e) => {
                println!("cargo:warning=>>> Failed to run Makefile: {}", e);
            }
        }
    }

    println!("cargo:rerun-if-changed=Makefile");
    println!("cargo:rerun-if-changed=scripts/properties.py");
    println!("cargo:rerun-if-changed=flux.desktop");
}
