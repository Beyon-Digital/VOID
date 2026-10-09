//! Tempo-master arbitration and drift measurement (PRO-03, T94 model
//! side).
//!
//! Exactly one sync source may be master at a time — "No competing
//! transport master" is a T94 acceptance criterion, so a second claim
//! is a typed [`MasterConflict`], never a silent takeover. Conflict
//! resolution is explicit and recorded: keep the held master, or
//! switch at a declared point. Drift is *measured* against tolerance,
//! not asserted.

use serde::{Deserialize, Serialize};

/// A sync source that could claim master.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SyncSource {
    /// VOID's internal transport clock (always available offline).
    Internal,
    /// Incoming MIDI Clock on a named endpoint.
    MidiClockIn { endpoint: String },
    /// Incoming MTC on a named endpoint.
    MtcIn { endpoint: String },
    /// An Ableton Link peer session — gated: VOID does not ship a
    /// Link path this lane (see docs/show/ADR-LINK.md); the variant
    /// exists so the *rule* is modelled.
    LinkPeer { session: String },
    /// External transport over a scoped OSC prefix.
    OscExternal { prefix: String },
}

impl SyncSource {
    pub fn id(&self) -> String {
        match self {
            SyncSource::Internal => "internal".into(),
            SyncSource::MidiClockIn { endpoint } => format!("midi-clock:{endpoint}"),
            SyncSource::MtcIn { endpoint } => format!("mtc:{endpoint}"),
            SyncSource::LinkPeer { session } => format!("link:{session}"),
            SyncSource::OscExternal { prefix } => format!("osc:{prefix}"),
        }
    }
}

/// A competing-master claim, retained for the resolution record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterConflict {
    /// The source currently holding master.
    pub held: SyncSource,
    /// The source that tried to claim it.
    pub attempted: SyncSource,
}

/// How a conflict was resolved — recorded in the arbitration log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ConflictResolution {
    /// Attempted source rejected; held master keeps master.
    KeepHeld,
    /// Switch masters at the next transport stop (no mid-phrase jump).
    SwitchAtNextStop,
    /// Switch immediately (discontinuity is explicit and logged).
    SwitchImmediate,
}

/// One arbitration-log entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ArbiterEvent {
    MasterClaimed { source: String },
    ConflictRejected { held: String, attempted: String },
    SwitchScheduled { from: String, to: String, at: String },
    MasterSwitched { from: String, to: String },
    MasterReleased { source: String },
}

/// The arbiter: at most one master, ever.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterArbiter {
    master: Option<SyncSource>,
    /// A switch scheduled at next stop, applied by `on_transport_stop`.
    pending_switch: Option<SyncSource>,
    /// Append-only arbitration record — the T94 "resolution record".
    log: Vec<ArbiterEvent>,
}

impl MasterArbiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn master(&self) -> Option<&SyncSource> {
        self.master.as_ref()
    }

    pub fn log(&self) -> &[ArbiterEvent] {
        &self.log
    }

    /// Claim mastership. First claim wins; a second claim is a typed
    /// conflict — resolve it explicitly with [`Self::resolve`].
    pub fn claim(&mut self, source: SyncSource) -> Result<(), MasterConflict> {
        match &self.master {
            None => {
                self.log.push(ArbiterEvent::MasterClaimed {
                    source: source.id(),
                });
                self.master = Some(source);
                Ok(())
            }
            Some(held) if *held == source => Ok(()), // idempotent re-claim
            Some(held) => Err(MasterConflict {
                held: held.clone(),
                attempted: source,
            }),
        }
    }

    /// Apply a resolution to a live conflict. `KeepHeld` is a no-op
    /// on state but lands in the log — the *record* is the point.
    pub fn resolve(&mut self, conflict: &MasterConflict, resolution: ConflictResolution) {
        match resolution {
            ConflictResolution::KeepHeld => {
                self.log.push(ArbiterEvent::ConflictRejected {
                    held: conflict.held.id(),
                    attempted: conflict.attempted.id(),
                });
            }
            ConflictResolution::SwitchAtNextStop => {
                self.pending_switch = Some(conflict.attempted.clone());
                self.log.push(ArbiterEvent::SwitchScheduled {
                    from: conflict.held.id(),
                    to: conflict.attempted.id(),
                    at: "next-stop".to_string(),
                });
            }
            ConflictResolution::SwitchImmediate => {
                if self.master.as_ref() == Some(&conflict.held) {
                    let from = conflict.held.id();
                    let to = conflict.attempted.id();
                    self.master = Some(conflict.attempted.clone());
                    self.pending_switch = None;
                    self.log.push(ArbiterEvent::MasterSwitched { from, to });
                }
            }
        }
    }

    /// The transport stopped — apply any scheduled switch.
    pub fn on_transport_stop(&mut self) {
        if let Some(next) = self.pending_switch.take() {
            if let Some(held) = self.master.take() {
                self.log.push(ArbiterEvent::MasterSwitched {
                    from: held.id(),
                    to: next.id(),
                });
            }
            self.master = Some(next);
        }
    }

    /// Release mastership entirely (sync lost / source disconnected).
    /// Returns false if `source` did not hold master — honest.
    pub fn release(&mut self, source: &SyncSource) -> bool {
        if self.master.as_ref() == Some(source) {
            self.master = None;
            self.log.push(ArbiterEvent::MasterReleased {
                source: source.id(),
            });
            true
        } else {
            false
        }
    }
}

/// Drift measurement result — ppm error over a measurement window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    /// Expected clock edges for the window (from declared rate).
    pub expected: u64,
    /// Observed clock edges.
    pub received: u64,
    /// Signed error in parts-per-million (received − expected).
    pub error_ppm: f64,
    /// `expected` window length in seconds the measurement covered.
    pub window_seconds: f64,
    /// Whether |ppm| is within the configured tolerance.
    pub within_tolerance: bool,
    /// Instant jump detector: observed position jumped vs the pulse
    /// train — sync must resync, not interpolate across a cliff.
    pub jump_detected: bool,
}

/// Tolerance model for a clock master — measured drift must stay
/// within `tolerance_ppm` or the source is flagged, never smoothed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftModel {
    /// Absolute ppm error allowed before flagging (e.g. 100 ppm ≈
    /// a very sloppy clock; consumer gear ~50–500 ppm).
    pub tolerance_ppm: f64,
    /// Position discontinuity threshold in pulses before a jump is
    /// declared (hysteresis so a single dropped pulse ≠ resync).
    pub jump_threshold_pulses: u32,
}

impl Default for DriftModel {
    fn default() -> Self {
        DriftModel {
            tolerance_ppm: 100.0,
            jump_threshold_pulses: 6,
        }
    }
}

impl DriftModel {
    /// Measure received-vs-expected over a window. `expected` comes
    /// from the declared rate × window length — never from rounding
    /// the observed count.
    pub fn measure(
        &self,
        expected: u64,
        received: u64,
        window_seconds: f64,
        observed_pulses_since_expected_pos: i64,
    ) -> DriftReport {
        let error_ppm = if expected == 0 {
            if received == 0 {
                0.0
            } else {
                f64::INFINITY // nothing expected, something arrived — flag hard
            }
        } else {
            ((received as f64) - (expected as f64)) / (expected as f64) * 1.0e6
        };
        DriftReport {
            expected,
            received,
            error_ppm,
            window_seconds,
            within_tolerance: error_ppm.is_finite() && error_ppm.abs() <= self.tolerance_ppm,
            jump_detected: observed_pulses_since_expected_pos.unsigned_abs() as u32
                > self.jump_threshold_pulses,
        }
    }
}

/// Health of the active sync source — surfaced to the UI, never
/// hidden behind "synced".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SyncHealth {
    /// Within tolerance and no jump.
    Locked,
    /// Receiving but drifting beyond tolerance.
    Drifting { report: DriftReport },
    /// A position discontinuity larger than the jump threshold —
    /// resync required; do not interpolate across it.
    ResyncRequired { report: DriftReport },
    /// No traffic on the declared source for `since_ms`.
    Lost { since_ms: u64 },
    /// No master claimed — local transport runs standalone.
    NoMaster,
}

/// Decide sync health from a drift report + liveness.
pub fn sync_health(
    master_present: bool,
    last_report: Option<&DriftReport>,
    silence_ms: u64,
    silence_lost_ms: u64,
) -> SyncHealth {
    if !master_present {
        return SyncHealth::NoMaster;
    }
    if silence_ms >= silence_lost_ms {
        return SyncHealth::Lost { since_ms: silence_ms };
    }
    match last_report {
        None => SyncHealth::Locked, // no measurement yet — honest: no evidence of drift
        Some(r) if r.jump_detected => SyncHealth::ResyncRequired { report: r.clone() },
        Some(r) if !r.within_tolerance => SyncHealth::Drifting { report: r.clone() },
        Some(_) => SyncHealth::Locked,
    }
}
