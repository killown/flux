use std::process::Command;
use std::env;

fn main() {
    // Only run on release builds
    if env::var("PROFILE").unwrap() == "release" {
        println!("cargo:warning=Attempting to sync file-props to .local/bin...");
        
        let status = Command::new("make")
            .arg("install")
            .status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:warning=file-props installed successfully via Makefile.");
            }
        }
    }

    // Optimization: only rerun if these files actually change
    println!("cargo:rerun-if-changed=Makefile");
    println!("cargo:rerun-if-changed=scripts/properties.py");
}
