//! Host policy ceilings — the non-negotiable caps a manifest may
//! declare *below* but never *above*. A module asking for more than
//! the policy is rejected at manifest validation (not silently
//! clamped — clamping would lie about what was granted).

use serde::{Deserialize, Serialize};

use crate::error::{Result, WasmError};

/// One epoch tick in milliseconds. The runtime's epoch ticker advances
/// the engine epoch this often; deadlines and cancels resolve on this
/// granularity.
pub const EPOCH_TICK_MS: u64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostPolicy {
    /// Hard ceiling on linear-memory grant, bytes (per instance).
    pub max_memory_bytes: usize,
    /// Hard ceiling on fuel budget per guest call.
    pub max_fuel: u64,
    /// Hard ceiling on per-call epoch deadline, milliseconds.
    pub max_deadline_ms: u64,
    /// Hard ceiling on a module's persisted state blob, bytes.
    pub max_state_bytes: usize,
    /// Hard ceiling on one render_off/midi_xform output buffer, bytes.
    pub max_out_bytes: usize,
    /// Hard ceiling on indexed parameters a module may expose.
    pub max_params: u32,
    /// Maximum bytes retained from `void_host.log` per instance.
    pub max_log_bytes: usize,
}

impl Default for HostPolicy {
    /// Conservative initial defaults (tunable engineering defaults per
    /// CONTRACTS.md §2 — not achieved benchmarks). 64 MiB memory,
    /// 50M fuel units, 5 s call deadline, 1 MiB state, 16 MiB output.
    fn default() -> Self {
        HostPolicy {
            max_memory_bytes: 64 * 1024 * 1024,
            max_fuel: 50_000_000,
            max_deadline_ms: 5_000,
            max_state_bytes: 1024 * 1024,
            max_out_bytes: 16 * 1024 * 1024,
            max_params: 1024,
            max_log_bytes: 64 * 1024,
        }
    }
}

/// Per-module declared limits. All fields are required: a module must
/// name its own budget inside the policy envelope so abuse is visible
/// in the manifest itself. Serialized with string-int64 fields per the
/// repo's package-boundary convention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredLimits {
    #[serde(with = "crate::strnum::u64str")]
    pub max_memory_bytes: u64,
    #[serde(with = "crate::strnum::u64str")]
    pub max_fuel: u64,
    #[serde(with = "crate::strnum::u64str")]
    pub deadline_ms: u64,
    #[serde(with = "crate::strnum::u64str")]
    pub max_state_bytes: u64,
    #[serde(with = "crate::strnum::u64str")]
    pub max_out_bytes: u64,
    #[serde(with = "crate::strnum::u64str")]
    pub max_params: u64,
}

impl DeclaredLimits {
    /// Validate the declaration against the policy ceiling: every
    /// limit must be positive and ≤ policy. Failing this rejects the
    /// manifest — never grants more than declared, never grants above
    /// policy.
    pub fn validate_against(&self, policy: &HostPolicy) -> Result<()> {
        let fail = |what: &str, declared: u64, ceiling: u64| -> WasmError {
            WasmError::InvalidManifest(format!(
                "limits.{what} declared {declared} exceeds policy ceiling {ceiling}"
            ))
        };
        let nonzero = |what: &str| -> WasmError {
            WasmError::InvalidManifest(format!("limits.{what} must be > 0"))
        };
        if self.max_memory_bytes == 0 {
            return Err(nonzero("max_memory_bytes"));
        }
        if self.max_memory_bytes > policy.max_memory_bytes as u64 {
            return Err(fail(
                "max_memory_bytes",
                self.max_memory_bytes,
                policy.max_memory_bytes as u64,
            ));
        }
        if self.max_fuel == 0 {
            return Err(nonzero("max_fuel"));
        }
        if self.max_fuel > policy.max_fuel {
            return Err(fail("max_fuel", self.max_fuel, policy.max_fuel));
        }
        if self.deadline_ms == 0 {
            return Err(nonzero("deadline_ms"));
        }
        if self.deadline_ms > policy.max_deadline_ms {
            return Err(fail(
                "deadline_ms",
                self.deadline_ms,
                policy.max_deadline_ms,
            ));
        }
        if self.max_state_bytes > policy.max_state_bytes as u64 {
            return Err(fail(
                "max_state_bytes",
                self.max_state_bytes,
                policy.max_state_bytes as u64,
            ));
        }
        if self.max_out_bytes == 0 {
            return Err(nonzero("max_out_bytes"));
        }
        if self.max_out_bytes > policy.max_out_bytes as u64 {
            return Err(fail(
                "max_out_bytes",
                self.max_out_bytes,
                policy.max_out_bytes as u64,
            ));
        }
        if self.max_params > policy.max_params as u64 {
            return Err(fail(
                "max_params",
                self.max_params,
                policy.max_params as u64,
            ));
        }
        Ok(())
    }
}
