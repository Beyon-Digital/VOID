//! [`SpatialGate`] — the honest availability verdict for every spatial
//! output path (W26 done-only-when: "unsupported outputs unavailable
//! with a reason").
//!
//! Every `check()` answers exactly one of:
//! - [`GateVerdict::Available`] — and it *carries the validator record*
//!   that makes it available (no record → not available);
//! - [`GateVerdict::Unavailable`] — with a typed
//!   [`SpatialUnavailable`] reason.
//!
//! There is no stereo-as-proxy path: requesting an output the gate
//! cannot prove returns `Unavailable`, never a narrower approximation
//! relabelled as the target. Licensed/specification paths (Atmos ADM
//! BWF, Atmos MP4) are `LicensedRendererRequired` /
//! `ApprovedValidatorRequired` until a qualified validator is
//! registered; head-tracked binaural is `HeadTrackingUnavailable`
//! until hardware is declared; channel-bus paths additionally require
//! the monitor config to actually drive the layout.

use serde::{Deserialize, Serialize};

use crate::layout::SpatialLayout;
use crate::monitoring::{MonitorVerdict, MonitoringConfig};
use crate::objects::ObjectContainerSpec;

/// A spatial output path a caller may request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SpatialOutputPath {
    /// Channel-based output bus in a declared layout.
    ChannelBus { layout: SpatialLayout },
    /// Binaural headphone monitoring; `head_tracked` requires
    /// declared hardware.
    Binaural { head_tracked: bool },
    /// Dolby Atmos ADM BWF deliverable (bed + objects).
    AtmosAdmBwf { container: ObjectContainerSpec },
    /// Dolby Atmos MP4 deliverable — separate codec/container gate
    /// from ADM BWF (OUT-08: codec/rendering permissions are distinct).
    AtmosMp4 { container: ObjectContainerSpec },
    /// Bed+object immersive render to a declared immersive speaker set
    /// (renderer-side, not an encoded deliverable).
    BedPlusObjects { container: ObjectContainerSpec },
}

/// A validator registration — the recorded proof a path is genuinely
/// exercised. `SelfCheck` validators are honest about their scope: a
/// model-level self-check validates metadata/layout legality, never
/// bitstream conformance. External/specification conformance requires
/// `kind = Approved` with a real validator identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorRecord {
    /// Validator identity, e.g. "void-spatial/layout-selfcheck" or a
    /// named external tool + version.
    pub id: String,
    pub kind: ValidatorKind,
    /// Path scopes this record covers, e.g. "channelBus:7.1.4",
    /// "binaural", "atmos-adm-bwf". An empty scope list covers nothing.
    pub scopes: Vec<String>,
    /// Where the validator's evidence lives (artifact hash/path),
    /// required for `Approved` validators.
    pub evidence_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidatorKind {
    /// Model-level self-check produced by this crate — honest about
    /// being internal validation, not external/spec conformance.
    SelfCheck,
    /// An approved external validator (licensed renderer, conformance
    /// suite) — must carry `evidence_ref`.
    Approved,
}

/// Typed reason a spatial path is unavailable — the W26/T93 answer
/// instead of a fake "supported" flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum SpatialUnavailable {
    /// No monitoring configuration declared at all.
    NoMonitoringConfig,
    /// Monitor config rejected `layout` — carries the monitor verdict
    /// reason (no fallback / unreachable fallback).
    MonitorUnreachable {
        layout: SpatialLayout,
        reason: String,
    },
    /// Monitor config failed validation (uncalibrated speakers,
    /// illegal fallback) — the config is unusable, not approximated.
    MonitoringInvalid { errors: Vec<String> },
    /// No validator record covers this path — nothing is "available"
    /// on faith.
    ApprovedValidatorRequired { path: String },
    /// Only a SelfCheck validator exists but the path requires an
    /// approved external validator (licensed deliverable formats).
    ExternalValidatorRequired { path: String },
    /// Licensed renderer/toolchain required (Dolby Atmos) — gated on
    /// actual specs/licences per W26 §Implement-2.
    LicensedRendererRequired { what: String },
    /// Head-tracking hardware/driver not declared for this build.
    HeadTrackingUnavailable,
    /// The container spec itself is invalid — errors enumerated.
    InvalidContainer { errors: Vec<String> },
    /// Platform doesn't provide the path (e.g. no declared endpoint).
    PlatformUnsupported { platform: String, what: String },
}

/// The verdict — Available *with its validator record* or
/// Unavailable *with a typed reason*.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum GateVerdict {
    Available {
        validator: ValidatorRecord,
        /// How monitoring resolves for this path (e.g. "direct",
        /// "downmix to 5.1", "binaural") — recorded, not assumed.
        monitor_route: String,
    },
    Unavailable {
        reason: SpatialUnavailable,
    },
}

impl GateVerdict {
    pub fn is_available(&self) -> bool {
        matches!(self, GateVerdict::Available { .. })
    }
}

/// The gate environment: what this machine/build has declared.
#[derive(Debug, Clone, Default)]
pub struct GateEnvironment {
    /// Declared monitoring configuration (validated before use).
    pub monitoring: Option<MonitoringConfig>,
    /// Registered validator records (self-checks and approved).
    pub validators: Vec<ValidatorRecord>,
    /// Head-tracking hardware is declared and probed.
    pub head_tracking_declared: bool,
    /// Platform id (e.g. "linux-x86_64").
    pub platform: String,
}

/// Scope strings a path requires a validator to cover.
fn required_scope(path: &SpatialOutputPath) -> &'static str {
    match path {
        SpatialOutputPath::ChannelBus { layout } => match layout {
            SpatialLayout::Mono => "channelBus:mono",
            SpatialLayout::Stereo => "channelBus:stereo",
            SpatialLayout::Quad => "channelBus:quad",
            SpatialLayout::Surround51 => "channelBus:5.1",
            SpatialLayout::Surround71 => "channelBus:7.1",
            SpatialLayout::Surround712 => "channelBus:7.1.2",
            SpatialLayout::Surround714 => "channelBus:7.1.4",
            SpatialLayout::Surround914 => "channelBus:9.1.4",
            SpatialLayout::Surround222 => "channelBus:22.2",
        },
        SpatialOutputPath::Binaural { .. } => "binaural",
        SpatialOutputPath::AtmosAdmBwf { .. } => "atmos-adm-bwf",
        SpatialOutputPath::AtmosMp4 { .. } => "atmos-mp4",
        SpatialOutputPath::BedPlusObjects { .. } => "bed+objects",
    }
}

/// Whether a path requires an `Approved` (external) validator —
/// licensed/spec deliverables can never ride on a self-check.
fn requires_approved(path: &SpatialOutputPath) -> bool {
    matches!(
        path,
        SpatialOutputPath::AtmosAdmBwf { .. } | SpatialOutputPath::AtmosMp4 { .. }
    )
}

pub struct SpatialGate {
    env: GateEnvironment,
}

impl SpatialGate {
    pub fn new(env: GateEnvironment) -> Self {
        Self { env }
    }

    /// The self-check record this crate can honestly issue — covers
    /// metadata/layout legality only, never external conformance.
    pub fn layout_selfcheck() -> ValidatorRecord {
        ValidatorRecord {
            id: "void-spatial/layout-selfcheck".into(),
            kind: ValidatorKind::SelfCheck,
            scopes: vec![
                "channelBus:mono".into(),
                "channelBus:stereo".into(),
                "channelBus:quad".into(),
                "channelBus:5.1".into(),
                "channelBus:7.1".into(),
                "channelBus:7.1.2".into(),
                "channelBus:7.1.4".into(),
                "channelBus:9.1.4".into(),
                "channelBus:22.2".into(),
                "binaural".into(),
                "bed+objects".into(),
            ],
            evidence_ref: None,
        }
    }

    fn find_validator(&self, scope: &str, approved: bool) -> Option<&ValidatorRecord> {
        self.env.validators.iter().find(|v| {
            v.scopes.iter().any(|s| s == scope)
                && (!approved || v.kind == ValidatorKind::Approved)
                && (v.kind == ValidatorKind::SelfCheck || v.evidence_ref.is_some())
        })
    }

    /// Check one output path. Pure and total: always a verdict.
    pub fn check(&self, path: &SpatialOutputPath) -> GateVerdict {
        // 1. Path shape validation first — an invalid request gets an
        //    InvalidContainer verdict, not an availability guess.
        if let Some(container) = container_of(path) {
            if let Err(errors) = container.validate() {
                return GateVerdict::Unavailable {
                    reason: SpatialUnavailable::InvalidContainer {
                        errors: errors.iter().map(|e| e.to_string()).collect(),
                    },
                };
            }
        }

        let scope = required_scope(path);
        let path_name = format!("{path:?}");

        // 2. Licensed/spec deliverables gate on licence + external
        //    validator before anything else.
        if matches!(path, SpatialOutputPath::AtmosAdmBwf { .. }) {
            // The licence question precedes validation: unlicensed
            // tooling never produces an Atmos file at all.
            match self.find_validator(scope, true) {
                None => {
                    return GateVerdict::Unavailable {
                        reason: SpatialUnavailable::LicensedRendererRequired {
                            what: "Dolby Atmos ADM BWF encode + conformance validator".into(),
                        },
                    };
                }
                Some(v) => {
                    return GateVerdict::Available {
                        validator: v.clone(),
                        monitor_route: "deliverable (no monitoring required)".into(),
                    };
                }
            }
        }
        if matches!(path, SpatialOutputPath::AtmosMp4 { .. }) {
            match self.find_validator(scope, true) {
                None => {
                    return GateVerdict::Unavailable {
                        reason: SpatialUnavailable::LicensedRendererRequired {
                            what: "Dolby Atmos MP4/IMM encode + QC validator".into(),
                        },
                    };
                }
                Some(v) => {
                    return GateVerdict::Available {
                        validator: v.clone(),
                        monitor_route: "deliverable (no monitoring required)".into(),
                    };
                }
            }
        }

        // 3. Head-tracking requires declared hardware.
        if let SpatialOutputPath::Binaural { head_tracked: true } = path {
            if !self.env.head_tracking_declared {
                return GateVerdict::Unavailable {
                    reason: SpatialUnavailable::HeadTrackingUnavailable,
                };
            }
        }

        // 4. Monitoring must resolve the path's content.
        let monitor_route = match self.resolve_monitoring(path) {
            Ok(route) => route,
            Err(reason) => return GateVerdict::Unavailable { reason },
        };

        // 5. A validator record covering the scope must exist —
        //    approved for licensed paths (handled above), self-check
        //    or better otherwise.
        match (
            self.find_validator(scope, requires_approved(path)),
            requires_approved(path),
        ) {
            (None, true) => GateVerdict::Unavailable {
                reason: SpatialUnavailable::ExternalValidatorRequired { path: path_name },
            },
            (None, false) => GateVerdict::Unavailable {
                reason: SpatialUnavailable::ApprovedValidatorRequired { path: scope.into() },
            },
            (Some(v), _) => GateVerdict::Available {
                validator: v.clone(),
                monitor_route,
            },
        }
    }

    /// Resolve how the requested content would be monitored: binaural
    /// paths render to headphones directly; bus/object paths need a
    /// monitor config that can drive the content layout (or a
    /// declared fallback for wider content).
    fn resolve_monitoring(&self, path: &SpatialOutputPath) -> Result<String, SpatialUnavailable> {
        match path {
            SpatialOutputPath::Binaural { .. } => Ok("headphones".into()),
            SpatialOutputPath::ChannelBus { layout } => {
                let mon = self
                    .env
                    .monitoring
                    .as_ref()
                    .ok_or(SpatialUnavailable::NoMonitoringConfig)?;
                if let Err(errors) = mon.validate() {
                    return Err(SpatialUnavailable::MonitoringInvalid {
                        errors: errors.iter().map(|e| e.to_string()).collect(),
                    });
                }
                match mon.monitor_for(*layout) {
                    MonitorVerdict::Direct => Ok(format!("direct:{layout}")),
                    MonitorVerdict::ViaFallback { to } => Ok(match to {
                        Some(l) => format!("downmix:{layout}->{l}"),
                        None => "binaural-fallback".into(),
                    }),
                    MonitorVerdict::Unavailable { reason } => {
                        Err(SpatialUnavailable::MonitorUnreachable {
                            layout: *layout,
                            reason: format!("{reason:?}"),
                        })
                    }
                }
            }
            SpatialOutputPath::BedPlusObjects { container } => {
                // Immersive render to a speaker set: the bed (or the
                // largest legal bed implied) must be monitorable;
                // objects render against the same declared set.
                let mon = self
                    .env
                    .monitoring
                    .as_ref()
                    .ok_or(SpatialUnavailable::NoMonitoringConfig)?;
                if let Err(errors) = mon.validate() {
                    return Err(SpatialUnavailable::MonitoringInvalid {
                        errors: errors.iter().map(|e| e.to_string()).collect(),
                    });
                }
                let bed = container.bed.unwrap_or(SpatialLayout::Surround712);
                match mon.monitor_for(bed) {
                    MonitorVerdict::Direct => Ok(format!("bed-direct:{bed}+objects")),
                    MonitorVerdict::ViaFallback { to } => Ok(match to {
                        Some(l) => format!("bed-fallback:{bed}->{l}+objects"),
                        None => "binaural-fallback+objects".into(),
                    }),
                    MonitorVerdict::Unavailable { reason } => {
                        Err(SpatialUnavailable::MonitorUnreachable {
                            layout: bed,
                            reason: format!("{reason:?}"),
                        })
                    }
                }
            }
            // Atmos paths return before this — deliverable QC, not
            // monitoring.
            _ => Ok("deliverable".into()),
        }
    }
}

fn container_of(path: &SpatialOutputPath) -> Option<&ObjectContainerSpec> {
    match path {
        SpatialOutputPath::AtmosAdmBwf { container }
        | SpatialOutputPath::AtmosMp4 { container }
        | SpatialOutputPath::BedPlusObjects { container } => Some(container),
        _ => None,
    }
}
