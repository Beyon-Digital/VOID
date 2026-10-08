//! Streaming + voice-budget policy for sampled content (W19 / SND-03, T73).
//!
//! Two bounded resources, both declared by the pack manifest:
//!
//! - **Cache budget** (bytes): how much audio a pack may hold in memory —
//!   preload bodies + stream lookahead. A plan that needs more than the
//!   budget is rejected, not silently over-allocated.
//! - **Voice budget** (voices): how many concurrent triggers the pack may
//!   hold. Overflow is an explicit policy decision — deny or steal —
//!   never an unbounded allocation (CONTRACTS §2 spirit).
//!
//! The descriptor tells the engine HOW to source a file; the audio path
//! itself is engine-side (docs/content-rights/NEEDS.md).

use crate::error::{ContentError, Result};
use serde::{Deserialize, Serialize};

/// How a zone's audio is sourced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingMode {
    /// Whole file decoded into memory at load. For short one-shots.
    Preload,
    /// First `preload_frames` held in memory; the rest read from disk in
    /// `chunk_frames` increments. For long samples / large instruments.
    Stream,
    /// Never resident — rendered only inside an offline job (bounce,
    /// export) where the job budget, not the realtime path, bounds it.
    OfflineOnly,
}

/// Per-file streaming descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StreamingDescriptor {
    pub mode: StreamingMode,
    /// Frames decoded up-front (mode=stream) or total (mode=preload).
    pub preload_frames: u64,
    /// Disk read granularity for streamed content (0 when not streaming).
    pub chunk_frames: u64,
    /// Bytes per frame for budget math (channels × bytes-per-sample).
    pub bytes_per_frame: u32,
    /// Total frames in the source file.
    pub total_frames: u64,
}

impl StreamingDescriptor {
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| ContentError::InvalidManifest(m.into());
        if self.bytes_per_frame == 0 {
            return Err(bad("bytesPerFrame must be > 0"));
        }
        match self.mode {
            StreamingMode::Preload => {
                if self.preload_frames != self.total_frames {
                    return Err(bad("preload mode must cover the whole file"));
                }
            }
            StreamingMode::Stream => {
                if self.preload_frames == 0 || self.preload_frames >= self.total_frames {
                    return Err(bad("stream mode needs 0 < preload < total"));
                }
                if self.chunk_frames == 0 {
                    return Err(bad("stream mode needs chunkFrames > 0"));
                }
            }
            StreamingMode::OfflineOnly => {
                if self.preload_frames != 0 {
                    return Err(bad("offline content holds no preload"));
                }
            }
        }
        Ok(())
    }

    /// Bytes this descriptor permanently occupies in the cache.
    pub fn resident_bytes(&self) -> u64 {
        self.preload_frames
            .saturating_mul(self.bytes_per_frame as u64)
    }
}

/// A full streaming plan for an instrument: one descriptor per file.
/// `validate_against` enforces the pack cache budget.
#[derive(Debug, Clone)]
pub struct StreamingPlan {
    pub descriptors: Vec<StreamingDescriptor>,
}

impl StreamingPlan {
    /// Total resident bytes the plan needs.
    pub fn required_cache_bytes(&self) -> u64 {
        self.descriptors.iter().map(|d| d.resident_bytes()).sum()
    }

    /// Reject a plan that overruns the pack's declared cache budget.
    /// Bounded allocation is the contract; "needs more" is a pack-author
    /// error, not a runtime surprise.
    pub fn validate_against(&self, cache_budget_bytes: u64) -> Result<()> {
        for d in &self.descriptors {
            d.validate()?;
        }
        let required = self.required_cache_bytes();
        if required > cache_budget_bytes {
            return Err(ContentError::CacheBudgetExceeded {
                required,
                budget: cache_budget_bytes,
            });
        }
        Ok(())
    }
}

/// Overflow policy when the voice budget is hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceSteal {
    /// Refuse the trigger — no voice, caller surfaces "voice full".
    Deny,
    /// Take over the oldest voice (deterministic — lowest seq).
    StealOldest,
}

/// One live voice. Gain is hundredths of dB (exact); `seq` is the
/// allocation order used by StealOldest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    pub voice_id: u64,
    pub zone_id: String,
    pub key: u8,
    pub velocity: u8,
    pub seq: u64,
}

/// Bounded voice allocator. Holds at most `budget` voices; overflow
/// follows the declared policy. Deterministic: identical allocation
/// sequences give identical voice tables.
#[derive(Debug, Clone)]
pub struct VoiceAllocator {
    budget: u32,
    policy: VoiceSteal,
    voices: Vec<Voice>,
    next_seq: u64,
}

/// What happened to a note-on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceOutcome {
    /// Fresh voice allocated.
    Allocated(Voice),
    /// Budget full; the oldest voice was replaced.
    Stole { voice: Voice, evicted: Voice },
    /// Budget full and policy denies — nothing allocated.
    Denied,
}

impl VoiceAllocator {
    pub fn new(budget: u32, policy: VoiceSteal) -> Result<Self> {
        if budget == 0 {
            return Err(ContentError::InvalidManifest(
                "voice budget must be > 0".into(),
            ));
        }
        Ok(Self {
            budget,
            policy,
            voices: Vec::with_capacity(budget as usize),
            next_seq: 0,
        })
    }

    pub fn active(&self) -> usize {
        self.voices.len()
    }

    pub fn budget(&self) -> u32 {
        self.budget
    }

    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    /// Allocate a voice for a trigger on `zone_id`.
    pub fn note_on(&mut self, zone_id: &str, key: u8, velocity: u8) -> VoiceOutcome {
        let mk = |seq: u64| Voice {
            voice_id: seq,
            zone_id: zone_id.to_string(),
            key,
            velocity,
            seq,
        };
        if (self.voices.len() as u32) < self.budget {
            let v = mk(self.next_seq);
            self.next_seq += 1;
            self.voices.push(v.clone());
            return VoiceOutcome::Allocated(v);
        }
        match self.policy {
            VoiceSteal::Deny => VoiceOutcome::Denied,
            VoiceSteal::StealOldest => {
                // Oldest = lowest seq. Vec stays sorted by seq because
                // seq is monotone and we remove the min — use position
                // scan to be explicit rather than assuming order.
                let (pos, evicted) = self
                    .voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.seq)
                    .map(|(p, v)| (p, v.clone()))
                    .expect("budget>0 implies non-empty");
                let v = mk(self.next_seq);
                self.next_seq += 1;
                self.voices[pos] = v.clone();
                VoiceOutcome::Stole { voice: v, evicted }
            }
        }
    }

    /// Release a voice by id (note-off / group choke). No-op when absent.
    pub fn release(&mut self, voice_id: u64) -> bool {
        if let Some(pos) = self.voices.iter().position(|v| v.voice_id == voice_id) {
            self.voices.remove(pos);
            true
        } else {
            false
        }
    }

    /// Release every voice matching a zone — choke-group semantics.
    pub fn release_zone(&mut self, zone_id: &str) -> usize {
        let before = self.voices.len();
        self.voices.retain(|v| v.zone_id != zone_id);
        before - self.voices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(mode: StreamingMode, preload: u64, chunk: u64, total: u64) -> StreamingDescriptor {
        StreamingDescriptor {
            mode,
            preload_frames: preload,
            chunk_frames: chunk,
            bytes_per_frame: 4,
            total_frames: total,
        }
    }

    #[test]
    fn plan_budget_enforced() {
        let plan = StreamingPlan {
            descriptors: vec![
                desc(StreamingMode::Preload, 1000, 0, 1000),
                desc(StreamingMode::Stream, 2000, 512, 10000),
            ],
        };
        // 1000*4 + 2000*4 = 12000 bytes required.
        plan.validate_against(12000).unwrap();
        let err = plan.validate_against(11999).unwrap_err();
        assert!(matches!(err, ContentError::CacheBudgetExceeded { .. }));
    }

    #[test]
    fn stream_descriptor_shape() {
        assert!(desc(StreamingMode::Preload, 1000, 0, 2000)
            .validate()
            .is_err());
        assert!(desc(StreamingMode::Stream, 0, 512, 2000)
            .validate()
            .is_err());
        assert!(desc(StreamingMode::Stream, 500, 512, 2000)
            .validate()
            .is_ok());
        assert!(desc(StreamingMode::OfflineOnly, 8, 0, 2000)
            .validate()
            .is_err());
        assert!(desc(StreamingMode::OfflineOnly, 0, 0, 2000)
            .validate()
            .is_ok());
    }

    #[test]
    fn voice_budget_enforced_and_steal() {
        let mut a = VoiceAllocator::new(2, VoiceSteal::StealOldest).unwrap();
        let VoiceOutcome::Allocated(v1) = a.note_on("z", 60, 100) else {
            panic!()
        };
        let VoiceOutcome::Allocated(v2) = a.note_on("z", 62, 100) else {
            panic!()
        };
        match a.note_on("z", 64, 100) {
            VoiceOutcome::Stole { evicted, .. } => assert_eq!(evicted, v1),
            _ => panic!("expected steal"),
        }
        assert_eq!(a.active(), 2);
        a.release(v2.voice_id);
        assert_eq!(a.active(), 1);
    }

    #[test]
    fn deny_policy_refuses() {
        let mut a = VoiceAllocator::new(1, VoiceSteal::Deny).unwrap();
        a.note_on("z", 60, 100);
        assert_eq!(a.note_on("z", 62, 100), VoiceOutcome::Denied);
        assert_eq!(a.active(), 1);
    }
}
