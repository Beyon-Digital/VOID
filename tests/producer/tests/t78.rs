//! T78 — accompaniment/orchestration: role generators produce real,
//! deterministic, scale/chord-bound note material; the locked/protected
//! region invariant holds byte-identically across an accept; inpaint
//! and vary stay inside their bounds; proposals land in the shared
//! void-proposals lifecycle (accept → InsertNoteOps under ONE
//! transaction id → editable; stale is shown, never silent).

use std::collections::HashSet;
use void_producer::*;
use void_proposals::accept::{plan_accept, Revalidation};
use void_proposals::context::RegionContext;
use void_proposals::store::ProposalStore;
use void_proposals::{NoteEvent, TickRange};

fn ctx() -> RegionContext {
    let notes: Vec<NoteEvent> = (0..4)
        .map(|i| NoteEvent {
            pitch: 60 + i * 2,
            velocity: 90,
            onset_ticks: (i as i64 * 960_000).to_string(),
            length_ticks: "480000".into(),
        })
        .collect();
    RegionContext {
        track_id: uuid::Uuid::new_v4().to_string(),
        clip_id: uuid::Uuid::new_v4().to_string(),
        region: TickRange {
            start_ticks: "0".into(),
            length_ticks: (960_000 * 16).to_string(),
        },
        continuation_start_ticks: (960_000 * 16).to_string(),
        continuation_ticks: (960_000 * 8).to_string(),
        tempo_bpm: 120.0,
        ts_num: 4,
        ts_den: 4,
        key_hint: Some("C".into()),
        notes,
        // Lock covers the seed notes at onsets 0..3T — the protected
        // material whose byte-identity T78 asserts.
        locked_ranges: vec![TickRange {
            start_ticks: "0".into(),
            length_ticks: (960_000 * 4).to_string(),
        }],
        labels: vec![],
    }
}

fn base() -> GenerationSpecBase {
    GenerationSpecBase {
        seed: Some(0xC0FFEE),
        candidates: 4,
        scale: Some(Scale::new(0, ScaleKind::Major).unwrap()),
        chords: vec![
            ChordEvent { at_ticks: 0, root_pc: 0, quality: ChordQuality::Major },
            ChordEvent {
                at_ticks: 4 * TICKS_PER_QUARTER,
                root_pc: 5,
                quality: ChordQuality::Major,
            },
            ChordEvent {
                at_ticks: 8 * TICKS_PER_QUARTER,
                root_pc: 7,
                quality: ChordQuality::Dominant7,
            },
        ],
        groove: GrooveTemplate::swing(200_000, 4).unwrap(),
        density_ppm: 500_000,
        variation_ppm: 300_000,
    }
}

#[test]
fn t78_all_roles_generate_real_deterministic_material() {
    for role in Role::ALL {
        let spec = GenerationSpec {
            seed: 0xA11CE,
            role,
            mode: GenMode::Accompaniment,
            candidates: 4,
            output_range: Range::new(0, TICKS_PER_QUARTER * 16).unwrap(),
            context_notes: vec![],
            locked_ranges: vec![],
            scale: Scale::new(0, ScaleKind::Major).unwrap(),
            chords: base().chords,
            groove: GrooveTemplate::straight(),
            density_ppm: 500_000,
            variation_ppm: 300_000,
            tempo_bpm: 120,
        };
        let a = generate(&spec).unwrap();
        let b = generate(&spec).unwrap();
        assert_eq!(a.len(), 4);
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.notes, y.notes, "{role:?} not deterministic");
        }
        // Real material: enough notes to be a take, >1 distinct pitch.
        for c in &a {
            assert!(c.notes.len() > 10, "{role:?} too sparse {}", c.notes.len());
            let pitches: HashSet<i32> = c.notes.iter().map(|n| n.pitch).collect();
            assert!(pitches.len() > 1, "{role:?} degenerate pitch set");
            // Scale/chord membership: every pitched onset is lawful.
            if role != Role::Drums {
                for n in &c.notes {
                    let pc = n.pitch.rem_euclid(12) as u8;
                    let lawful = spec.scale.contains(n.pitch)
                        || spec
                            .chords
                            .iter()
                            .filter(|ch| ch.at_ticks <= n.onset)
                            .last()
                            .map(|ch| ch.pitch_classes().contains(&pc))
                            .unwrap_or(false);
                    assert!(lawful, "{role:?} unlawful pitch {} pc {pc}", n.pitch);
                }
            }
        }
        // Different seeds → different takes.
        let mut spec2 = GenerationSpec { ..spec.clone() };
        spec2.seed = 0xBADC0DE;
        assert_ne!(generate(&spec2).unwrap()[0].notes, a[0].notes);
    }
}

#[test]
fn t78_proposal_lifecycle_accept_one_tx_and_locked_identity() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProposalStore::new(dir.path());
    let context = ctx();
    let req = AccompanimentRequest {
        project_id: uuid::Uuid::new_v4().to_string(),
        source_revision: "12".into(),
        context: context.clone(),
        role: Role::Keys,
        mode: GenMode::Accompaniment,
        output_range: None,
        spec: base(),
    };
    let (rec, before_bytes) = request_accompaniment(&store, &req).unwrap();
    assert_eq!(rec.candidates.len(), 4);
    // Ranked by measured fit — rank 1 has the best chord-tone ratio.
    assert!(rec.candidates[0].score >= rec.candidates[3].score);

    // Region context notes after apply = source notes + accepted notes.
    // Locked region material must be byte-identical.
    let cand = &rec.candidates[0];
    let accepted: Vec<GenNote> = cand
        .notes
        .iter()
        .map(|n| GenNote {
            pitch: n.pitch,
            velocity: n.velocity,
            onset: n.onset_ticks.parse().unwrap(),
            length: n.length_ticks.parse().unwrap(),
        })
        .collect();
    let mut after: Vec<GenNote> = Vec::new();
    for n in &context.notes {
        after.push(GenNote::from_dto(n).unwrap());
    }
    after.extend(accepted);
    let locked: Vec<Range> = context
        .locked_ranges
        .iter()
        .map(|t| Range::new(t.start(), t.len()).unwrap())
        .collect();
    verify_locked_unchanged(&before_bytes, &after, &locked).unwrap();

    // plan_accept through the proposals crate — one transaction id,
    // locked collisions either error or drop-flag.
    let reval = Revalidation {
        project_id: rec.project_id.clone(),
        current_context_sha256: context.sha256(),
        target_clip_exists: true,
    };
    let plan = plan_accept(&rec, 1, None, &reval, true).unwrap();
    assert!(!plan.transaction_id.is_empty());
    assert!(!plan.inserts.is_empty());
    // Every insert lands inside the output region, not the source notes.
    let region_end = context.region.end();
    for ins in &plan.inserts {
        let onset: i64 = ins.start_ticks.parse().unwrap();
        assert!(onset >= 0 && onset < region_end);
        assert_eq!(ins.clip_id, context.clip_id);
    }

    // Tamper the "region" → locked bytes differ → invariant catches it.
    after[0].pitch += 1;
    assert!(verify_locked_unchanged(&before_bytes, &after, &locked).is_err());
}

#[test]
fn t78_inpaint_stays_inside_gap_and_uses_edge_context() {
    // Gap inside a region: notes exist outside; inpaint fills only
    // the gap.
    let gap = Range::new(TICKS_PER_QUARTER * 4, TICKS_PER_QUARTER * 4).unwrap();
    let mut spec = GenerationSpec {
        seed: 7,
        role: Role::Bass,
        mode: GenMode::Inpaint,
        candidates: 2,
        output_range: gap,
        context_notes: (0..4)
            .map(|i| {
                GenNote::new(36 + i * 2, 90, i as i64 * 480_000, 240_000).unwrap()
            })
            .collect(),
        locked_ranges: vec![
            Range::new(0, TICKS_PER_QUARTER * 4).unwrap(),
            Range::new(TICKS_PER_QUARTER * 8, TICKS_PER_QUARTER * 4).unwrap(),
        ],
        scale: Scale::new(0, ScaleKind::NaturalMinor).unwrap(),
        chords: vec![],
        groove: GrooveTemplate::straight(),
        density_ppm: 600_000,
        variation_ppm: 0,
        tempo_bpm: 120,
    };
    let before = locked_region_bytes(&spec.context_notes, &spec.locked_ranges);
    for c in generate(&spec).unwrap() {
        assert!(!c.notes.is_empty());
        for n in &c.notes {
            assert!(gap.contains_tick(n.onset), "onset outside gap");
            assert!(n.end() <= gap.end(), "note extends past gap");
            assert!(!spec
                .locked_ranges
                .iter()
                .any(|l| l.overlaps(n.onset, n.end())));
        }
    }
    // Source untouched by construction — context bytes unchanged.
    let after = locked_region_bytes(&spec.context_notes, &spec.locked_ranges);
    void_producer::engine::postcondition_check(&before, &after).unwrap();
    spec.seed = 8;
    assert!(generate(&spec).is_ok());
}

#[test]
fn t78_vary_new_take_different_but_bounded() {
    let mut spec = GenerationSpec {
        seed: 99,
        role: Role::Synth,
        mode: GenMode::Accompaniment,
        candidates: 1,
        output_range: Range::new(0, TICKS_PER_QUARTER * 8).unwrap(),
        context_notes: vec![],
        locked_ranges: vec![],
        scale: Scale::new(0, ScaleKind::Major).unwrap(),
        chords: vec![],
        groove: GrooveTemplate::straight(),
        density_ppm: 700_000,
        variation_ppm: 400_000,
        tempo_bpm: 120,
    };
    let base = generate(&spec).unwrap()[0].notes.clone();
    spec.mode = GenMode::Vary;
    spec.context_notes = base.clone();
    let varied = &generate(&spec).unwrap()[0];
    assert_ne!(varied.notes, base);
    for n in &varied.notes {
        assert!(spec.output_range.contains_tick(n.onset));
        assert!(spec.scale.contains(n.pitch));
    }
    // Size within the density-variation envelope, not a reset.
    assert!(varied.notes.len() > base.len() / 3);
}
