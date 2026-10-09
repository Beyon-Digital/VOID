//! Object/bed metadata model (OUT-07).
//!
//! An object carries position, extent, gain and divergence as *bounded*
//! descriptors — validated on construction paths, never clamped.
//! [`ObjectContainerSpec`] is the bed+object container declaration:
//! a channel-based bed layout plus independently positioned objects.
//! It describes what a deliverable would contain; it does not claim
//! any renderer produced it — that is [`crate::gate`] verdict work.

use serde::{Deserialize, Serialize};

use crate::error::SpatialError;
use crate::layout::SpatialLayout;

/// Maximum objects in one container — mirrors the Atmos ADM bound
/// (118 objects) so a container that could never be authored is
/// rejected up front rather than truncated.
pub const MAX_OBJECTS: usize = 118;

/// Position descriptor in normalized ADM-style polar coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPosition {
    /// Degrees, −180..=180, positive anticlockwise (left = +).
    pub azimuth_deg: f32,
    /// Degrees, −90..=90, positive up.
    pub elevation_deg: f32,
    /// Normalized distance, 0 (at the listener) ..= 1 (room edge).
    pub distance: f32,
}

/// Angular extent of an object — how much of the sphere it occupies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectExtent {
    /// 0 (point source) ..= 1 (full hemisphere width).
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

/// One audio object in a container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectDescriptor {
    pub object_id: String,
    /// Free-form label (e.g. "lead-vocal"); display only.
    pub name: Option<String>,
    pub position: ObjectPosition,
    pub extent: ObjectExtent,
    /// Static gain in dB, −60..=+12. Rejects NaN/Inf.
    pub gain_db: f32,
    /// Divergence 0..=1 — how much the object bleeds toward other
    /// loudspeakers as it moves off-axis.
    pub divergence: f32,
    /// Source channel count routed into this object (1 = mono object;
    /// larger = channel-coupled object).
    pub channel_count: u8,
}

/// Bed+object container specification — the deliverable's metadata
/// shape. A `None` bed is legal (pure-object container); a present
/// bed must satisfy [`SpatialLayout::is_legal_bed`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectContainerSpec {
    pub container_id: String,
    /// Channel-based bed under the objects, if any.
    pub bed: Option<SpatialLayout>,
    pub objects: Vec<ObjectDescriptor>,
    /// Sample rate the object metadata is timed at (frame rate for
    /// dynamic position automation lives with the engine, not here).
    pub sample_rate_hz: u32,
}

fn check_finite(v: f32, what: &str) -> Result<(), SpatialError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(SpatialError::NonFinite { field: what.into() })
    }
}

impl ObjectPosition {
    pub fn validate(&self) -> Result<(), SpatialError> {
        check_finite(self.azimuth_deg, "position.azimuthDeg")?;
        check_finite(self.elevation_deg, "position.elevationDeg")?;
        check_finite(self.distance, "position.distance")?;
        if !(-180.0..=180.0).contains(&self.azimuth_deg) {
            return Err(SpatialError::OutOfRange {
                field: "position.azimuthDeg".into(),
                value: self.azimuth_deg.to_string(),
                bounds: "-180..=180".into(),
            });
        }
        if !(-90.0..=90.0).contains(&self.elevation_deg) {
            return Err(SpatialError::OutOfRange {
                field: "position.elevationDeg".into(),
                value: self.elevation_deg.to_string(),
                bounds: "-90..=90".into(),
            });
        }
        if !(0.0..=1.0).contains(&self.distance) {
            return Err(SpatialError::OutOfRange {
                field: "position.distance".into(),
                value: self.distance.to_string(),
                bounds: "0..=1".into(),
            });
        }
        Ok(())
    }
}

impl ObjectExtent {
    pub fn validate(&self) -> Result<(), SpatialError> {
        for (name, v) in [
            ("extent.width", self.width),
            ("extent.height", self.height),
            ("extent.depth", self.depth),
        ] {
            check_finite(v, name)?;
            if !(0.0..=1.0).contains(&v) {
                return Err(SpatialError::OutOfRange {
                    field: name.into(),
                    value: v.to_string(),
                    bounds: "0..=1".into(),
                });
            }
        }
        Ok(())
    }
}

impl ObjectDescriptor {
    pub fn validate(&self) -> Result<(), SpatialError> {
        if self.object_id.is_empty() {
            return Err(SpatialError::MissingField {
                field: "objectId".into(),
            });
        }
        if self.channel_count == 0 {
            return Err(SpatialError::OutOfRange {
                field: "channelCount".into(),
                value: "0".into(),
                bounds: "1..=u8::MAX".into(),
            });
        }
        self.position.validate()?;
        self.extent.validate()?;
        check_finite(self.gain_db, "gainDb")?;
        if !(-60.0..=12.0).contains(&self.gain_db) {
            return Err(SpatialError::OutOfRange {
                field: "gainDb".into(),
                value: self.gain_db.to_string(),
                bounds: "-60..=+12 dB".into(),
            });
        }
        check_finite(self.divergence, "divergence")?;
        if !(0.0..=1.0).contains(&self.divergence) {
            return Err(SpatialError::OutOfRange {
                field: "divergence".into(),
                value: self.divergence.to_string(),
                bounds: "0..=1".into(),
            });
        }
        Ok(())
    }
}

impl ObjectContainerSpec {
    /// Validate the container spec. Returns every violation as a
    /// list rather than stopping at the first — a spec rejected for
    /// five reasons tells the author all five.
    pub fn validate(&self) -> Result<(), Vec<SpatialError>> {
        let mut errors = Vec::new();
        let mut bad = |e: SpatialError| errors.push(e);

        if self.container_id.is_empty() {
            bad(SpatialError::MissingField {
                field: "containerId".into(),
            });
        }
        if let Some(bed) = self.bed {
            if !bed.is_legal_bed() {
                bad(SpatialError::IllegalBed { layout: bed });
            }
        }
        if self.objects.len() > MAX_OBJECTS {
            bad(SpatialError::OutOfRange {
                field: "objects".into(),
                value: self.objects.len().to_string(),
                bounds: format!("0..={MAX_OBJECTS}"),
            });
        }
        if self.sample_rate_hz == 0 {
            bad(SpatialError::OutOfRange {
                field: "sampleRateHz".into(),
                value: "0".into(),
                bounds: ">=1".into(),
            });
        }
        let mut seen = std::collections::HashSet::new();
        for (i, obj) in self.objects.iter().enumerate() {
            if !seen.insert(obj.object_id.as_str()) {
                bad(SpatialError::DuplicateObjectId {
                    object_id: obj.object_id.clone(),
                });
            }
            if let Err(e) = obj.validate() {
                bad(SpatialError::InvalidObject {
                    index: i,
                    detail: e.to_string(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Total bed + object channels the container implies — the
    /// honest channel budget a renderer would need.
    pub fn implied_channels(&self) -> u64 {
        self.bed.map(|b| b.channels() as u64).unwrap_or(0)
            + self
                .objects
                .iter()
                .map(|o| o.channel_count as u64)
                .sum::<u64>()
    }
}
