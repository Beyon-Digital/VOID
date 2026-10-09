//! Stem-batch export planning (W21 T80-side; OUT-02, OUT-03).
//!
//! Produces one `void_export::ExportSpec` per stem (track or bus) plus
//! a plan manifest. Everything is a spec — the actual renders ride the
//! existing `ExportRunner`/job queue. Shared invariants enforced here:
//! every stem shares the same range/format/tempo map (stems must line
//! up), output names are derived deterministically and sanitized through
//! the export crate's own rules, and the total frame plan is identical
//! across stems by construction.

use crate::error::{ProducerError, Result};
use serde::{Deserialize, Serialize};
use void_export::{
    BitDepth, ChannelLayout, ExportFormat, ExportSpec, FramePlan, TailPolicy, TempoSegment,
};

/// A named stem member: one track or bus routed to the export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StemSource {
    /// Track or bus id (UUID-shaped string; validated for id shape).
    pub source_id: String,
    pub name: String,
    pub kind: StemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StemKind {
    Track,
    Bus,
}

/// Shared render parameters every stem inherits (the "line up" rule).
#[derive(Debug, Clone)]
pub struct StemBatchTemplate {
    pub project_id: String,
    pub checkpoint_id: String,
    pub source_revision: String,
    /// [start, end) ticks, end > start.
    pub range_start_ticks: i64,
    pub range_end_ticks: i64,
    pub tempo_map: Vec<TempoSegment>,
    pub sample_rate: u32,
    pub bit_depth: BitDepth,
    pub channels: ChannelLayout,
    pub tail: TailPolicy,
    /// Asset hashes the render may read (checkpoint's manifest list).
    pub asset_hashes: Vec<String>,
}

/// The finished plan: one ExportSpec per source + a manifest binding
/// them (deterministic job ids: job_id = uuid5-ish derived from
/// checkpoint+source so a replanned batch diffs cleanly).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StemBatchPlan {
    pub plan_id: String,
    pub project_id: String,
    pub checkpoint_id: String,
    pub range_start_ticks: String,
    pub range_end_ticks: String,
    /// Frame plan shared by every member — computed once, asserted
    /// identical per stem (they share range+map+rate by construction).
    pub frame_plan: FramePlan,
    pub members: Vec<StemMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StemMember {
    pub source: StemSource,
    pub spec: ExportSpec,
}

/// Deterministic UUID for a stem job: uuid v5 over (checkpoint, source).
fn member_job_id(checkpoint: &str, source: &str) -> String {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("void-stem:{checkpoint}:{source}").as_bytes(),
    )
    .to_string()
}

/// Build the batch: validates shared invariants (sample rate is one of
/// the approved set; range ordered; tempo map covers the range — all
/// checked by ExportSpec::validate per member, plus a plan-level
/// frame-plan equality assert).
pub fn plan_stem_batch(
    template: &StemBatchTemplate,
    sources: &[StemSource],
    output_prefix: &str,
) -> Result<StemBatchPlan> {
    if sources.is_empty() {
        return Err(ProducerError::InvalidSpec("empty stem list".into()));
    }
    if sources.len() > 64 {
        return Err(ProducerError::InvalidSpec("stem batch > 64".into()));
    }
    let mut names = std::collections::HashSet::new();
    let mut members = Vec::with_capacity(sources.len());
    let mut shared_plan: Option<FramePlan> = None;
    for s in sources {
        if s.source_id.trim().is_empty() || s.name.trim().is_empty() {
            return Err(ProducerError::InvalidSpec("blank stem id/name".into()));
        }
        // Map the stem name onto the scoped-filename charset, then run
        // the export crate's own validation — unsafe chars never reach
        // a spec.
        let safe: String = s
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let base = void_export::sanitize_output_name(&format!("{output_prefix}_{safe}"))?;
        let spec = ExportSpec {
            job_id: member_job_id(&template.checkpoint_id, &s.source_id),
            project_id: template.project_id.clone(),
            checkpoint_id: template.checkpoint_id.clone(),
            source_revision: template.source_revision.clone(),
            asset_hashes: template.asset_hashes.clone(),
            range_start_ticks: template.range_start_ticks.to_string(),
            range_end_ticks: template.range_end_ticks.to_string(),
            format: ExportFormat::Wav,
            channels: Some(template.channels),
            bit_depth: Some(template.bit_depth),
            sample_rate: Some(template.sample_rate),
            tail: template.tail,
            tempo_map: template.tempo_map.clone(),
            output_name: base.clone(),
            deadline_monotonic_ns: None,
        };
        spec.validate()?;
        // Carry the stem's solo-scope in parameters via the job-spec
        // mapping later; the ExportSpec itself is pure render config.
        let plan = spec.frame_plan()?;
        if let Some(shared) = &shared_plan {
            if *shared != plan {
                return Err(ProducerError::InvalidSpec(format!(
                    "stem {:?} frame plan diverges",
                    s.source_id
                )));
            }
        } else {
            shared_plan = Some(plan.clone());
        }
        if !names.insert(spec.output_name.clone()) {
            return Err(ProducerError::InvalidSpec(format!(
                "duplicate output name after sanitize: {:?}",
                spec.output_name
            )));
        }
        members.push(StemMember {
            source: s.clone(),
            spec,
        });
    }
    Ok(StemBatchPlan {
        plan_id: member_job_id(&template.checkpoint_id, "plan"),
        project_id: template.project_id.clone(),
        checkpoint_id: template.checkpoint_id.clone(),
        range_start_ticks: template.range_start_ticks.to_string(),
        range_end_ticks: template.range_end_ticks.to_string(),
        frame_plan: shared_plan.expect("non-empty sources"),
        members,
    })
}

/// Persist the plan manifest next to the export outputs (JSON).
pub fn write_plan(plan: &StemBatchPlan, dir: &std::path::Path) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let p = dir.join(format!("stem-plan-{}.json", plan.plan_id));
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(plan)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use void_export::{BitDepth, ChannelLayout};

    fn template() -> StemBatchTemplate {
        StemBatchTemplate {
            project_id: uuid::Uuid::new_v4().to_string(),
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
            source_revision: "7".into(),
            range_start_ticks: 0,
            range_end_ticks: 960_000 * 8,
            tempo_map: vec![TempoSegment {
                at_ticks: "0".into(),
                bpm: 120.0,
            }],
            sample_rate: 48_000,
            bit_depth: BitDepth::Float32,
            channels: ChannelLayout::Stereo,
            tail: TailPolicy::Milliseconds { ms: 250 },
            asset_hashes: vec![],
        }
    }

    fn src(id: &str, name: &str) -> StemSource {
        StemSource {
            source_id: id.into(),
            name: name.into(),
            kind: StemKind::Track,
        }
    }

    #[test]
    fn batch_produces_specs_with_shared_frame_plan() {
        let t = template();
        let plan = plan_stem_batch(&t, &[src("t1", "drums"), src("t2", "bass")], "mix").unwrap();
        assert_eq!(plan.members.len(), 2);
        for m in &plan.members {
            assert_eq!(m.spec.format, ExportFormat::Wav);
            assert_eq!(m.spec.range_end_ticks, "7680000");
            let fp = m.spec.frame_plan().unwrap();
            assert_eq!(fp, plan.frame_plan);
        }
        assert_eq!(plan.members[0].spec.output_name, "mix_drums");
        // Deterministic job ids.
        let again = plan_stem_batch(&t, &[src("t1", "drums"), src("t2", "bass")], "mix").unwrap();
        assert_eq!(plan.members[0].spec.job_id, again.members[0].spec.job_id);
    }

    #[test]
    fn bad_rate_and_names_rejected() {
        let mut t = template();
        t.sample_rate = 44_100.min(0); // becomes 0 → invalid
        assert!(plan_stem_batch(&t, &[src("t1", "a")], "x").is_err());
        let t = template();
        // Names that sanitize to the same thing collide.
        assert!(plan_stem_batch(&t, &[src("a", "x y"), src("b", "x_y")], "p").is_err());
        assert!(plan_stem_batch(&template(), &[], "p").is_err());
    }
}
