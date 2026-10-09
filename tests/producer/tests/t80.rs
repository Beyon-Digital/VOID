//! T80 — producer workflow gate Linux-verifiable halves: stem-batch
//! specs fan out over void-export (shared frame plan, deterministic ids),
//! consolidation collects external media into the container without
//! evicting alternatives' assets, project alternatives seal/branch/
//! switch/protect, and import plans remap ids with a loss report.

use std::collections::BTreeSet;
use void_assets::AssetStore;
use void_export::TempoSegment;
use void_producer::*;
use void_producer::import::{ForeignClip, ForeignSend, ForeignBus, ForeignNote, ForeignTrack};

fn stem_template() -> StemBatchTemplate {
    StemBatchTemplate {
        project_id: uuid::Uuid::new_v4().to_string(),
        checkpoint_id: uuid::Uuid::new_v4().to_string(),
        source_revision: "3".into(),
        range_start_ticks: 0,
        range_end_ticks: TICKS_PER_QUARTER * 16,
        tempo_map: vec![
            TempoSegment { at_ticks: "0".into(), bpm: 120.0 },
            TempoSegment { at_ticks: (TICKS_PER_QUARTER * 8).to_string(), bpm: 90.0 },
        ],
        sample_rate: 48_000,
        bit_depth: void_export::BitDepth::Pcm24,
        channels: void_export::ChannelLayout::Stereo,
        tail: void_export::TailPolicy::Milliseconds { ms: 500 },
        asset_hashes: vec!["a".repeat(64)],
    }
}

#[test]
fn t80_stem_batch_over_void_export() {
    let t = stem_template();
    let sources = vec![
        StemSource { source_id: "trk-drums".into(), name: "Drums".into(), kind: StemKind::Track },
        StemSource { source_id: "bus-reverb".into(), name: "Reverb".into(), kind: StemKind::Bus },
        StemSource { source_id: "trk-lead".into(), name: "Lead Vox".into(), kind: StemKind::Track },
    ];
    let plan = plan_stem_batch(&t, &sources, "song").unwrap();
    assert_eq!(plan.members.len(), 3);
    for m in &plan.members {
        // Every member is a valid void-export spec — validated, not
        // assumed: recompute the frame plan and compare.
        m.spec.validate().unwrap();
        assert_eq!(m.spec.frame_plan().unwrap(), plan.frame_plan);
        assert_eq!(m.spec.format, void_export::ExportFormat::Wav);
        // Sanitized output names (spaces → _).
        assert!(m.spec.output_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'));
    }
    assert_eq!(plan.members[2].spec.output_name, "song_Lead_Vox");
    // Deterministic ids — replanning yields the same job ids.
    let again = plan_stem_batch(&t, &sources, "song").unwrap();
    for (a, b) in plan.members.iter().zip(again.members.iter()) {
        assert_eq!(a.spec.job_id, b.spec.job_id);
    }
    // Write + read back manifest.
    let dir = tempfile::tempdir().unwrap();
    let p = stems::write_plan(&plan, dir.path()).unwrap();
    let back: StemBatchPlan = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    assert_eq!(back.members.len(), 3);
}

#[test]
fn t80_consolidation_collects_and_protects_alternative_assets() {
    let dir = tempfile::tempdir().unwrap();
    let store = AssetStore::new(dir.path().join("assets"), 1 << 22).unwrap();
    // An alternative references sha-A — consolidation must not evict it
    // even though a new external file has the same content.
    let protected = store.import_bytes(b"shared drum loop", "wav").unwrap();

    let mut ledger = AlternativeLedger::new("proj");
    let alt = ledger
        .create("mix-v1", "ckpt-1", None, vec![protected.sha256.clone()])
        .unwrap();
    ledger.seal(&alt.alternative_id).unwrap();
    assert_eq!(ledger.protected_asset_set(), vec![protected.sha256.clone()]);

    let f1 = dir.path().join("loop.wav");
    let f2 = dir.path().join("pad.wav");
    std::fs::write(&f1, b"shared drum loop").unwrap();
    std::fs::write(&f2, b"new pad take").unwrap();
    let refs = vec![
        ExternalRef { path: f1.display().to_string(), referenced_by: "clip-drums".into() },
        ExternalRef { path: f2.display().to_string(), referenced_by: "clip-pad".into() },
        ExternalRef { path: dir.path().join("gone.wav").display().to_string(), referenced_by: "clip-x".into() },
    ];
    let plan = plan_consolidation("proj", &refs, &ledger.protected_asset_set()).unwrap();
    assert!(!plan.complete); // missing file reported, not hidden
    assert_eq!(plan.refs[0].status, RefStatus::AlreadyContained); // protected dedup
    assert_eq!(plan.refs[2].status, RefStatus::Missing);

    let done = execute_consolidation(&plan, &store).unwrap();
    // The new asset landed; the protected one is untouched and resolvable.
    let new_sha = done.refs[1].sha256.clone().unwrap();
    assert!(store.find(&new_sha).is_some());
    assert!(store.find(&protected.sha256).is_some());
    assert_eq!(store.verify(&new_sha).unwrap(), 12);
    // Idempotent re-run — same refs now resolve as present/duplicated.
    let plan2 = plan_consolidation("proj", &refs[..2], &ledger.protected_asset_set()).unwrap();
    assert!(plan2.complete);
}

#[test]
fn t80_alternatives_branch_seal_switch_rules() {
    let mut l = AlternativeLedger::new("proj");
    let main_alt = l
        .create("v1-main", "ckpt-A", None, vec!["sha1".into()])
        .unwrap();
    // Branch requires sealed parent.
    assert!(l.create("v2-alt", "ckpt-B", Some(&main_alt.alternative_id), vec![]).is_err());
    l.seal(&main_alt.alternative_id).unwrap();
    let branch = l
        .create("v2-alt", "ckpt-B", Some(&main_alt.alternative_id), vec!["sha2".into()])
        .unwrap();
    l.switch(Some(&branch.alternative_id)).unwrap();
    assert_eq!(l.active.as_deref(), Some(branch.alternative_id.as_str()));
    l.switch(None).unwrap(); // back to main line — allowed
    // Sealed can't delete; parented can't delete.
    assert!(l.delete(&main_alt.alternative_id).is_err());
    l.seal(&branch.alternative_id).unwrap();
    assert!(l.delete(&branch.alternative_id).is_err());
    assert_eq!(l.protected_asset_set(), vec!["sha1", "sha2"]);
}

#[test]
fn t80_import_plan_remaps_ids_reports_losses_no_dangling_sends() {
    let src = ForeignProject {
        name: "foreign-daw".into(),
        tempo_bpm: 140.0,
        ts_num: 4,
        ts_den: 4,
        tracks: vec![
            ForeignTrack {
                id: "T-audio".into(),
                name: "Audio 1".into(),
                kind: "audio".into(),
                clips: vec![ForeignClip {
                    id: "C1".into(),
                    start_ticks: 0,
                    length_ticks: TICKS_PER_QUARTER * 4,
                    source_path: "/ext/a.wav".into(),
                    notes: vec![],
                }],
            },
            ForeignTrack {
                id: "T-midi".into(),
                name: "Keys".into(),
                kind: "midi".into(),
                clips: vec![ForeignClip {
                    id: "C2".into(),
                    start_ticks: 0,
                    length_ticks: TICKS_PER_QUARTER * 2,
                    source_path: String::new(),
                    notes: vec![
                        ForeignNote { pitch: 60, velocity: 100, onset_ticks: 0, length_ticks: 240_000 },
                        ForeignNote { pitch: 64, velocity: 88, onset_ticks: 240_000, length_ticks: 240_000 },
                    ],
                }],
            },
            ForeignTrack {
                id: "T-video".into(),
                name: "Video".into(),
                kind: "video".into(),
                clips: vec![],
            },
        ],
        sends: vec![
            ForeignSend { from_track_id: "T-audio".into(), to_bus_id: "BUS-1".into(), gain_db: -6.0 },
            ForeignSend { from_track_id: "T-audio".into(), to_bus_id: "BUS-404".into(), gain_db: -3.0 },
        ],
        buses: vec![ForeignBus { id: "BUS-1".into(), name: "Verb".into() }],
    };
    let plan = plan_import(&src, &BTreeSet::new()).unwrap();
    // Video track → loss entry, audio+midi mapped.
    assert_eq!(plan.track_ids.len(), 2);
    assert_eq!(plan.losses.len(), 1);
    assert_eq!(plan.losses[0].feature, "track-kind");
    // Sends: one resolves, one dangles (reported, not linked).
    assert_eq!(plan.resolved_sends.len(), 1);
    assert_eq!(plan.unresolved.len(), 1);
    assert_eq!(plan.unresolved[0].to_bus_id, "BUS-404");
    // All new ids unique; ops ordered bus → tracks → clips → sends.
    let ids: BTreeSet<&String> = plan
        .track_ids.values()
        .chain(plan.clip_ids.values())
        .chain(plan.bus_ids.values())
        .collect();
    // 2 tracks + 2 clips + 1 bus = 5 unique ids.
    assert_eq!(ids.len(), 5);
    assert!(matches!(plan.ops[0], PlannedOp::CreateBus { .. }));
    assert!(plan.ops.iter().any(|o| matches!(o, PlannedOp::CreateSend { .. })));
    // 2 tracks + 2 clips + 1 bus = 5 unique ids.
    // Notes survive into the CreateClip op as real payloads.
    let clip_op = plan.ops.iter().find_map(|o| match o {
        PlannedOp::CreateClip { notes, .. } if !notes.is_empty() => Some(notes),
        _ => None,
    }).unwrap();
    assert_eq!(clip_op.len(), 2);
    assert_eq!(clip_op[0].pitch, 60);
}
