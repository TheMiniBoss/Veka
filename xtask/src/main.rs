//! Tâches de maintenance du dépôt : `cargo xtask <tâche>`.
//!
//! - `check-core` : vérifie que le cœur de Veka ne dépend d'aucune API système et qu'il compile
//!   pour WebAssembly (règle n° 1 de `Docs/VEKA_ETAPE1.md` §3).

use std::process::{Command, ExitCode};

/// Les crates du cœur : elles ne doivent jamais voir l'OS.
const CORE: &[&str] = &["veka-foundation", "veka-graphics", "veka-text", "veka-ui"];

/// Dépendances interdites dans le cœur (fenêtres, OS, GPU).
const FORBIDDEN: &[&str] = &[
    "windows",
    "windows-sys",
    "winapi",
    "objc2",
    "objc",
    "cocoa",
    "core-foundation",
    "core-text",
    "dwrote",
    "fontconfig",
    "yeslogic-fontconfig-sys",
    "winit",
    "wgpu",
    "libc",
    "web-sys",
    "js-sys",
];

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("check-core") => check_core(),
        _ => {
            eprintln!("usage : cargo xtask check-core");
            ExitCode::FAILURE
        }
    }
}

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
}

fn check_core() -> ExitCode {
    let mut ok = true;
    for krate in CORE {
        // Toutes les cibles : une dépendance propre à Windows ou macOS compte aussi.
        let out = cargo()
            .args([
                "tree", "-q", "-p", krate, "-e", "normal", "--target", "all", "--prefix", "none",
            ])
            .args(["--format", "{p}"])
            .output()
            .expect("cargo tree");
        if !out.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&out.stderr));
            return ExitCode::FAILURE;
        }
        let tree = String::from_utf8_lossy(&out.stdout);
        for line in tree.lines() {
            let name = line.split_whitespace().next().unwrap_or("");
            if FORBIDDEN.contains(&name) {
                eprintln!("✗ {krate} dépend de `{name}` : le cœur ne doit pas voir l'OS");
                ok = false;
            }
        }
    }
    let mut wasm = cargo();
    wasm.args(["check", "-q", "--target", "wasm32-unknown-unknown"]);
    for krate in CORE {
        wasm.args(["-p", krate]);
    }
    if !wasm.status().expect("cargo check").success() {
        eprintln!("✗ le cœur ne compile pas pour WebAssembly");
        ok = false;
    }
    if ok {
        println!("✓ cœur sans dépendance système, compile pour WebAssembly");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
