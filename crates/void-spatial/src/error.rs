//! Typed spatial errors — descriptor/config validation failures.
//! *Availability* is a verdict, not an error: see
//! [`crate::gate::SpatialUnavailable`] for the T92/T93 "unavailable
//! with a reason" vocabulary.

use thiserror::Error;

use crate::layout::{SpatialLayout, Speaker};

#[derive(Debug, Error)]
pub enum SpatialError {
    #[error("missing required field '{field}'")]
    MissingField { field: String },

    #[error("non-finite value in '{field}'")]
    NonFinite { field: String },

    #[error("'{field}' = {value} out of range ({bounds})")]
    OutOfRange {
        field: String,
        value: String,
        bounds: String,
    },

    #[error("layout {layout} is not a legal channel bed (5.1/7.1/7.1.2 only)")]
    IllegalBed { layout: SpatialLayout },

    #[error("duplicate object id '{object_id}'")]
    DuplicateObjectId { object_id: String },

    #[error("object {index} invalid: {detail}")]
    InvalidObject { index: usize, detail: String },

    #[error("speaker {speaker} has no calibration trim")]
    MissingCalibration { speaker: Speaker },

    #[error("calibration trim declared for {speaker}, which is not in the speaker set")]
    StrayCalibration { speaker: Speaker },

    #[error("layout {from} cannot feed {to} ({reason})")]
    IllegalConversion {
        from: SpatialLayout,
        to: SpatialLayout,
        reason: String,
    },
}
