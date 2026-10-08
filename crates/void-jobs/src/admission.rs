//! §2 AI queue admission: "Four waiting jobs, one heavy worker and up to
//! two small jobs only when resource reservations permit" (T50).
//!
//! Admission is a pure read of the jobs index — the coordinator calls
//! `decide` before `start`, so policy lives in one place and the state
//! machine stays the authority. Heavy = reserves VRAM, or >4GiB RAM, or
//! >4 threads — inference-ish work. Small jobs interleave only when the
//! running heavy's reservations leave room (approximated by the heavy
//! count: the §2 policy is about not starving the audio thread, and a
//! single heavy already bounds its own footprint by reservation).

use crate::db::JobDb;
use crate::error::{JobError, Result};
use crate::job::{JobRecord, JobSpec, JobStatus};

/// §2 limits.
pub const MAX_WAITING: usize = 4;
pub const MAX_HEAVY_RUNNING: usize = 1;
pub const MAX_SMALL_RUNNING: usize = 2;

/// Rough heaviness per §2/T50: VRAM or big reservations = heavy.
pub fn is_heavy(spec: &JobSpec) -> bool {
    let ram: u64 = spec.reservations.ram_bytes.parse().unwrap_or(0);
    let vram: u64 = spec.reservations.vram_bytes.parse().unwrap_or(0);
    vram > 0 || ram > 4 * 1024 * 1024 * 1024 || spec.reservations.cpu_threads > 4
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admit {
    /// Submit + start now.
    StartNow,
    /// Admitted to the queue; a running slot frees up later.
    Queue,
    /// Waiting room is full — the caller must refuse or drop.
    Full,
}

/// Decide for a spec that has NOT been submitted yet.
/// Audio work is never gated by inference jobs — audio-lane tasks do
/// not go through this queue at all (T50: "audio is not gated").
pub fn decide(db: &JobDb, spec: &JobSpec) -> Result<Admit> {
    let jobs = db.nonterminal()?;
    let waiting = jobs.iter().filter(|j| j.status == JobStatus::Queued).count();
    let running: Vec<&JobRecord> = jobs
        .iter()
        .filter(|j| matches!(j.status, JobStatus::Running | JobStatus::Cancelling))
        .collect();
    let heavy_running = running.iter().filter(|j| is_heavy(&j.spec)).count();
    let small_running = running.len() - heavy_running;

    let heavy = is_heavy(spec);
    let slot = if heavy {
        heavy_running < MAX_HEAVY_RUNNING
    } else {
        small_running < MAX_SMALL_RUNNING
    };
    if slot {
        return Ok(Admit::StartNow);
    }
    if waiting < MAX_WAITING {
        return Ok(Admit::Queue);
    }
    Err(JobError::Busy("job queue full (4 waiting)".into()))
}
