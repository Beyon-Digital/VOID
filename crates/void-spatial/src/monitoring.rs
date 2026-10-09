//! Monitoring configuration (OUT-06, T93 model side).
//!
//! A [`MonitoringConfig`] declares the physical speaker set, per-speaker
//! level-calibration trims and the declared fallback used when content
//! does not match the set. Validation is strict: every speaker in the
//! declared set must carry a finite trim within bounds, and a declared
//! fold-down fallback must be a legal fold per [`crate::layout::legality`].
//! "Stereo is close enough" is never a valid fallback — `legality`
//! decides reachability, and a fallback that can't be reached is a
//! validation error, not a silent stereo proxy.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::SpatialError;
use crate::layout::{legality, Legality, SpatialLayout, Speaker};

/// Trim bound: per-speaker calibration within ±20 dB.
pub const MAX_TRIM_DB: f32 = 20.0;

/// What the monitor path falls back to when the content layout
/// cannot drive the declared speaker set directly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FallbackMode {
    /// Declared fold-down to a smaller channel layout — the explicit
    /// downmix T92/T93 requires (e.g. 7.1.4 content on a 5.1 rig).
    DeclaredDownmix { to: SpatialLayout },
    /// Binaural headphone render — allowed as a declared fallback for
    /// any layout; headphones are a real monitoring target, not a
    /// proxy claim.
    Binaural,
    /// No fallback — content that doesn't fit the speaker set is
    /// simply unplayable on this monitor config.
    None,
}

/// Declared monitoring configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringConfig {
    /// The physical loudspeaker layout actually present. Every speaker
    /// of this layout must appear in `level_calibration_db`.
    pub speaker_set: SpatialLayout,
    /// Per-speaker calibration trim in dB (finite, ±[`MAX_TRIM_DB`]).
    /// A missing entry for any declared speaker is a validation error:
    /// uncalibrated output is not a calibrated monitor path.
    pub level_calibration_db: BTreeMap<Speaker, f32>,
    /// What happens when the content layout exceeds the speaker set.
    pub fallback: FallbackMode,
    /// Optional listener-distance note (metres) recorded for QC —
    /// informational only.
    pub distance_m: Option<f32>,
}

/// Can this monitor config play `content` — and via what route?
#[derive(Debug, Clone, PartialEq)]
pub enum MonitorVerdict {
    /// The speaker set matches the content exactly.
    Direct,
    /// Playable only via the declared fallback; carries the resolved
    /// target layout (or `None` for binaural).
    ViaFallback { to: Option<SpatialLayout> },
    /// Not playable — typed reason, never approximated.
    Unavailable { reason: MonitorUnavailable },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MonitorUnavailable {
    /// Content needs a fold-down but the config declares none.
    NoDeclaredFallback { content: SpatialLayout },
    /// Declared fallback can't legally fold this content (e.g. a
    /// wider layout, or an illegal quad target).
    FallbackUnreachable {
        content: SpatialLayout,
        fallback: String,
    },
}

impl MonitoringConfig {
    pub fn validate(&self) -> Result<(), Vec<SpatialError>> {
        let mut errors = Vec::new();
        // Every declared speaker needs a finite bounded trim.
        for a in self.speaker_set.speaker_assignments() {
            match self.level_calibration_db.get(&a.speaker) {
                None => errors.push(SpatialError::MissingCalibration {
                    speaker: a.speaker,
                }),
                Some(t) => {
                    if !t.is_finite() {
                        errors.push(SpatialError::NonFinite {
                            field: format!("levelCalibrationDb.{:?}", a.speaker),
                        });
                    } else if t.abs() > MAX_TRIM_DB {
                        errors.push(SpatialError::OutOfRange {
                            field: format!("levelCalibrationDb.{:?}", a.speaker),
                            value: t.to_string(),
                            bounds: format!("±{MAX_TRIM_DB} dB"),
                        });
                    }
                }
            }
        }
        // Stray trims for speakers not in the set are suspicious but
        // harmless; flag them so a typo'd speaker id is not ignored.
        for s in self.level_calibration_db.keys() {
            if !self
                .speaker_set
                .speaker_assignments()
                .iter()
                .any(|a| &a.speaker == s)
            {
                errors.push(SpatialError::StrayCalibration { speaker: *s });
            }
        }
        // Declared downmix fallback: `to` must be playable on the
        // physical set (exact, mono replication, or a contained
        // speaker subset) and must not equal the set — a "fallback"
        // to the same layout declares nothing.
        if let FallbackMode::DeclaredDownmix { to } = &self.fallback {
            if *to == self.speaker_set {
                errors.push(SpatialError::OutOfRange {
                    field: "fallback.to".into(),
                    value: to.to_string(),
                    bounds: "smaller than speakerSet".into(),
                });
            } else if !can_drive(*to, self.speaker_set) {
                errors.push(SpatialError::OutOfRange {
                    field: "fallback.to".into(),
                    value: to.to_string(),
                    bounds: format!("reachable on {}", self.speaker_set),
                });
            }
        }
        if let Some(d) = self.distance_m {
            if !(d.is_finite() && d > 0.0) {
                errors.push(SpatialError::OutOfRange {
                    field: "distanceM".into(),
                    value: d.to_string(),
                    bounds: ">0".into(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Resolve whether `content` can play on this monitor config.
    /// `Direct` covers the exact match, mono replication, and a
    /// narrower source placed on its matching speaker subset (stereo
    /// on a 5.1 rig is *placement*, not an upmix). Wider content is
    /// playable only via the declared fallback; anything else is
    /// typed-unavailable.
    pub fn monitor_for(&self, content: SpatialLayout) -> MonitorVerdict {
        if can_drive(content, self.speaker_set) {
            return MonitorVerdict::Direct;
        }
        match &self.fallback {
            FallbackMode::None => MonitorVerdict::Unavailable {
                reason: MonitorUnavailable::NoDeclaredFallback { content },
            },
            FallbackMode::Binaural => MonitorVerdict::ViaFallback { to: None },
            FallbackMode::DeclaredDownmix { to } => {
                // Content must legally fold to `to` (direct fit is
                // pointless but legal), and `to` must be playable on
                // the physical set.
                let folds = matches!(
                    legality(content, *to),
                    Legality::RequiresDeclaredDownmix | Legality::Direct
                );
                if folds && can_drive(*to, self.speaker_set) {
                    MonitorVerdict::ViaFallback { to: Some(*to) }
                } else {
                    MonitorVerdict::Unavailable {
                        reason: MonitorUnavailable::FallbackUnreachable {
                            content,
                            fallback: format!("downmix to {to}"),
                        },
                    }
                }
            }
        }
    }
}

/// Whether `content` can drive `set` without inventing spatial
/// information: exact match, mono replication onto any set, or every
/// content speaker physically present in the set (placement — the
/// remaining channels stay silent, nothing is "upmixed").
pub fn can_drive(content: SpatialLayout, set: SpatialLayout) -> bool {
    if content == set || content == SpatialLayout::Mono {
        return true;
    }
    let set_speakers: std::collections::BTreeSet<Speaker> = set
        .speaker_assignments()
        .iter()
        .map(|a| a.speaker)
        .collect();
    content
        .speaker_assignments()
        .iter()
        .all(|a| set_speakers.contains(&a.speaker))
}
