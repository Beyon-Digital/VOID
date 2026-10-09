//! VOID native control protocol: FlatBuffers bindings, framing, validation,
//! and receipt tracking. Wire semantics: `protocol/README.md` +
//! `docs/void-handoff/CONTRACTS.md`.

#[allow(clippy::all, dead_code, unused_imports)]
pub mod generated {
    #![allow(clippy::all, unused_imports)]
    include!(concat!(env!("OUT_DIR"), "/void_control_generated.rs"));
}

pub mod frame;
pub mod ids;
pub mod limits;
pub mod receipts;
pub mod validate;

pub use generated::voidproto as proto;

pub const PROTOCOL_MAJOR: u16 = 1;
/// Minor 1 = rev-2: take/scene/job/model/proposal views + recording, ingest,
/// bypass, rescan, save-as, proposal, job, and launch ops (NEEDS.md).
pub const PROTOCOL_MINOR: u16 = 1;

/// Musical time resolution: ticks per quarter note (CONTRACTS.md §1).
pub const TICKS_PER_QUARTER: i64 = 960_000;
