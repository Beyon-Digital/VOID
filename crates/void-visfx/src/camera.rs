//! Camera-conducting policy model (T86, GEST-04). The hand tracker is
//! optional/local-only (AI-09): this crate owns the policy + session
//! state machine + emitted control events; the native box supplies the
//! actual camera/landmarks (recorded as NEEDS). No raw frame or
//! embedding may be retained or uploaded anywhere — enforced as policy
//! assertions, not trust.

use crate::error::{PolicyViolation, Result, VisFxError};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// Consent gate states — conducting cannot start until Granted, and
/// any transition back to NotAsked/Denied revokes mid-session (T86
/// deny + revoke).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentState {
    NotAsked,
    Granted,
    Denied,
    /// Was granted then revoked — same refusal path as Denied.
    Revoked,
}

impl ConsentState {
    pub fn allows_camera(self) -> bool {
        matches!(self, Self::Granted)
    }
}

/// Privacy assertions the runtime must enforce — declared as policy
/// fields (NEVER trust a runtime that can't show these). Default is
/// the maximum-privacy posture; loosening requires editing policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraAsserts {
    /// Raw camera frames/audio are processed in-place, discarded
    /// immediately — never written to disk, never kept beyond the
    /// frame's processing window.
    pub no_raw_retention: bool,
    /// No upload path may exist — camera data never leaves the box
    /// (network asserts enforced by the sandboxed tracker worker).
    pub no_network_upload: bool,
    /// Cap on retained landmark/control events (ring buffer); 0 =
    /// keep nothing beyond the current frame.
    pub max_landmark_history: u32,
    /// On occlusion (anything other than a hand on the lens), every
    /// frame is dropped — never processed (hands-only retraction, T86).
    pub drop_occluded_frames: bool,
    /// When tracking is lost, release-held runs within this window.
    pub loss_release_window_ns: u64,
}

impl Default for CameraAsserts {
    fn default() -> Self {
        Self {
            no_raw_retention: true,
            no_network_upload: true,
            max_landmark_history: 256,
            drop_occluded_frames: true,
            loss_release_window_ns: 750_000_000, // 750ms
        }
    }
}

/// Full conducting policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraPolicy {
    pub asserts: CameraAsserts,
    /// Min landmark confidence to emit a control event (AI-09:
    /// dropped/low-confidence frames must not leave stuck notes).
    pub min_confidence: f32,
    /// Frames under `min_confidence` (or empty frames) tolerated
    /// before a tracking-loss begins. Frames >= this threshold count
    /// as "low-confidence tracking" — after `loss_release_window_ns`
    /// of continuous loss the session emits release-all.
    pub drop_below_confidence: f32,
    /// One-euro-style smoothing window (frames) applied to landmark
    /// positions before gesture mapping. 1 = no smoothing.
    pub smoothing_window: u32,
    /// Clutch: conducting only engages while the clutch gesture is
    /// held; released clutch freezes held notes (they stay on until
    /// re-engaged or tracking lost — clutch is a gate, not a kill).
    pub clutch_required: bool,
    /// Max concurrent held controls (harmonic bound).
    pub max_held: u32,
}

impl Default for CameraPolicy {
    fn default() -> Self {
        Self {
            asserts: CameraAsserts::default(),
            min_confidence: 0.5,
            drop_below_confidence: 0.25,
            smoothing_window: 4,
            clutch_required: true,
            max_held: 12,
        }
    }
}

impl CameraPolicy {
    /// Violations that would make the policy dishonest — the runtime
    /// must be able to enforce every assert.
    pub fn violations(&self) -> Vec<PolicyViolation> {
        let mut v = Vec::new();
        if !self.asserts.no_raw_retention {
            v.push(PolicyViolation::RawRetention);
        }
        if !self.asserts.no_network_upload {
            v.push(PolicyViolation::NetworkUpload);
        }
        if self.asserts.max_landmark_history > 4096 {
            v.push(PolicyViolation::UnboundedLandmarkHistory);
        }
        if !(0.0..=1.0).contains(&self.min_confidence) || self.min_confidence < 0.2 {
            v.push(PolicyViolation::WeakConfidence);
        }
        if self.asserts.loss_release_window_ns == 0
            || self.asserts.loss_release_window_ns > 5_000_000_000
        {
            v.push(PolicyViolation::NoLossTimeout);
        }
        v
    }

    pub fn validate(&self) -> Result<()> {
        let v = self.violations();
        if let Some(first) = v.into_iter().next() {
            return Err(VisFxError::PolicyViolation(format!("{first:?}")));
        }
        if !(0.0..=1.0).contains(&self.drop_below_confidence)
            || self.drop_below_confidence >= self.min_confidence
        {
            return Err(VisFxError::PolicyViolation(
                "dropBelowConfidence must be < minConfidence".into(),
            ));
        }
        if self.smoothing_window == 0 || self.smoothing_window > 120 {
            return Err(VisFxError::PolicyViolation("smoothing_window 1..120".into()));
        }
        if self.max_held == 0 || self.max_held > 64 {
            return Err(VisFxError::PolicyViolation("max_held 1..64".into()));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Calibration record — measured facts the user confirmed before
// conducting may engage (T86 calibration).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalibrationRecord {
    /// RFC3339 when calibration completed.
    pub calibrated_at: String,
    /// Model id that produced landmarks (AI-09 family).
    pub tracker_model_id: String,
    pub tracker_model_sha256: String,
    /// Camera/sample facts: fps the pipeline actually sustained.
    pub sample_fps_num: u32,
    pub sample_fps_den: u32,
    /// Measured landmark confidence during calibration (0..1).
    pub measured_confidence: f32,
    /// Normalized palm-to-canvas bounds the user swept (min/max x,y).
    pub bounds: [f32; 4],
    /// Latency measured landmark→control event (ms, integer).
    pub measured_latency_ms: u32,
}

impl CalibrationRecord {
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| VisFxError::InvalidSpec(m.into());
        if self.calibrated_at.is_empty() {
            return Err(bad("calibratedAt empty"));
        }
        if self.tracker_model_id.is_empty() || self.tracker_model_sha256.len() != 64 {
            return Err(bad("tracker model id/sha256"));
        }
        if self.sample_fps_num == 0 || self.sample_fps_den == 0 {
            return Err(bad("sample fps"));
        }
        if !(0.0..=1.0).contains(&self.measured_confidence) {
            return Err(bad("measured_confidence 0..1"));
        }
        for (i, b) in self.bounds.iter().enumerate() {
            if !(0.0..=1.0).contains(b) {
                return Err(bad(&format!("bounds[{i}] 0..1")));
            }
        }
        if self.bounds[0] >= self.bounds[2] || self.bounds[1] >= self.bounds[3] {
            return Err(bad("bounds must be min<max"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Inputs (tracker → session) + control events (session → conductor)
// ---------------------------------------------------------------------------

/// A gesture the tracker (AI-09 class) decoded on the native box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GestureKind {
    /// Clutch gesture (e.g. fist) — engages conducting.
    Clutch,
    /// Open palm — positional conducting.
    OpenPalm,
    /// Pinch — note onset (strength in `arg`).
    Pinch,
    /// Spread fingers — release gesture.
    Release,
    /// Unmapped/unknown.
    Other(String),
}

/// One tracker frame delivered to the policy machine. Raw pixels never
/// reach this type — by policy construction the session only ever sees
/// landmarks/confidence (T86 no raw retention).
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerFrame {
    /// Monotonic ns (engine clock).
    pub t_ns: u64,
    /// Detection confidence 0..1.
    pub confidence: f32,
    /// Gesture classification.
    pub gesture: GestureKind,
    /// Normalized hand position (x,y ∈ 0..1).
    pub x: f32,
    pub y: f32,
    /// Gesture strength (e.g. pinch velocity) 0..1.
    pub arg: f32,
    /// True when the lens is occluded by something that is NOT a hand —
    /// per T86 these frames are dropped unprocessed.
    pub occluded: bool,
}

/// Why a frame was dropped — surfaced for tests/UI honesty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    Occluded,
    BelowConfidence,
}

/// Control events emitted by the session — music events only, never
/// video/frames (GEST-04).
#[derive(Debug, Clone, PartialEq)]
pub enum ControlEvent {
    /// Conducting engaged (consent + calibration + clutch).
    Engaged,
    /// Conducting disengaged; `notes_on` controls stay held.
    Disengaged,
    /// Note/control on: id is the control slot (pitch-mapped).
    ControlOn { id: String, velocity: f32 },
    /// Param update for a held control (pitch-bend class).
    ControlMove { id: String, x: f32, y: f32 },
    /// Single control off.
    ControlOff { id: String },
    /// Tracking-loss release: EVERY held control off at once — the
    /// T86 "release the notes" event (never stuck notes).
    ReleaseAll { cause: ReleaseCause },
    /// Frame dropped per policy (counted for tests/UI).
    FrameDropped { reason: DropReason },
    /// Policy refused camera use at ingest time.
    PolicyRefused { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCause {
    TrackingLost,
    ConsentRevoked,
    SessionEnded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionState {
    Idle,
    Calibrated,
    Armed,
    Engaged,
    Ended,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Policy-enforced conducting session. Emits control events; never
/// holds raw frames. `landmark_log` is a bounded ring (policy cap) —
/// retention of LANDMARK events (facts), never pixels.
pub struct ConductorSession {
    policy: CameraPolicy,
    consent: ConsentState,
    state: SessionState,
    calibration: Option<CalibrationRecord>,
    clutch_down: bool,
    /// loss start (monotonic ns) — None while tracking healthy.
    loss_since: Option<u64>,
    /// held control slots (max_held cap).
    held: Vec<String>,
    /// smoothing ring of recent positions.
    window: Vec<(f32, f32)>,
    /// bounded landmark/control event log (facts only).
    log: Vec<ControlEvent>,
    dropped: u64,
}

impl ConductorSession {
    pub fn new(policy: CameraPolicy) -> Result<Self> {
        policy.validate()?;
        Ok(Self {
            policy,
            consent: ConsentState::NotAsked,
            state: SessionState::Idle,
            calibration: None,
            clutch_down: false,
            loss_since: None,
            held: Vec::new(),
            window: Vec::new(),
            log: Vec::new(),
            dropped: 0,
        })
    }

    fn emit(&mut self, ev: ControlEvent) -> ControlEvent {
        // bounded landmark history — facts only, never raw frames
        let cap = self.policy.asserts.max_landmark_history as usize;
        if cap == 0 {
            self.log.clear();
        } else if self.log.len() >= cap {
            self.log.remove(0);
        }
        if cap > 0 {
            self.log.push(ev.clone());
        }
        ev
    }

    pub fn event_log(&self) -> &[ControlEvent] {
        &self.log
    }
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }
    pub fn held(&self) -> &[String] {
        &self.held
    }

    // -- consent gate --------------------------------------------------------

    /// User granted camera consent. No camera ingest may start before
    /// this; starting without it emits PolicyRefused and no event.
    pub fn grant_consent(&mut self) -> Result<()> {
        self.consent = ConsentState::Granted;
        if self.state == SessionState::Idle && self.calibration.is_some() {
            self.state = SessionState::Armed;
        }
        Ok(())
    }

    /// Deny — the refusal path: no camera, session stays Idle/Ended.
    pub fn deny_consent(&mut self) -> Vec<ControlEvent> {
        self.consent = ConsentState::Denied;
        vec![self.emit(ControlEvent::PolicyRefused {
            reason: "consent_denied".into(),
        })]
    }

    /// Revoke mid-session — T86: immediate release-all + disarm.
    pub fn revoke_consent(&mut self, now_ns: u64) -> Vec<ControlEvent> {
        let _ = now_ns;
        self.consent = ConsentState::Revoked;
        self.release_all(ReleaseCause::ConsentRevoked)
    }

    /// Attach a validated calibration — required before engaging.
    pub fn calibrate(&mut self, rec: CalibrationRecord) -> Result<()> {
        rec.validate()?;
        self.calibration = Some(rec);
        if self.state == SessionState::Idle && self.consent.allows_camera() {
            self.state = SessionState::Armed;
        } else if self.state == SessionState::Idle {
            self.state = SessionState::Calibrated;
        }
        Ok(())
    }

    // -- frame ingest ---------------------------------------------------------

    /// Process one tracker frame. Emits events per policy; occluded or
    /// low-confidence frames are dropped (counted, never processed).
    /// Gating: consent + calibration + clutch + confidence + smooth.
    pub fn ingest(&mut self, f: &TrackerFrame) -> Vec<ControlEvent> {
        let mut out = Vec::new();
        if self.state == SessionState::Ended {
            out.push(self.emit(ControlEvent::PolicyRefused {
                reason: "session_ended".into(),
            }));
            return out;
        }
        // occlusion — hands-only retraction: frame discarded unprocessed.
        if f.occluded && self.policy.asserts.drop_occluded_frames {
            self.dropped += 1;
            out.push(self.emit(ControlEvent::FrameDropped {
                reason: DropReason::Occluded,
            }));
            self.track_loss(f.t_ns, &mut out);
            return out;
        }
        // consent gate — refuse before any processing.
        if !self.consent.allows_camera() {
            out.push(self.emit(ControlEvent::PolicyRefused {
                reason: "no_consent".into(),
            }));
            return out;
        }
        if self.calibration.is_none() {
            out.push(self.emit(ControlEvent::PolicyRefused {
                reason: "not_calibrated".into(),
            }));
            return out;
        }
        // confidence gate — low-confidence frames dropped, counted as
        // tracking degradation (AI-09: don't fire phantom controls).
        if f.confidence < self.policy.drop_below_confidence {
            self.dropped += 1;
            out.push(self.emit(ControlEvent::FrameDropped {
                reason: DropReason::BelowConfidence,
            }));
            self.track_loss(f.t_ns, &mut out);
            return out;
        }
        // healthy frame: clear loss state.
        self.loss_since = None;

        let (sx, sy) = self.smooth(f.x, f.y);

        match f.gesture.clone() {
            GestureKind::Clutch => {
                if !self.clutch_down {
                    self.clutch_down = true;
                    if self.state != SessionState::Engaged {
                        self.state = SessionState::Engaged;
                        out.push(self.emit(ControlEvent::Engaged));
                    }
                }
            }
            GestureKind::Release => {
                if self.clutch_down {
                    self.clutch_down = false;
                    self.state = SessionState::Armed;
                    out.push(self.emit(ControlEvent::Disengaged));
                }
            }
            GestureKind::Pinch => {
                if self.gate_open() && f.confidence >= self.policy.min_confidence {
                    let id = self.slot_for(sx, sy);
                    if !self.held.contains(&id) {
                        if self.held.len() >= self.policy.max_held as usize {
                            // harmonic bound: refuse extra onsets.
                            out.push(self.emit(ControlEvent::PolicyRefused {
                                reason: "max_held".into(),
                            }));
                            return out;
                        }
                        self.held.push(id.clone());
                    }
                    out.push(self.emit(ControlEvent::ControlOn {
                        id,
                        velocity: f.arg.clamp(0.0, 1.0),
                    }));
                } else {
                    self.dropped += 1;
                    out.push(self.emit(ControlEvent::FrameDropped {
                        reason: DropReason::BelowConfidence,
                    }));
                }
            }
            GestureKind::OpenPalm => {
                if self.gate_open() && f.confidence >= self.policy.min_confidence {
                    if let Some(id) = self.nearest_held(sx, sy) {
                        out.push(self.emit(ControlEvent::ControlMove { id, x: sx, y: sy }));
                    }
                }
            }
            GestureKind::Other(_) => {}
        }
        out
    }

    fn gate_open(&self) -> bool {
        !self.policy.clutch_required || (self.clutch_down && self.state == SessionState::Engaged)
    }

    /// Map a smoothed position to a control slot (pitch lattice —
    /// deterministic 12-slot quantization of the calibrated bounds).
    fn slot_for(&self, x: f32, y: f32) -> String {
        let cal = self.calibration.as_ref();
        let (nx, ny) = match cal {
            Some(c) => (
                ((x - c.bounds[0]) / (c.bounds[2] - c.bounds[0]).max(1e-6)).clamp(0.0, 1.0),
                ((y - c.bounds[1]) / (c.bounds[3] - c.bounds[1]).max(1e-6)).clamp(0.0, 1.0),
            ),
            None => (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)),
        };
        let semis = (nx * 11.0).round() as i32;
        let oct = (ny * 3.0).floor() as i32;
        format!("n{}:{}", oct, semis)
    }

    fn nearest_held(&self, x: f32, y: f32) -> Option<String> {
        let _ = (x, y);
        self.held.last().cloned()
    }

    /// rolling mean over the smoothing window (bounded by policy).
    fn smooth(&mut self, x: f32, y: f32) -> (f32, f32) {
        self.window.push((x, y));
        let n = self.policy.smoothing_window as usize;
        if self.window.len() > n {
            self.window.remove(0);
        }
        let (sx, sy) = self
            .window
            .iter()
            .fold((0f32, 0f32), |(ax, ay), (x, y)| (ax + x, ay + y));
        let k = self.window.len().max(1) as f32;
        (sx / k, sy / k)
    }

    /// Tracking-loss bookkeeping: after loss_release_window_ns of
    /// continuous loss, release everything held (T86).
    fn track_loss(&mut self, now_ns: u64, out: &mut Vec<ControlEvent>) {
        match self.loss_since {
            Some(t0)
                if now_ns.saturating_sub(t0) >= self.policy.asserts.loss_release_window_ns =>
            {
                out.extend(self.release_all(ReleaseCause::TrackingLost));
            }
            Some(_) => {}
            None => self.loss_since = Some(now_ns),
        }
    }

    /// End the session — release everything, no more ingest.
    pub fn end(&mut self) -> Vec<ControlEvent> {
        self.state = SessionState::Ended;
        self.release_all(ReleaseCause::SessionEnded)
    }

    fn release_all(&mut self, cause: ReleaseCause) -> Vec<ControlEvent> {
        let mut out = Vec::new();
        if !self.held.is_empty() {
            self.held.clear();
            out.push(self.emit(ControlEvent::ReleaseAll { cause }));
        }
        if self.state != SessionState::Ended {
            self.state = SessionState::Idle;
            self.clutch_down = false;
            self.loss_since = None;
            self.window.clear();
        } else {
            self.clutch_down = false;
            self.loss_since = None;
            self.window.clear();
        }
        out
    }
}
