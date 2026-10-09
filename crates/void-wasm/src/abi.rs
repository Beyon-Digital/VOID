//! void-wasm ABI v1 — the narrow descriptor every restricted module
//! speaks. Export surface (module → host) plus the single host
//! import (host → module). Documented in docs/wasm/ABI.md.
//!
//! Exports (required):
//!   `memory` — linear memory (defined internally, never imported).
//!   `void_alloc(len: i32) -> i32` — guest allocator; host reserves
//!     output buffers by calling this, then passes the pointer into
//!     entry points. A missing/misbehaving allocator is an ABI fault.
//!   `void_abi_version() -> i32` — must return `ABI_VERSION`.
//!   `void_init() -> i32` — 0 = accept; nonzero = reject at load.
//!
//! Exports (capability-gated — required iff the manifest declares):
//!   params: `void_param_count() -> i32`,
//!           `void_param_get(index: i32) -> f64`,
//!           `void_param_set(index: i32, value: f64) -> i32` (0=ok)
//!   state:  `void_state_len() -> i32`,
//!           `void_state_save(ptr: i32) -> i32` (bytes written),
//!           `void_state_restore(ptr: i32, len: i32) -> i32` (0=ok)
//!   render-off: `void_render_off(ptr: i32, frames: i32) -> i32`
//!           — writes f32 PCM at the host-allocated pointer; OFFLINE
//!           only (placement is a declared constant, never rt).
//!   midi-transform: `void_midi_xform(in_ptr: i32, in_len: i32,
//!           out_ptr: i32, out_cap: i32) -> i32` — writes packed
//!           NoteEvents; return = bytes written ≤ out_cap.
//!
//! Imports (module may request — only this one is granted):
//!   `void_host.log(level: i32, ptr: i32, len: i32)` — bounded,
//!     recorded, never leaks to fs/net. Any other import fails
//!     validation AND linking (defense in depth).

pub const ABI_NAME: &str = "void-wasm/1";
pub const ABI_VERSION: i32 = 1;

// Required exports.
pub const FN_ALLOC: &str = "void_alloc";
pub const FN_ABI_VERSION: &str = "void_abi_version";
pub const FN_INIT: &str = "void_init";
pub const FN_PARAM_COUNT: &str = "void_param_count";
pub const FN_PARAM_GET: &str = "void_param_get";
pub const FN_PARAM_SET: &str = "void_param_set";
pub const FN_STATE_LEN: &str = "void_state_len";
pub const FN_STATE_SAVE: &str = "void_state_save";
pub const FN_STATE_RESTORE: &str = "void_state_restore";
pub const FN_RENDER_OFF: &str = "void_render_off";
pub const FN_MIDI_XFORM: &str = "void_midi_xform";
pub const EXPORT_MEMORY: &str = "memory";

/// Packed MIDI event record on the wire buffer — little-endian, 24 B:
///   0..8   onset_ticks     i64 (960000 ticks/quarter, signed)
///   8..16  length_ticks    i64
///   16     pitch           u8  (0..=127)
///   17     velocity        u8  (0..=127)
///   18     channel         u8  (0..=15)
///   19     kind            u8  (0=note,1=cc — reserved expansion)
///   20..24 reserved        0
pub const NOTE_EVENT_SIZE: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteEvent {
    pub onset_ticks: i64,
    pub length_ticks: i64,
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
    pub kind: u8,
}

impl NoteEvent {
    pub fn encode(&self, out: &mut [u8]) {
        out[0..8].copy_from_slice(&self.onset_ticks.to_le_bytes());
        out[8..16].copy_from_slice(&self.length_ticks.to_le_bytes());
        out[16] = self.pitch;
        out[17] = self.velocity;
        out[18] = self.channel;
        out[19] = self.kind;
        out[20..24].fill(0);
    }

    pub fn decode(buf: &[u8]) -> Option<NoteEvent> {
        if buf.len() < NOTE_EVENT_SIZE {
            return None;
        }
        Some(NoteEvent {
            onset_ticks: i64::from_le_bytes(buf[0..8].try_into().ok()?),
            length_ticks: i64::from_le_bytes(buf[8..16].try_into().ok()?),
            pitch: buf[16],
            velocity: buf[17],
            channel: buf[18],
            kind: buf[19],
        })
    }

    /// Validate musical ranges (pitch/velocity ≤127, channel ≤15,
    /// positive length) — malformed events are data errors, never
    /// passed to the engine.
    pub fn is_valid(&self) -> bool {
        self.pitch <= 127 && self.velocity <= 127 && self.channel <= 15 && self.length_ticks > 0
    }
}

pub fn encode_events(events: &[NoteEvent]) -> Vec<u8> {
    let mut out = vec![0u8; events.len() * NOTE_EVENT_SIZE];
    for (i, e) in events.iter().enumerate() {
        e.encode(&mut out[i * NOTE_EVENT_SIZE..(i + 1) * NOTE_EVENT_SIZE]);
    }
    out
}

pub fn decode_events(buf: &[u8]) -> Vec<NoteEvent> {
    buf.chunks_exact(NOTE_EVENT_SIZE)
        .filter_map(NoteEvent::decode)
        .collect()
}
