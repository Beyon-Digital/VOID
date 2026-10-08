use std::path::PathBuf;
use std::process::Command;

/// Regenerate FlatBuffers Rust bindings from protocol/void_control.fbs at
/// build time. Requires flatc == REQUIRED_FLATC (tools/protocol-gen.sh keeps
/// the same pin).
fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../..").canonicalize().unwrap();
    let schema = root.join("protocol/void_control.fbs");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed={}", schema.display());

    let required = "25.9.23";
    let version_out = Command::new("flatc")
        .arg("--version")
        .output()
        .expect("flatc not found on PATH — install the pinned flatc (tools/protocol-gen.sh)");
    let version_str = String::from_utf8_lossy(&version_out.stdout);
    let version = version_str.split_whitespace().last().unwrap_or("");
    assert_eq!(
        version, required,
        "flatc {required} required for protocol codegen (found {version})"
    );

    let status = Command::new("flatc")
        .args(["--rust", "-o"])
        .arg(&out_dir)
        .arg(&schema)
        .status()
        .expect("failed to run flatc");
    assert!(status.success(), "flatc codegen failed");
}
