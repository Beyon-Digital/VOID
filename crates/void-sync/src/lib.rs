//! void-sync — external synchronization model for VOID (W27 non-native
//! part; PRO-03 policy, T94/T96 model side).
//!
//! Real byte codecs and honest state machines for the sync surfaces
//! VOID models: MTC (full + quarter-frame, bit-exact), MIDI Clock
//! (24 ppqn rational math), MMC commands, single-master arbitration
//! with recorded conflict resolution and measured drift, OSC
//! address-space + bounded message validation, DMX descriptors.
//!
//! Nothing here claims wire delivery: endpoints are descriptors and
//! verdicts are typed. Link is a documented non-implementation
//! (`link` + `docs/show/ADR-LINK.md`). Hardware transport wiring is
//! an integrator deliverable.

mod clock;
mod dmx;
mod error;
mod link;
mod master;
mod mmc;
mod mtc;
mod osc;

pub use clock::{
    pulses_to_ticks, ticks_to_pulses, ClockEvent, ClockTransport, MidiClockMessage, TickPulse,
    MIDI_CLOCK_PPQN,
};
pub use dmx::{
    DmxChannelDescriptor, DmxFixture, DmxRig, DmxUniverse, DMX_UNIVERSE_CHANNELS, MAX_STROBE_HZ,
};
pub use error::SyncError;
pub use link::LinkAvailability;
pub use master::{
    sync_health, ArbiterEvent, ConflictResolution, DriftModel, DriftReport, MasterArbiter,
    MasterConflict, SyncHealth, SyncSource,
};
pub use mmc::{decode as mmc_decode, encode as mmc_encode, MmcCommand, MmcFrameKind};
pub use mtc::{
    decode_full_message, demux, encode_full_message, encode_quarter_frame_pieces,
    encode_quarter_frames, quarter_frame_message, MtcFrameRate, MtcTime, MtcWireEvent,
    QuarterFrameAssembler, FULL_MESSAGE_LEN, QUARTER_FRAME_COUNT, QUARTER_FRAME_MESSAGE_LEN,
};
pub use osc::{
    validate_against, OscAddress, OscArg, OscArgSpec, OscArgType, OscControl, OscMessage,
    MAX_OSC_PAYLOAD_ARGS,
};
