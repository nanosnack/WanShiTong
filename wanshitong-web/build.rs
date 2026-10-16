//! Generates the files `statics.rs` serves:
//!
//! - `$OUT_DIR/tailwind.css`: `styles.css` (the topcoat UI theme) and the
//!   Tailwind classes used in `src/`.
//! - `$OUT_DIR/ts/*.js`: the TypeScript in `ts/`, type-checked and compiled.

use std::{env, path::PathBuf, process::Command};

/// The Tailwind CLI release downloaded when `TAILWIND_CLI` is unset. Keep it
/// the version of nixpkgs' `tailwindcss_4`, so the CSS is the same either way.
const TAILWIND_VERSION: &str = "4.3.3";

fn main() {
    // Printing any rerun-if directive replaces Cargo's default of rerunning on
    // every change in the package, so everything the outputs depend on is
    // listed: the theme, the sources Tailwind scans for classes, the
    // TypeScript, and the CLIs.
    println!("cargo:rerun-if-changed=styles.css");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=ts");
    // The Nix dev shell and package name the Tailwind and TypeScript CLIs in
    // the environment. Without them, Tailwind is downloaded and `tsc` is
    // looked up on PATH, so a plain `cargo build` works too.
    println!("cargo:rerun-if-env-changed=TAILWIND_CLI");
    println!("cargo:rerun-if-env-changed=TSC");

    let tailwind = topcoat::tailwind::BuildConfig::new().input("styles.css");
    let tailwind = if env::var_os("TAILWIND_CLI").is_some() {
        tailwind.executable_env("TAILWIND_CLI")
    } else {
        tailwind.version_checksum(TAILWIND_VERSION, tailwind_checksum())
    };
    tailwind.render().unwrap();

    compile_typescript();
}

/// The sha256 of the Tailwind CLI that topcoat downloads for this host, from
/// the release's `sha256sums.txt`.
fn tailwind_checksum() -> &'static str {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => {
            "sha256:dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a"
        }
        ("linux", "aarch64") => {
            "sha256:55fd0b241214eff3de1e8ee4f22796662f2d2e7a49bcfca7477cfd0bac398195"
        }
        ("macos", "x86_64") => {
            "sha256:7922e0953f2110c05976e3bf58f14e643d90427575e766b7d433f5f80cbee7e1"
        }
        ("macos", "aarch64") => {
            "sha256:cdf646702987a743464dff4d9c60fd4480d1c1e73dd819a9a67f1078815dce9d"
        }
        ("windows", "x86_64") => {
            "sha256:e0e260ce048014e9268f6237ff18f8ccf02cef521cbd0ae04e82c2cdf7aa3955"
        }
        (os, arch) => panic!(
            "Tailwind {TAILWIND_VERSION} has no CLI for {os} {arch}; \
             set TAILWIND_CLI to a Tailwind CLI executable"
        ),
    }
}

fn compile_typescript() {
    let tsc = env::var("TSC").unwrap_or_else(|_| "tsc".into());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap()).join("ts");
    let status = Command::new(&tsc)
        .args(["--project", "ts/tsconfig.json", "--outDir"])
        .arg(&out_dir)
        .status()
        .unwrap_or_else(|e| {
            panic!("running the TypeScript compiler `{tsc}` (set TSC, or put tsc on PATH): {e}")
        });
    assert!(status.success(), "the TypeScript in ts/ failed to compile");
}
