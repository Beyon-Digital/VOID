//! Native/SDK integration gates (T98) — ARA and native-plugin export
//! are rights-gated features: the gate cannot flip to `Qualified`
//! without evidence (SDK version, licence/rights record, commit). The
//! honest Linux-verifiable state today is `Gated` — see
//! docs/wasm/adr/ for the rationale.

use serde::{Deserialize, Serialize};

use crate::error::{Result, WasmError};

/// The SDK-gated integrations tracked by this lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatedIntegration {
    /// ARA (Audio Random Access) editor integration — Celemony SDK.
    Ara,
    /// Exporting a module/patch as a native plugin (VST3/CLAP/AU) —
    /// requires each target SDK's build+distribution rights.
    PluginExport,
}

impl GatedIntegration {
    pub fn as_str(&self) -> &'static str {
        match self {
            GatedIntegration::Ara => "ara",
            GatedIntegration::PluginExport => "plugin_export",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GateStatus {
    /// Closed: rights/SDK qualification unresolved. `reason` names the
    /// blocking item. Any request for the feature returns
    /// `IntegrationGated`.
    Gated { reason: String },
    /// Open: evidence recorded (sdk id+version, rights reference,
    /// qualifying commit). Empty evidence can never qualify.
    Qualified {
        sdk: String,
        rights_ref: String,
        commit: String,
    },
}

/// One gate's recorded state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateRecord {
    pub integration: GatedIntegration,
    pub status: GateStatus,
}

/// The lane's gate ledger — constructed, never silently flipped.
pub fn gate_register() -> Vec<GateRecord> {
    vec![
        GateRecord {
            integration: GatedIntegration::Ara,
            status: GateStatus::Gated {
                reason: "ARA SDK development/distribution rights unresolved — \
                         Celemony licence required before evaluation (docs/wasm/adr/0001)"
                    .into(),
            },
        },
        GateRecord {
            integration: GatedIntegration::PluginExport,
            status: GateStatus::Gated {
                reason:
                    "native plugin export would emit arbitrary signed binaries — \
                         blocked pending per-format SDK rights + signer policy (docs/wasm/adr/0002)"
                        .into(),
            },
        },
    ]
}

/// Try to qualify a gate — real evidence required. This is the ONLY
/// path from Gated to Qualified, and it refuses empty evidence.
pub fn try_qualify(
    integration: GatedIntegration,
    sdk: &str,
    rights_ref: &str,
    commit: &str,
) -> Result<GateRecord> {
    if sdk.trim().is_empty() || rights_ref.trim().is_empty() || commit.trim().is_empty() {
        return Err(WasmError::IntegrationGated(
            "qualification requires sdk, rights_ref and commit — none may be empty",
        ));
    }
    Ok(GateRecord {
        integration,
        status: GateStatus::Qualified {
            sdk: sdk.to_string(),
            rights_ref: rights_ref.to_string(),
            commit: commit.to_string(),
        },
    })
}

/// Ask whether an integration may be used. Gated → typed denial.
pub fn require_open(record: &GateRecord) -> Result<()> {
    match &record.status {
        GateStatus::Qualified { .. } => Ok(()),
        GateStatus::Gated { .. } => Err(WasmError::IntegrationGated(
            // Static reason keeps the error's lifetime 'static; the
            // detail text lives in the record itself.
            match record.integration {
                GatedIntegration::Ara => "ARA integration gated — SDK rights unresolved",
                GatedIntegration::PluginExport => {
                    "plugin export gated — per-format SDK/signer rights unresolved"
                }
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_start_closed() {
        for rec in gate_register() {
            assert!(matches!(rec.status, GateStatus::Gated { .. }));
            assert!(require_open(&rec).is_err());
        }
    }

    #[test]
    fn qualification_requires_evidence() {
        assert!(try_qualify(GatedIntegration::Ara, "", "", "").is_err());
        assert!(try_qualify(GatedIntegration::Ara, "ARA SDK 2.2", "", "abc").is_err());
        let ok = try_qualify(
            GatedIntegration::Ara,
            "ARA SDK 2.2",
            "licence-rec-1",
            "deadbeef",
        )
        .unwrap();
        assert!(require_open(&ok).is_ok());
    }
}
