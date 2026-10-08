//! The exchange document — the format-neutral song model this crate
//! interchanges against.
//!
//! It deliberately spans *more* than the major.1 wire surface: fades,
//! markers, per-note channel/release, plugin descriptors and state refs
//! exist here because foreign formats carry them and T77 requires us to
//! account for them honestly. The wire op plan (`plan.rs`) is where those
//! fields hit their actual limits — anything the ops cannot express is a
//! loss entry, never silently dropped.
//!
//! Serde contract: camelCase; all signed 64-bit tick/sample fields and
//! u64 byte counts serialize as DECIMAL STRINGS (`crates/void-protocol`,
//! `apps/void-tauri/src-tauri/src/codec.rs`).

use crate::registry::PluginDescriptor;
use serde::{Deserialize, Serialize};

pub const TICKS_PER_QUARTER: i64 = void_protocol::TICKS_PER_QUARTER;
pub const EXCHANGE_FORMAT: &str = "void-exchange/1";

pub mod ticks_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum T {
            S(String),
            // Accept bare integers only when they fit i64 exactly; JSON
            // floats are rejected (they lie about ticks).
            I(i64),
        }
        match T::deserialize(d)? {
            T::S(s) => s.trim().parse::<i64>().map_err(|_| {
                serde::de::Error::custom(format!("tick field {:?} is not a decimal i64", s))
            }),
            T::I(v) => Ok(v),
        }
    }
    pub mod opt {
        use super::*;
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TicksOrInt {
            S(String),
            I(i64),
        }

        pub fn serialize<S: Serializer>(v: &Option<i64>, s: S) -> Result<S::Ok, S::Error> {
            match v {
                Some(v) => s.serialize_str(&v.to_string()),
                None => s.serialize_none(),
            }
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
            match Option::<TicksOrInt>::deserialize(d)? {
                None => Ok(None),
                Some(TicksOrInt::S(s)) => s.trim().parse::<i64>().map(Some).map_err(|_| {
                    serde::de::Error::custom(format!("tick field {:?} is not a decimal i64", s))
                }),
                Some(TicksOrInt::I(v)) => Ok(Some(v)),
            }
        }
    }
}

/// Same contract for u64 fields (byte counts).
pub mod u64_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum V {
            S(String),
            N(u64),
        }
        match V::deserialize(d)? {
            V::S(s) => s
                .trim()
                .parse::<u64>()
                .map_err(|_| serde::de::Error::custom(format!("bad u64 string {s:?}"))),
            V::N(n) => Ok(n),
        }
    }
}

/// Wire-mirrored track kinds (`void_control.fbs` TrackKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    AUDIO,
    MIDI,
    INSTRUMENT,
    BUS,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoPoint {
    #[serde(with = "ticks_serde")]
    pub at_ticks: i64,
    /// Beats per minute; finite and > 0. Step changes only — tempo ramps
    /// are NEEDS item 27 and must not appear here.
    pub bpm: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSignaturePoint {
    #[serde(with = "ticks_serde")]
    pub at_ticks: i64,
    pub numerator: u32,
    /// Denominator as written (4 = quarter). Must be a power of two to be
    /// representable in SMF; other denominators are legal in the document
    /// but approximate on MIDI export.
    pub denominator: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    #[serde(with = "ticks_serde")]
    pub at_ticks: i64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRange {
    #[serde(with = "ticks_serde")]
    pub start_ticks: i64,
    #[serde(with = "ticks_serde")]
    pub end_ticks: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// Opaque stable id. DAWproject notes carry no identity — imported
    /// notes get deterministic v5 uuids derived from clip id + ordinal.
    pub id: String,
    /// MIDI note number 0..=127.
    pub pitch: u8,
    /// 1..=127 (wire `InsertNoteOp` floor).
    pub velocity: u8,
    /// Release velocity if the source carried one (0..=127).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_velocity: Option<u8>,
    /// MIDI channel 0..=15. The wire has no note channel field — op-plan
    /// emission reports non-zero channels as approximations.
    #[serde(default)]
    pub channel: u8,
    /// Clip-content-relative start and length (wire `InsertNoteOp`
    /// semantics — F1 lane verified clip-relative).
    #[serde(with = "ticks_serde")]
    pub start_ticks: i64,
    #[serde(with = "ticks_serde")]
    pub length_ticks: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ClipContent {
    /// Audio clip referencing an immutable asset. The payload itself is in
    /// `assets[]` + the container `files` map — never embedded here.
    #[serde(rename = "audio")]
    Audio { asset_id: String },
    /// MIDI clip; notes sorted by (start_ticks, id) at emit.
    #[serde(rename = "notes")]
    Notes { notes: Vec<Note> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Arrangement position and visible length (timeline ticks).
    #[serde(with = "ticks_serde")]
    pub start_ticks: i64,
    #[serde(with = "ticks_serde")]
    pub length_ticks: i64,
    /// Offset into the clip's content (dawproject playStart).
    #[serde(with = "ticks_serde", default)]
    pub offset_ticks: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Source fade times — richer than the wire model (NEEDS item 18):
    /// preserved for interchange honesty, approximated in the op plan.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "ticks_serde::opt"
    )]
    pub fade_in_ticks: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "ticks_serde::opt"
    )]
    pub fade_out_ticks: Option<i64>,
    pub content: ClipContent,
}

fn default_true() -> bool {
    true
}

/// A plugin slot on a track. `descriptor` is always present; `state` is
/// the opaque blob reference when the source carried one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSlot {
    pub instance_id: String,
    /// Slot index within the track's device chain (-1 = append).
    pub slot: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// False when the descriptor was recorded but the plugin is known
    /// missing (loaded="false" in dawproject terms). Kept through
    /// round-trips — a missing plugin is still preserved state.
    #[serde(default = "default_true")]
    pub present: bool,
    pub descriptor: PluginDescriptor,
    #[serde(default)]
    pub parameters: Vec<PluginParam>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<PluginStateRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginParam {
    /// Format-side parameter id (dawproject parameterID; Void keeps it a
    /// string to match `SetPluginParamOp.param_id`).
    pub param_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub value: PluginParamValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum PluginParamValue {
    Real(f64),
    Bool(bool),
    Integer(i64),
    /// Enum index; labels preserved when the source carried them.
    Enum {
        index: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        labels: Option<Vec<String>>,
    },
    /// Device-side time signature parameter — exists in the schema but is
    /// not expressible as a plugin param on the wire.
    TimeSignature {
        numerator: u32,
        denominator: u32,
    },
}

/// Reference to an opaque plugin-state blob inside the container `files`
/// map. Bytes live under `files[state.rel_path]`; the sha256 authenticates
/// them. `format_version` is the *state* format version the descriptor
/// declares (`PluginDescriptor::state_format_version` snapshot).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateRef {
    pub rel_path: String,
    pub sha256: String,
    /// Decimal string per the int64-string contract.
    pub bytes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_version: Option<String>,
}

/// Immutable asset descriptor carried in `assets[]`; payloads live in the
/// container `files` map keyed by `rel_path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRef {
    pub asset_id: String,
    /// Container-relative media path (`media/<name>` in dawproject terms,
    /// `assets/sha256/<hash>` on the VOID side). Safe-relative always.
    pub rel_path: String,
    pub media_type: String,
    pub sha256: String,
    pub bytes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "ticks_serde::opt"
    )]
    pub duration_ticks: Option<i64>,
    /// True when the descriptor exists but the payload was absent/unusable
    /// in the source container (external ref, unreadable, oversized).
    /// The clip and its timing are preserved; the audio is not.
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub kind: TrackKind,
    /// Linear gain (1.0 = unity), finite.
    #[serde(default = "default_gain")]
    pub gain_linear: f64,
    /// -1.0..=1.0, finite.
    #[serde(default)]
    pub pan: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub soloed: bool,
    /// Arrangement order is list order.
    #[serde(default)]
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub plugins: Vec<PluginSlot>,
}

fn default_gain() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeDocument {
    /// `EXCHANGE_FORMAT` — written always, validated on read.
    pub format: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub sample_rate: u32,
    /// Piecewise-constant tempo map, sorted by tick, unique positions.
    /// Empty on read means "unspecified" (callers use initial_bpm).
    #[serde(default)]
    pub tempo_map: Vec<TempoPoint>,
    #[serde(default)]
    pub time_signatures: Vec<TimeSignaturePoint>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_range: Option<LoopRange>,
    /// Immutable payload descriptors referenced by clips.
    #[serde(default)]
    pub assets: Vec<AssetRef>,
}

impl ExchangeDocument {
    pub fn new(name: impl Into<String>, sample_rate: u32) -> Self {
        Self {
            format: EXCHANGE_FORMAT.to_string(),
            name: name.into(),
            comment: None,
            sample_rate,
            tempo_map: Vec::new(),
            time_signatures: Vec::new(),
            tracks: Vec::new(),
            markers: Vec::new(),
            loop_range: None,
            assets: Vec::new(),
        }
    }

    /// Structural validation — returns every problem found (an import
    /// report should show them all, not the first). Type-level guarantees
    /// (finite f64, u8 ranges) are checked where they exist.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut bad = |m: String| errors.push(m);

        if self.format != EXCHANGE_FORMAT {
            bad(format!(
                "format must be {EXCHANGE_FORMAT:?}, got {:?}",
                self.format
            ));
        }
        if self.name.is_empty() {
            bad("name is empty".into());
        }
        if self.sample_rate == 0 || self.sample_rate > 768_000 {
            bad(format!("sampleRate {} out of range", self.sample_rate));
        }

        let mut prev_tick = None;
        for (i, tp) in self.tempo_map.iter().enumerate() {
            if !tp.bpm.is_finite() || tp.bpm <= 0.0 {
                bad(format!(
                    "tempoMap[{i}] bpm {} is not finite-positive",
                    tp.bpm
                ));
            }
            if let Some(p) = prev_tick {
                if tp.at_ticks <= p {
                    bad(format!(
                        "tempoMap[{i}] atTicks {} not strictly increasing after {}",
                        tp.at_ticks, p
                    ));
                }
            }
            prev_tick = Some(tp.at_ticks);
        }
        let mut prev_ts = None;
        for (i, ts) in self.time_signatures.iter().enumerate() {
            if ts.numerator == 0
                || ts.numerator > 255
                || ts.denominator == 0
                || ts.denominator > 255
            {
                bad(format!(
                    "timeSignatures[{i}] {}/{} out of u8 range",
                    ts.numerator, ts.denominator
                ));
            }
            if let Some(p) = prev_ts {
                if ts.at_ticks <= p {
                    bad(format!(
                        "timeSignatures[{i}] atTicks {} not strictly increasing",
                        ts.at_ticks
                    ));
                }
            }
            prev_ts = Some(ts.at_ticks);
        }
        if let Some(lr) = &self.loop_range {
            if lr.end_ticks <= lr.start_ticks {
                bad(format!(
                    "loopRange end {} must exceed start {}",
                    lr.end_ticks, lr.start_ticks
                ));
            }
        }

        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for t in &self.tracks {
            if !seen.insert(t.id.as_str()) {
                bad(format!("duplicate track id {:?}", t.id));
            }
            if !t.gain_linear.is_finite() || t.gain_linear < 0.0 {
                bad(format!(
                    "track {:?} gain {} not finite-nonnegative",
                    t.id, t.gain_linear
                ));
            }
            if !t.pan.is_finite() || !(-1.0..=1.0).contains(&t.pan) {
                bad(format!("track {:?} pan {} out of -1..1", t.id, t.pan));
            }
            for c in &t.clips {
                if !seen.insert(c.id.as_str()) {
                    bad(format!("duplicate clip id {:?}", c.id));
                }
                if c.length_ticks <= 0 {
                    bad(format!(
                        "clip {:?} lengthTicks {} not positive",
                        c.id, c.length_ticks
                    ));
                }
                if c.offset_ticks < 0 {
                    bad(format!(
                        "clip {:?} offsetTicks {} negative",
                        c.id, c.offset_ticks
                    ));
                }
                if let ClipContent::Notes { notes } = &c.content {
                    for n in notes {
                        if n.pitch > 127 {
                            bad(format!("note {:?} pitch {} > 127", n.id, n.pitch));
                        }
                        if n.velocity == 0 || n.velocity > 127 {
                            bad(format!(
                                "note {:?} velocity {} out of 1..127",
                                n.id, n.velocity
                            ));
                        }
                        if n.channel > 15 {
                            bad(format!("note {:?} channel {} > 15", n.id, n.channel));
                        }
                        if n.length_ticks <= 0 {
                            bad(format!(
                                "note {:?} lengthTicks {} not positive",
                                n.id, n.length_ticks
                            ));
                        }
                    }
                }
                if let ClipContent::Audio { asset_id } = &c.content {
                    if !self.assets.iter().any(|a| a.asset_id == *asset_id) {
                        bad(format!(
                            "clip {:?} references unknown asset {:?}",
                            c.id, asset_id
                        ));
                    }
                }
            }
            for p in &t.plugins {
                if !seen.insert(p.instance_id.as_str()) {
                    bad(format!("duplicate plugin instance id {:?}", p.instance_id));
                }
            }
        }
        for a in &self.assets {
            if !crate::dawproject::is_safe_rel_path(&a.rel_path) {
                bad(format!(
                    "asset {:?} relPath {:?} is not safe-relative",
                    a.asset_id, a.rel_path
                ));
            }
            if a.sha256.len() != 64
                || !a
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                bad(format!("asset {:?} sha256 malformed", a.asset_id));
            }
            if a.bytes.parse::<u64>().is_err() {
                bad(format!("asset {:?} bytes not a decimal u64", a.asset_id));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
