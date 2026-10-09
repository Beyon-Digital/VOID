//! ModuleSpec — a verified, loadable unit: manifest + wasm bytes that
//! have passed every static check the host can run before touching the
//! engine. Static checks are ALSO enforced dynamically at
//! instantiate/store level (defense in depth); doing them up front
//! gives precise errors (`ImportDenied`, `MemoryLimitExceeded`)
//! instead of generic linker failures.

use std::path::{Path, PathBuf};
use wasmparser::{Parser, Payload, TypeRef};

use crate::capability::granted_import;
use crate::error::{Result, WasmError};
use crate::manifest::{sha256_hex, ModuleManifest, MANIFEST_FILE, MODULE_FILE, WASM_MAGIC};
use crate::policy::HostPolicy;

/// A fully-verified module package: manifest + wasm bytes.
/// Construct only via `ModuleSpec::from_parts` / `from_dir`.
#[derive(Debug, Clone)]
pub struct ModuleSpec {
    pub manifest: ModuleManifest,
    /// Raw `module.wasm` bytes (sha256-bound to the manifest).
    pub wasm_bytes: Vec<u8>,
    /// Raw manifest bytes verbatim — what `manifest_sha256` binds and
    /// what the registry stores (never a reserialization).
    pub manifest_json: String,
    /// sha256 of the canonical manifest bytes that produced this spec.
    pub manifest_sha256: String,
    /// Imports the module declared (validated ⊆ granted surface).
    pub imports: Vec<(String, String)>,
    /// Defined memory minimum bytes (0 if module defines no memory —
    /// the ABI still requires a `memory` export for buffer passing,
    /// caught separately).
    pub declared_memory_min: usize,
}

impl ModuleSpec {
    /// Verify a (manifest_json, wasm_bytes) pair end to end:
    /// schema → capabilities → ambient deny → limits → sha256 bind →
    /// wasm magic → import surface scan → memory minimum.
    pub fn from_parts(
        manifest_json: &str,
        wasm_bytes: &[u8],
        policy: &HostPolicy,
    ) -> Result<ModuleSpec> {
        let manifest = ModuleManifest::parse(manifest_json, policy)?;
        manifest.verify_wasm(wasm_bytes)?;
        let scan = scan_wasm(wasm_bytes, manifest.limits.max_memory_bytes as usize)?;
        Ok(ModuleSpec {
            manifest,
            manifest_json: manifest_json.to_string(),
            wasm_bytes: wasm_bytes.to_vec(),
            manifest_sha256: sha256_hex(manifest_json.as_bytes()),
            imports: scan.imports,
            declared_memory_min: scan.memory_min_bytes,
        })
    }

    /// Load a package directory containing `manifest.json` + `module.wasm`.
    pub fn from_dir(dir: &Path, policy: &HostPolicy) -> Result<ModuleSpec> {
        let manifest_json = std::fs::read_to_string(dir.join(MANIFEST_FILE))?;
        let wasm_bytes = std::fs::read(dir.join(MODULE_FILE))?;
        Self::from_parts(&manifest_json, &wasm_bytes, policy)
    }
}

pub struct WasmScan {
    pub imports: Vec<(String, String)>,
    pub memory_min_bytes: usize,
}

/// Walk the module's sections: reject non-wasm bytes, deny every
/// import outside the granted `void_host` surface, and check the
/// defined memory's declared minimum against the manifest limit.
/// Does NOT compile — compile safety is wasmtime's job; this is the
/// policy gate that produces precise denials.
pub fn scan_wasm(bytes: &[u8], memory_limit: usize) -> Result<WasmScan> {
    if bytes.len() < 8 || &bytes[0..4] != WASM_MAGIC {
        return Err(WasmError::InvalidModulePackage(
            "not a WebAssembly module (bad magic — native code is never a module)".into(),
        ));
    }
    let mut imports = Vec::new();
    let mut memory_min_bytes = 0usize;
    for payload in Parser::new(0).parse_all(bytes) {
        let payload =
            payload.map_err(|e| WasmError::InvalidModulePackage(format!("malformed wasm: {e}")))?;
        match payload {
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.map_err(|e| {
                        WasmError::InvalidModulePackage(format!("malformed import: {e}"))
                    })?;
                    let module = import.module.to_string();
                    let name = import.name.to_string();
                    // Every import must be inside the granted
                    // surface — WASI families and arbitrary host
                    // names are denied.
                    if !granted_import(&module, &name) {
                        return Err(WasmError::ImportDenied { module, name });
                    }
                    // Only function imports reach the linker
                    // surface; a log import typed as
                    // memory/global/table is an ABI violation.
                    if !matches!(import.ty, TypeRef::Func(..)) {
                        return Err(WasmError::AbiViolation(format!(
                            "import {module}.{name} must be a function"
                        )));
                    }
                    imports.push((module, name));
                }
            }
            Payload::MemorySection(reader) => {
                for mem in reader {
                    let mem = mem.map_err(|e| {
                        WasmError::InvalidModulePackage(format!("malformed memory decl: {e}"))
                    })?;
                    if mem.shared {
                        return Err(WasmError::AmbientDenied(
                            "threads (shared memory is never grantable)".into(),
                        ));
                    }
                    let min_bytes = (mem.initial as usize).saturating_mul(64 * 1024);
                    if min_bytes > memory_limit {
                        return Err(WasmError::MemoryLimitExceeded {
                            requested: min_bytes,
                            limit: memory_limit,
                        });
                    }
                    memory_min_bytes = memory_min_bytes.max(min_bytes);
                }
            }
            _ => {}
        }
    }
    Ok(WasmScan {
        imports,
        memory_min_bytes,
    })
}

/// Path helper for package dirs produced by tests/tooling.
pub fn package_dir(root: &Path, module_id: &str, version: &str) -> PathBuf {
    root.join(module_id).join(version)
}
