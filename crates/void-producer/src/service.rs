//! Accompaniment request → `void_proposals` ProposalRecord glue (T78).
//!
//! The producer engine generates real note candidates; the *record*
//! layer stays `void_proposals`' own types so the existing accept/
//! reject/stale lifecycle, store layout, and InsertNoteOp plan apply
//! verbatim — reuse, no fork. This module is the adapter: spec →
//! `generate()` → `ProposalRecord` (status Ready, candidates ranked by
//! measured fit) persisted through `ProposalStore`.

use crate::engine::{generate, GenMode, GenerationSpec, GeneratedCandidate};
use crate::error::{ProducerError, Result};
use crate::engine::postcondition_check;
use crate::music::{locked_region_bytes, GenNote, Range, Scale};
use serde::Serialize;
use sha2::{Digest, Sha256};
use void_proposals::record::{
    Candidate, ProposalProvenance, ProposalRecord, ProposalStatus,
};
use void_proposals::store::{utc_now, ProposalStore};
use void_proposals::{NoteEvent, RegionContext};

/// Deterministic generator identity — no external model file, so the
/// model fields carry the crate version hash honestly (same trick as
/// void-export's runtime_sha256).
pub const GENERATOR_ID: &str = "void-producer/deterministic";

fn crate_sha256() -> String {
    let mut h = Sha256::new();
    h.update(format!("void-producer/{}", env!("CARGO_PKG_VERSION")).as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Convert a generated candidate to the record's Candidate shape.
fn to_record_candidate(rank: u32, c: &GeneratedCandidate) -> Candidate {
    Candidate {
        rank,
        // score = measured chord-tone fit — a real number from the
        // generated material, not a placeholder.
        score: c.chord_tone_ppm as f64 / 1_000_000.0,
        rationale: format!(
            "{:?} {:?} seed={} notes={} occupancy={}ppm chord-tone={}ppm",
            c.role,
            c.mode,
            c.seed,
            c.notes.len(),
            c.occupancy_ppm,
            c.chord_tone_ppm
        ),
        notes: c
            .notes
            .iter()
            .map(|n| void_proposals::record::ProposedNote {
                pitch: n.pitch,
                velocity: n.velocity,
                onset_ticks: n.onset.to_string(),
                length_ticks: n.length.to_string(),
            })
            .collect(),
    }
}

/// The service-facing request: everything GenerationSpec needs, still
/// in integer domain. `context` is the proposals-side RegionContext
/// (source of track/clip ids + locked ranges + key hint + seed notes).
pub struct AccompanimentRequest {
    pub project_id: String,
    pub source_revision: String,
    pub context: RegionContext,
    pub role: crate::engine::Role,
    pub mode: GenMode,
    /// If given, generation targets this window instead of
    /// `context.region` (inpaint gap / continuation window).
    pub output_range: Option<Range>,
    pub spec: GenerationSpecBase,
}

/// Fields of GenerationSpec that are NOT derivable from RegionContext.
#[derive(Debug, Clone)]
pub struct GenerationSpecBase {
    pub seed: Option<u64>,
    pub candidates: usize,
    pub scale: Option<Scale>,
    pub chords: Vec<crate::music::ChordEvent>,
    pub groove: crate::music::GrooveTemplate,
    pub density_ppm: u32,
    pub variation_ppm: u32,
}

fn parse_notes(notes: &[NoteEvent]) -> Result<Vec<GenNote>> {
    notes.iter().map(GenNote::from_dto).collect()
}

fn ctx_range(ctx: &RegionContext) -> Result<Range> {
    Range::new(ctx.region.start(), ctx.region.len())
}

/// Build the integer-domain GenerationSpec from a request +
/// RegionContext (locked ranges map 1:1; scale falls back to key_hint
/// then C major; seed falls back to context-hash first 8 bytes).
pub fn spec_for(req: &AccompanimentRequest) -> Result<GenerationSpec> {
    let ctx = &req.context;
    ctx.validate()?;
    let region = ctx_range(ctx)?;
    let output = req.output_range.unwrap_or(region);
    // Continuation uses the declared continuation window if present.
    let cont_start: i64 = ctx.continuation_start_ticks.parse().unwrap_or(0);
    let cont_len: i64 = ctx.continuation_ticks.parse().unwrap_or(0);
    let output = match req.mode {
        GenMode::Continue if cont_len > 0 => Range::new(cont_start, cont_len)?,
        _ => output,
    };
    let locked: Vec<Range> = ctx
        .locked_ranges
        .iter()
        .map(|t| Range::new(t.start(), t.len()))
        .collect::<Result<Vec<_>>>()?;
    let scale = match (req.spec.scale, &ctx.key_hint) {
        (Some(s), _) => s,
        (None, Some(h)) => {
            Scale::parse_key(h, crate::music::ScaleKind::Major)
                .unwrap_or(Scale { root_pc: 0, kind: crate::music::ScaleKind::Major })
        }
        (None, None) => Scale { root_pc: 0, kind: crate::music::ScaleKind::Major },
    };
    // Seed: explicit, else first 8 bytes of the context hash —
    // deterministic per-region by construction.
    let seed = match req.spec.seed {
        Some(s) => s,
        None => {
            let h = ctx.sha256();
            let mut b = [0u8; 8];
            b.copy_from_slice(&hex_bytes(&h)?[..8]);
            u64::from_be_bytes(b)
        }
    };
    Ok(GenerationSpec {
        seed,
        role: req.role,
        mode: req.mode,
        candidates: req.spec.candidates,
        output_range: output,
        context_notes: parse_notes(&ctx.notes)?,
        locked_ranges: locked,
        scale,
        chords: req.spec.chords.clone(),
        groove: req.spec.groove.clone(),
        density_ppm: req.spec.density_ppm,
        variation_ppm: req.spec.variation_ppm,
        tempo_bpm: ctx.tempo_bpm.max(1.0) as u32,
    })
}

fn hex_bytes(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ProducerError::InvalidRequest("context hash not hex".into()));
    }
    Ok((0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect())
}

/// Run generation and persist a Ready ProposalRecord via the proposals
/// store. Returns the record + the pre-apply source-region byte
/// snapshot for the caller's postcondition check.
///
/// The candidates are ranked by measured chord-tone fit, then seeded
/// order — a real ordering the audition list can show.
pub fn request_accompaniment(
    store: &ProposalStore,
    req: &AccompanimentRequest,
) -> Result<(ProposalRecord, Vec<u8>)> {
    let spec = spec_for(req)?;
    let candidates = generate(&spec)?;
    if candidates.is_empty() {
        return Err(ProducerError::InvalidRequest(
            "generation produced no candidates".into(),
        ));
    }
    // Rank by measured chord-tone fit (desc), tie-break by occupancy.
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| {
        candidates[b]
            .chord_tone_ppm
            .cmp(&candidates[a].chord_tone_ppm)
            .then(candidates[b].occupancy_ppm.cmp(&candidates[a].occupancy_ppm))
    });
    let mut recs = Vec::with_capacity(candidates.len());
    for (r, &i) in order.iter().enumerate() {
        recs.push(to_record_candidate((r + 1) as u32, &candidates[i]));
    }
    let context_sha = req.context.sha256();
    // Pre-apply snapshot of protected material — T78 byte-identity.
    let before = locked_region_bytes(
        &spec.context_notes,
        &spec.locked_ranges,
    );
    let now = utc_now();
    let spec_json = serde_json::to_value(&SpecEcho {
        role: format!("{:?}", spec.role).to_lowercase(),
        mode: format!("{:?}", spec.mode).to_lowercase(),
        density_ppm: spec.density_ppm,
        variation_ppm: spec.variation_ppm,
        output_start_ticks: spec.output_range.start.to_string(),
        output_length_ticks: spec.output_range.length.to_string(),
        candidate_seeds: order
            .iter()
            .map(|&i| candidates[i].seed.to_string())
            .collect(),
    })?;
    let rec = ProposalRecord {
        format_version: 1,
        proposal_id: uuid::Uuid::new_v4().to_string(),
        project_id: req.project_id.clone(),
        source_revision: req.source_revision.clone(),
        context_sha256: context_sha,
        context: req.context.clone(),
        status: ProposalStatus::Ready,
        stale_cause: None,
        candidates: recs,
        provenance: Some(ProposalProvenance {
            job_id: format!("local-{}", uuid::Uuid::new_v4().simple()),
            generator_id: GENERATOR_ID.into(),
            generator_version: env!("CARGO_PKG_VERSION").into(),
            model_id: "deterministic-rule-engine".into(),
            runtime_id: GENERATOR_ID.into(),
            runtime_sha256: crate_sha256(),
            seed: spec.seed.to_string(),
            // No document artifact — the note list IS the artifact;
            // hash the spec echo for the audit slot.
            document_sha256: {
                let mut h = Sha256::new();
                h.update(spec_json.to_string().as_bytes());
                h.finalize().iter().map(|b| format!("{b:02x}")).collect()
            },
            analysis: spec_json,
            document_asset: String::new(),
        }),
        accepted: None,
        supersedes: None,
        error: None,
        created_at: now.clone(),
        updated_at: now,
    };
    store.save(&rec)?;
    Ok((rec, before))
}

/// Spec echo kept under provenance.analysis — reproducibility record.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecEcho {
    role: String,
    mode: String,
    density_ppm: u32,
    variation_ppm: u32,
    output_start_ticks: String,
    output_length_ticks: String,
    candidate_seeds: Vec<String>,
}

/// Verify the source region is byte-identical after the caller applied
/// the accept plan. Call with the post-apply context notes.
pub fn verify_locked_unchanged(
    before_bytes: &[u8],
    context_notes_after: &[GenNote],
    locked: &[Range],
) -> Result<()> {
    let after = locked_region_bytes(context_notes_after, locked);
    postcondition_check(before_bytes, &after)
}

/// Convenience used by tests + studio bridge: accept through the
/// proposals crate (plan + optional indices), then verify the locked
/// invariant on the post-apply region (caller supplies it).
pub fn accept_plan_checked(
    rec: &ProposalRecord,
    rank: u32,
    indices: Option<&[usize]>,
    reval: &void_proposals::accept::Revalidation,
    drop_locked: bool,
    after_notes: &[GenNote],
    locked: &[Range],
    before_bytes: &[u8],
) -> Result<void_proposals::accept::AcceptPlan> {
    let plan = void_proposals::accept::plan_accept(rec, rank, indices, reval, drop_locked)?;
    verify_locked_unchanged(before_bytes, after_notes, locked)?;
    Ok(plan)
}
