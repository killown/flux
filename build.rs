use std::process::Command;
use std::env;

fn main() {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    if profile == "release" {
        println!("cargo:warning=>>> Release build detected. Syncing assets via Makefile...");
        let status = Command::new("make")
            .arg("install")
            .status();

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
