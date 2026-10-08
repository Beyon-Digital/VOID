//! Missing-plugin preserved state (W20, T77 "missing plugins" leg).
//!
//! When a project references a plugin that is not installed, VOID keeps
//! its descriptor + opaque state blob verbatim inside the project — the
//! slot survives save/reopen and is rehydrated automatically when the
//! plugin reappears. Nothing is silently substituted: a missing plugin
//! renders as missing, with its state retained byte-for-byte.
//!
//! The store is a JSON document (`void-missing-plugins/1`) the coordinator
//! can persist inside checkpoint `app-state.json`; blobs are hex-encoded
//! (no new codec dep — sha256 hex already authenticates them).

use crate::registry::PluginDescriptor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreservedPluginState {
    /// Slot identity on the VOID side.
    pub instance_id: String,
    pub track_id: String,
    pub slot: i32,
    /// The descriptor exactly as last seen — name/vendor/version retained
    /// for display while the binary is absent.
    pub descriptor: PluginDescriptor,
    /// Opaque state blob, hex-encoded. `None` = the plugin never published
    /// state (legal — the slot is still preserved).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_hex: Option<String>,
    /// sha256 of the decoded blob (always present when state_hex is).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_sha256: Option<String>,
    /// Snapshot of descriptor.state_format_version at preserve time —
    /// matching is strict-equality when both sides declare one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_format_version: Option<String>,
    /// Whether this record's slot was rehydrated after the plugin
    /// reappeared. Resolved records are kept for audit/history.
    #[serde(default)]
    pub resolved: bool,
}

/// Why rehydration cannot proceed — typed so the UI can badge it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum StillMissing {
    /// No descriptor in the environment matches format+uid.
    NotInstalled,
    /// Found a same-uid build but its declared state format differs —
    /// a real host would also refuse; we keep the record.
    StateVersionMismatch { expected: String, found: String },
    /// Found a same-uid build with no binary for this host arch.
    ArchMismatch { host: String },
}

/// What `resolve` produces for one preserved record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Plugin reappeared and can consume the preserved blob.
    Rehydrate {
        /// The live descriptor that matched.
        descriptor: PluginDescriptor,
        /// The blob to feed it (may be empty when none was stored).
        state: Option<Vec<u8>>,
    },
    StillMissing(StillMissing),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingPluginStore {
    /// `void-missing-plugins/1`.
    pub format: String,
    #[serde(default)]
    pub records: Vec<PreservedPluginState>,
}

impl Default for MissingPluginStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MissingPluginStore {
    pub const FORMAT: &'static str = "void-missing-plugins/1";

    pub fn new() -> Self {
        Self {
            format: Self::FORMAT.to_string(),
            records: Vec::new(),
        }
    }

    /// Record a slot's descriptor+state. Idempotent per instance_id —
    /// re-preserving replaces the record (the newer blob wins).
    pub fn preserve(&mut self, record: PreservedPluginState) {
        self.records.retain(|r| r.instance_id != record.instance_id);
        self.records.push(record);
    }

    pub fn record_for(&self, instance_id: &str) -> Option<&PreservedPluginState> {
        self.records.iter().find(|r| r.instance_id == instance_id)
    }

    /// All unresolved records — the "missing plugins" badge list.
    pub fn missing(&self) -> impl Iterator<Item = &PreservedPluginState> {
        self.records.iter().filter(|r| !r.resolved)
    }

    /// Try to rehydrate `instance_id` against the builds actually present.
    /// `available` is the installed/registered set (e.g. registry rows or
    /// a fresh scan). A fake descriptor works — no plugin code runs here.
    pub fn resolve(&self, instance_id: &str, available: &[PluginDescriptor]) -> Option<Resolution> {
        let record = self.record_for(instance_id)?;
        if record.resolved {
            return None;
        }
        let candidates: Vec<&PluginDescriptor> = available
            .iter()
            .filter(|d| {
                d.format == record.descriptor.format && d.plugin_uid == record.descriptor.plugin_uid
            })
            .collect();
        if candidates.is_empty() {
            return Some(Resolution::StillMissing(StillMissing::NotInstalled));
        }
        // Prefer a state-format match; fall back to undeclared.
        for c in &candidates {
            if !c.can_load_state_of(&record.descriptor) {
                let expected = record
                    .state_format_version
                    .clone()
                    .or_else(|| record.descriptor.state_format_version.clone())
                    .unwrap_or_else(|| "unspecified".into());
                let found = c
                    .state_format_version
                    .clone()
                    .unwrap_or_else(|| "unspecified".into());
                return Some(Resolution::StillMissing(
                    StillMissing::StateVersionMismatch { expected, found },
                ));
            }
        }
        // `host_arch` is supplied by the caller via available descriptors
        // already being platform-filtered; an explicit arch check happens
        // in `resolve_for_arch`.
        let descriptor = (*candidates[0]).clone();
        let state = record
            .state_hex
            .as_ref()
            .map(|h| decode_hex(h))
            .transpose()
            .ok()?;
        Some(Resolution::Rehydrate { descriptor, state })
    }

    /// Same as `resolve` plus an explicit host-arch check (e.g. "arm64").
    pub fn resolve_for_arch(
        &self,
        instance_id: &str,
        available: &[PluginDescriptor],
        host_arch: &str,
    ) -> Option<Resolution> {
        let record = self.record_for(instance_id)?;
        let arch_ok: Vec<PluginDescriptor> = available
            .iter()
            .filter(|d| d.supports_arch(host_arch))
            .cloned()
            .collect();
        if arch_ok.is_empty()
            && available.iter().any(|d| {
                d.format == record.descriptor.format && d.plugin_uid == record.descriptor.plugin_uid
            })
        {
            return Some(Resolution::StillMissing(StillMissing::ArchMismatch {
                host: host_arch.to_string(),
            }));
        }
        self.resolve(instance_id, &arch_ok)
    }

    /// Mark a record consumed after a successful rehydrate.
    pub fn mark_resolved(&mut self, instance_id: &str) {
        if let Some(r) = self
            .records
            .iter_mut()
            .find(|r| r.instance_id == instance_id)
        {
            r.resolved = true;
        }
    }

    pub fn to_json(&self) -> crate::error::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn parse(bytes: &[u8]) -> crate::error::Result<Self> {
        let store: Self = serde_json::from_slice(bytes).map_err(|e| {
            crate::error::ExchangeError::Malformed(format!("missing-plugin store json: {e}"))
        })?;
        if store.format != Self::FORMAT {
            return Err(crate::error::ExchangeError::Unsupported(format!(
                "missing-plugin store format {:?} (expected {:?})",
                store.format,
                Self::FORMAT
            )));
        }
        Ok(store)
    }
}

pub fn decode_hex(s: &str) -> crate::error::Result<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(crate::error::ExchangeError::Malformed("state hex".into()));
    }
    Ok((0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0))
        .collect())
}

pub fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{:02x}", b);
    }
    out
}
