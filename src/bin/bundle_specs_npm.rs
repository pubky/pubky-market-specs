use std::env;
use std::io;
use std::process::{Command, ExitStatus};

// If the process hangs, try `cargo clean` to remove all locks.

fn main() {
    println!("Building wasm for pubky-market-specs...");

    let built = build_wasm("nodejs").expect("wasm-pack could not be started");
    if !built.success() {
        std::process::exit(1);
    }
    let patched = patch().expect("node could not be started");
    if !patched.success() {
        std::process::exit(1);
    }
    println!("pubky-market-specs JS binding package built successfully");
}

fn build_wasm(target: &str) -> io::Result<ExitStatus> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let output = Command::new("wasm-pack")
        .args([
            "build",
            &manifest_dir,
            "--release",
            "--target",
            target,
            "--out-dir",
            &format!("pkg/{}", target),
        ])
        .output()?;

    if !output.status.success() {
        eprintln!(
            "wasm-pack failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.status)
}

fn patch() -> io::Result<ExitStatus> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    println!("Applying {manifest_dir}/src/bin/patch.mjs to generate isomorphic code for web and nodejs...");

    let output = Command::new("node")
        .args([format!("{manifest_dir}/src/bin/patch.mjs")])
        .output()?;

    if !output.status.success() {
        eprintln!(
            "patch.mjs failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(output.status)
}
