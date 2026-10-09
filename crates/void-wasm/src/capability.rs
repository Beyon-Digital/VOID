//! Capability vocabulary for restricted modules (T97).
//!
//! Two disjoint sets:
//!   * `Capability` — things a module may *declare* and the host will
//!     honor by exposing the matching ABI surface (`params`, `state`,
//!     `render-off`, `midi-transform`, plus the honesty labels
//!     `no-fx`, `generator`).
//!   * `AmbientRequest` — ambient host powers (`fs`, `net`, `env`,
//!     `spawn`, `clock`, `threads`, `random`, `wasi`) that are
//!     NEVER grantable. Declaring one rejects the manifest outright;
//!     importing WASI anyway fails at instantiate because the linker
//!     contains no such imports (defense in depth).

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::{Result, WasmError};

/// Grantable capabilities a module may declare in its manifest.
/// Each maps to a required ABI export set — see `required_exports`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// May expose indexed f64 parameters through the param ABI.
    Params,
    /// May persist an opaque state blob across unload/reload.
    State,
    /// May run `void_render_off` — OFFLINE render only, never the
    /// critical audio path.
    RenderOff,
    /// May run `void_midi_xform` — bounded note-event transform.
    MidiTransform,
    /// Honesty label: declares it is NOT an audio-FX processor.
    /// Informational — no export surface implied.
    NoFx,
    /// Honesty label: generates material rather than transforming it.
    Generator,
}

impl Capability {
    /// All grantable capability names (manifest-level, kebab-case).
    pub fn parse(name: &str) -> Option<Capability> {
        Some(match name {
            "params" => Capability::Params,
            "state" => Capability::State,
            "render-off" => Capability::RenderOff,
            "midi-transform" => Capability::MidiTransform,
            "no-fx" => Capability::NoFx,
            "generator" => Capability::Generator,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::Params => "params",
            Capability::State => "state",
            Capability::RenderOff => "render-off",
            Capability::MidiTransform => "midi-transform",
            Capability::NoFx => "no-fx",
            Capability::Generator => "generator",
        }
    }

    /// ABI exports a module MUST provide when it declares this
    /// capability. Honesty labels require none.
    pub fn required_exports(&self) -> &'static [&'static str] {
        match self {
            Capability::Params => &["void_param_count", "void_param_get", "void_param_set"],
            Capability::State => &["void_state_len", "void_state_save", "void_state_restore"],
            Capability::RenderOff => &["void_render_off"],
            Capability::MidiTransform => &["void_midi_xform"],
            Capability::NoFx | Capability::Generator => &[],
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Capability::parse(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown capability `{s}`")))
    }
}

/// Ambient host powers — NEVER grantable to a restricted module.
/// A manifest may list attempted requests under `ambient_requests`
/// for audit; ANY entry rejects the manifest. The same names are
/// checked against the module's import table at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AmbientRequest {
    /// Filesystem (any WASI fd_* family member).
    Fs,
    /// Network (any WASI sock_* member or socket-ish import).
    Net,
    /// Environment variables.
    Env,
    /// Process spawn / exec.
    Spawn,
    /// Wall/CPU clocks.
    Clock,
    /// Threads / shared memory.
    Threads,
    /// Host randomness (nondeterminism inside restricted modules).
    Random,
    /// Any other WASI import family member.
    Wasi,
}

impl AmbientRequest {
    pub fn parse(name: &str) -> Option<AmbientRequest> {
        Some(match name {
            "fs" => AmbientRequest::Fs,
            "net" => AmbientRequest::Net,
            "env" => AmbientRequest::Env,
            "spawn" => AmbientRequest::Spawn,
            "clock" => AmbientRequest::Clock,
            "threads" => AmbientRequest::Threads,
            "random" => AmbientRequest::Random,
            "wasi" => AmbientRequest::Wasi,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AmbientRequest::Fs => "fs",
            AmbientRequest::Net => "net",
            AmbientRequest::Env => "env",
            AmbientRequest::Spawn => "spawn",
            AmbientRequest::Clock => "clock",
            AmbientRequest::Threads => "threads",
            AmbientRequest::Random => "random",
            AmbientRequest::Wasi => "wasi",
        }
    }
}

impl fmt::Display for AmbientRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Validate a manifest's `ambient_requests` list: ANY entry — even a
/// recognized one — is denied. Unknown entries are denied too (the
/// schema is closed: ambient powers are never grantable).
pub fn reject_ambient(requests: &[String]) -> Result<()> {
    if let Some(first) = requests.first() {
        return Err(WasmError::AmbientDenied(first.clone()));
    }
    Ok(())
}

/// Where a module's entry points may run. `audio_path` is deliberately
/// UNREPRESENTABLE — the schema cannot express RT placement, so no
/// manifest can be silently approved for the critical path (T97:
/// "no unqualified real-time execution").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Pure parameter/state surface — never renders audio.
    ParamSurface,
    /// Offline render/transform jobs under fuel+deadline budgets.
    OfflineRender,
    /// Bounded MIDI event transform, off the audio callback.
    MidiTransform,
}

/// An import entry that passed validation — the only granted host
/// surface today is `void_host.log` (bounded, recorded).
pub const HOST_MODULE: &str = "void_host";
pub const HOST_FN_LOG: &str = "log";

/// Is `module`.`name` an import the host linker provides? Kept
/// deliberately tiny: everything else is denied.
pub fn granted_import(module: &str, name: &str) -> bool {
    module == HOST_MODULE && name == HOST_FN_LOG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambient_never_grantable() {
        for name in [
            "fs", "net", "env", "spawn", "clock", "threads", "random", "wasi",
        ] {
            assert!(AmbientRequest::parse(name).is_some());
        }
        assert!(reject_ambient(&["net".into()]).is_err());
        assert!(reject_ambient(&["fs".into(), "net".into()]).is_err());
        assert!(reject_ambient(&["unknown-power".into()]).is_err());
        assert!(reject_ambient(&[]).is_ok());
    }

    #[test]
    fn only_log_import_granted() {
        assert!(granted_import("void_host", "log"));
        assert!(!granted_import("void_host", "open"));
        assert!(!granted_import("wasi_snapshot_preview1", "fd_read"));
        assert!(!granted_import("env", "system"));
    }
}
