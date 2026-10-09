//! W20 exchange test suite — T75 (isolation policy), T76 (registry +
//! missing-plugin preservation), T77 (deterministic interchange with
//! loss reports). Each test names the matrix row it evidences.

use void_exchange::dawproject::{export_dawproject, import_dawproject, FilesMap};
use void_exchange::document::*;
use void_exchange::isolation::{
    requested_policy, IsolationPolicy, IsolationUnavailable,
};
use void_exchange::loss::{ExchangeDirection, LossKind, LossReport};
use void_exchange::midi::{export_smf, import_smf};
use void_exchange::preserve::{MissingPluginStore, PreservedPluginState, Resolution};
use void_exchange::registry::{
    CompatibilityStatus, PluginDescriptor, PluginFormat, PluginRegistry, RegistryEntry,
};
use void_exchange::stems::{load_bundle, stage_bundle, StemsManifest, StemRef};

const SR: u32 = 48_000;

fn desc(format: PluginFormat, uid: &str, name: &str) -> PluginDescriptor {
    PluginDescriptor {
        format,
        plugin_uid: uid.into(),
        name: name.into(),
        vendor: Some("VoidCo".into()),
        version: Some("1.0".into()),
        device_role: Some(void_exchange::DeviceRole::AudioFx),
        arch: vec!["x86_64".into()],
        state_format_version: Some("1".into()),
    }
}

fn sha(b: &[u8]) -> String {
    use sha2::Digest;
    void_exchange::preserve::encode_hex(&sha2::Sha256::digest(b))
}

/// Representative fixture: 2 tracks (midi + instrument w/ plugin),
/// tempo + timesig maps, markers, notes incl. channel + release.
fn fixture_doc() -> (ExchangeDocument, FilesMap) {
    let mut doc = ExchangeDocument::new("w20-fixture", SR);
    doc.tempo_map = vec![
        TempoPoint { at_ticks: 0, bpm: 120.0 },
        TempoPoint { at_ticks: 1_920_000, bpm: 90.0 },
    ];
    doc.time_signatures = vec![
        TimeSignaturePoint { at_ticks: 0, numerator: 4, denominator: 4 },
        TimeSignaturePoint { at_ticks: 3_840_000, numerator: 3, denominator: 8 },
    ];
    doc.markers = vec![Marker {
        at_ticks: 480_000,
        name: "verse".into(),
        color: None,
    }];

    let state = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x42];
    let state_path = "plugins/verb/state.bin".to_string();
    let mut files = FilesMap::new();
    files.insert(state_path.clone(), state.clone());

    let midi_track = Track {
        id: "tr-midi".into(),
        name: "Keys".into(),
        color: Some("#336699".into()),
        kind: TrackKind::MIDI,
        gain_linear: 1.0,
        pan: 0.0,
        muted: false,
        soloed: false,
        clips: vec![Clip {
            id: "clip-a".into(),
            name: "riff".into(),
            start_ticks: 960_000,
            length_ticks: 1_920_000,
            offset_ticks: 0,
            enabled: true,
            fade_in_ticks: Some(240_000),
            fade_out_ticks: None,
            content: ClipContent::Notes {
                notes: vec![
                    Note {
                        id: "n0".into(),
                        pitch: 60,
                        velocity: 100,
                        release_velocity: Some(64),
                        channel: 0,
                        start_ticks: 0,
                        length_ticks: 480_000,
                    },
                    Note {
                        id: "n1".into(),
                        pitch: 64,
                        velocity: 90,
                        release_velocity: None,
                        channel: 1,
                        start_ticks: 480_000,
                        length_ticks: 960_000,
                    },
                    Note {
                        id: "n2".into(),
                        pitch: 67,
                        velocity: 110,
                        release_velocity: None,
                        channel: 0,
                        start_ticks: 480_000,
                        length_ticks: 480_000,
                    },
                ],
            },
        }],
        plugins: Vec::new(),
    };

    let inst_track = Track {
        id: "tr-inst".into(),
        name: "Bass".into(),
        color: None,
        kind: TrackKind::INSTRUMENT,
        gain_linear: 0.8,
        pan: -0.25,
        muted: true,
        soloed: false,
        clips: vec![Clip {
            id: "clip-b".into(),
            name: String::new(),
            start_ticks: 0,
            length_ticks: 960_000,
            offset_ticks: 0,
            enabled: true,
            fade_in_ticks: None,
            fade_out_ticks: None,
            content: ClipContent::Notes {
                notes: vec![Note {
                    id: "nb".into(),
                    pitch: 36,
                    velocity: 120,
                    release_velocity: None,
                    channel: 0,
                    start_ticks: 0,
                    length_ticks: 480_000,
                }],
            },
        }],
        plugins: vec![PluginSlot {
            instance_id: "verb-1".into(),
            slot: 0,
            enabled: true,
            present: true,
            descriptor: desc(PluginFormat::Vst3, "com.voidco.verb", "VoidVerb"),
            parameters: vec![
                PluginParam {
                    param_id: "0".into(),
                    name: Some("mix".into()),
                    value: PluginParamValue::Real(0.5),
                },
                PluginParam {
                    param_id: "1".into(),
                    name: Some("freeze".into()),
                    value: PluginParamValue::Bool(false),
                },
            ],
            state: Some(PluginStateRef {
                rel_path: state_path.clone(),
                sha256: sha(&state),
                bytes: state.len().to_string(),
                format_version: Some("1".into()),
            }),
        }],
    };
    doc.tracks = vec![midi_track, inst_track];
    (doc, files)
}

/// Semantic equality for round-trips: compares every field the schema
/// claims to carry, ignoring purely cosmetic order where format demands
/// sortedness anyway.
fn semantic_equal(a: &ExchangeDocument, b: &ExchangeDocument) -> Vec<String> {
    let mut d = Vec::new();
    if a.tempo_map != b.tempo_map {
        d.push(format!("tempoMap {:?} != {:?}", a.tempo_map, b.tempo_map));
    }
    if a.time_signatures != b.time_signatures {
        d.push(format!("timeSignatures {:?} != {:?}", a.time_signatures, b.time_signatures));
    }
    if a.markers != b.markers {
        d.push(format!("markers {:?} != {:?}", a.markers, b.markers));
    }
    if a.tracks.len() != b.tracks.len() {
        d.push(format!("track count {} != {}", a.tracks.len(), b.tracks.len()));
        return d;
    }
    for (ta, tb) in a.tracks.iter().zip(b.tracks.iter()) {
        if ta.name != tb.name {
            d.push(format!("track name {:?} != {:?}", ta.name, tb.name));
        }
        if ta.kind != tb.kind {
            d.push(format!("track {:?} kind {:?} != {:?}", ta.id, ta.kind, tb.kind));
        }
        if (ta.gain_linear - tb.gain_linear).abs() > 1e-9 || (ta.pan - tb.pan).abs() > 1e-9 {
            d.push(format!("track {:?} mixer drift", ta.id));
        }
        if ta.muted != tb.muted || ta.soloed != tb.soloed {
            d.push(format!("track {:?} mute/solo drift", ta.id));
        }
        if ta.clips.len() != tb.clips.len() {
            d.push(format!("track {:?} clip count {} != {}", ta.id, ta.clips.len(), tb.clips.len()));
            continue;
        }
        for (ca, cb) in ta.clips.iter().zip(tb.clips.iter()) {
            if ca.start_ticks != cb.start_ticks || ca.length_ticks != cb.length_ticks {
                d.push(format!("clip {:?} timing drift", ca.id));
            }
            if ca.enabled != cb.enabled {
                d.push(format!("clip {:?} enable drift", ca.id));
            }
            if ca.fade_in_ticks != cb.fade_in_ticks || ca.fade_out_ticks != cb.fade_out_ticks {
                d.push(format!("clip {:?} fades drift", ca.id));
            }
            match (&ca.content, &cb.content) {
                (ClipContent::Notes { notes: na }, ClipContent::Notes { notes: nb }) => {
                    let mut sa: Vec<_> = na.iter().collect();
                    let mut sb: Vec<_> = nb.iter().collect();
                    sa.sort_by(|x, y| (x.start_ticks, x.pitch).cmp(&(y.start_ticks, y.pitch)));
                    sb.sort_by(|x, y| (x.start_ticks, x.pitch).cmp(&(y.start_ticks, y.pitch)));
                    if sa.len() != sb.len() {
                        d.push(format!("clip {:?} note count {} != {}", ca.id, sa.len(), sb.len()));
                        continue;
                    }
                    for (x, y) in sa.iter().zip(sb.iter()) {
                        if x.pitch != y.pitch
                            || x.velocity != y.velocity
                            || x.start_ticks != y.start_ticks
                            || x.length_ticks != y.length_ticks
                            || x.channel != y.channel
                            || x.release_velocity != y.release_velocity
                        {
                            d.push(format!("clip {:?} note drift {x:?} vs {y:?}", ca.id));
                        }
                    }
                }
                (ClipContent::Audio { asset_id: _ }, ClipContent::Audio { asset_id: _ }) => {}
                _ => d.push(format!("clip {:?} content kind drift", ca.id)),
            }
        }
        if ta.plugins.len() != tb.plugins.len() {
            d.push(format!("track {:?} plugin count drift", ta.id));
            continue;
        }
        for (pa, pb) in ta.plugins.iter().zip(tb.plugins.iter()) {
            if pa.descriptor.format != pb.descriptor.format
                || pa.descriptor.plugin_uid != pb.descriptor.plugin_uid
                || pa.descriptor.name != pb.descriptor.name
                || pa.present != pb.present
                || pa.enabled != pb.enabled
            {
                d.push(format!("plugin {:?} descriptor drift", pa.instance_id));
            }
            if pa.parameters != pb.parameters {
                d.push(format!("plugin {:?} params drift", pa.instance_id));
            }
            match (&pa.state, &pb.state) {
                (Some(sa), Some(sb)) => {
                    if sa.sha256 != sb.sha256 || sa.rel_path != sb.rel_path {
                        d.push(format!("plugin {:?} state ref drift", pa.instance_id));
                    }
                }
                (None, None) => {}
                _ => d.push(format!("plugin {:?} state presence drift", pa.instance_id)),
            }
        }
    }
    d
}

#[test]
fn t77_dawproject_roundtrip_semantic_equal_zero_loss() {
    let (doc, files) = fixture_doc();
    let (zip, export_loss) = export_dawproject(&doc, &files).expect("export");
    // Subset round-trip must be lossless.
    assert!(
        export_loss.is_empty(),
        "export loss not empty: {:?}",
        export_loss.entries
    );
    let (back, back_files, import_loss) = import_dawproject(&zip, SR).expect("import");
    assert!(
        import_loss.is_empty(),
        "import loss not empty: {:?}",
        import_loss.entries
    );
    let drift = semantic_equal(&doc, &back);
    assert!(drift.is_empty(), "semantic drift: {drift:?}");
    // plugin state blob survives the container
    assert_eq!(
        back_files.get("plugins/verb/state.bin").map(|v| v.as_slice()),
        Some(&[0xDE, 0xAD, 0xBE, 0xEF, 0x42][..])
    );
}

#[test]
fn t77_dawproject_loss_report_entries_for_unsupported() {
    let (mut doc, files) = fixture_doc();
    // Scenes can't be injected via document API; use clip features the
    // subset intentionally drops: AAX device + loop range.
    doc.tracks[1].plugins.push(PluginSlot {
        instance_id: "aax-1".into(),
        slot: 1,
        enabled: true,
        present: true,
        descriptor: desc(PluginFormat::Aax, "com.avid.x", "AaxThing"),
        parameters: Vec::new(),
        state: None,
    });
    doc.loop_range = Some(LoopRange {
        start_ticks: 0,
        end_ticks: 960_000,
        enabled: true,
    });
    let (_zip, loss) = export_dawproject(&doc, &files).expect("export");
    let kinds: Vec<(&str, &str)> = loss
        .entries
        .iter()
        .map(|e| (e.element.as_str(), e.aspect.as_str()))
        .collect();
    assert!(kinds.iter().any(|(e, a)| e.contains("aax-1") && *a == "deviceElement"));
    assert!(kinds.iter().any(|(_, a)| *a == "loopRange"));
    for e in &loss.entries {
        assert_eq!(e.kind, LossKind::Dropped);
        assert!(!e.reason.is_empty());
    }
}

#[test]
fn t77_loss_report_determinism_byte_identical() {
    let (mut doc, files) = fixture_doc();
    doc.loop_range = Some(LoopRange { start_ticks: 0, end_ticks: 1, enabled: true });
    doc.tracks[1].plugins.push(PluginSlot {
        instance_id: "aax-2".into(),
        slot: 2,
        enabled: true,
        present: true,
        descriptor: desc(PluginFormat::Aax, "com.avid.y", "Y"),
        parameters: Vec::new(),
        state: None,
    });
    let (_, l1) = export_dawproject(&doc, &files).unwrap();
    let (_, l2) = export_dawproject(&doc, &files).unwrap();
    let j1 = l1.canonical_json().unwrap();
    let j2 = l2.canonical_json().unwrap();
    assert_eq!(j1, j2, "canonical reports differ");
    // parse→re-emit stable too
    let parsed = LossReport::parse(&j1).unwrap();
    assert_eq!(parsed.canonical_json().unwrap(), j1);
    // sorted contract holds after parsing a shuffled report
    let mut shuffled = parsed.clone();
    shuffled.entries.reverse();
    let re = LossReport::parse(&serde_json::to_vec(&shuffled).unwrap()).unwrap();
    assert_eq!(re.canonical_json().unwrap(), j1);
}

#[test]
fn t77_midi_roundtrip_preserves_tempo_curve() {
    let (doc, _files) = fixture_doc();
    // MIDI is a subset: plugins/mixer/audio clip facets legitimately lose.
    let (smf, loss) = export_smf(&doc).expect("export smf");
    assert_eq!(&smf[..4], b"MThd");
    let (back, il) = import_smf(&smf, "w20-fixture", SR).expect("import smf");
    // tempo POINT positions exact; bpm values are μs-quantized (SMF
    // stores integer μs/quarter) — the quantization must appear in the
    // export loss report, not be silently dropped.
    assert_eq!(back.tempo_map.len(), doc.tempo_map.len());
    for (a, b) in doc.tempo_map.iter().zip(back.tempo_map.iter()) {
        assert_eq!(a.at_ticks, b.at_ticks, "tempo point position drift");
        assert!(
            (a.bpm - b.bpm).abs() / a.bpm < 1e-5,
            "bpm drift beyond μs quantization: {} vs {}",
            a.bpm,
            b.bpm
        );
    }
    assert!(
        loss.entries.iter().any(|e| e.element.contains("tempo")),
        "expected tempo approximation entry, got {:?}",
        loss.entries
    );
    assert_eq!(back.time_signatures, doc.time_signatures, "sig map drift");
    let _ = il;
    assert_eq!(back.markers.len(), doc.markers.len(), "markers lost");
    // notes: per-track union of all notes preserved
    fn tuples(doc: &ExchangeDocument) -> Vec<(u8, u8, i64, i64, u8)> {
        let mut out = Vec::new();
        for t in &doc.tracks {
            for c in &t.clips {
                if let ClipContent::Notes { notes } = &c.content {
                    for n in notes {
                        out.push((
                            n.pitch,
                            n.velocity,
                            c.start_ticks + n.start_ticks,
                            n.length_ticks,
                            n.channel,
                        ));
                    }
                }
            }
        }
        out
    }
    let src = tuples(&doc);
    let dst = tuples(&back);
    let mut s = src.clone();
    let mut d = dst.clone();
    s.sort();
    d.sort();
    assert_eq!(s, d, "note tuples (pitch,vel,abs_start,len,ch) drift");
    // loss report lists what MIDI cannot carry
    assert!(loss
        .entries
        .iter()
        .any(|e| e.aspect == "plugins" || e.aspect == "mixer"));
}

#[test]
fn t77_stems_manifest_roundtrip_verified() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = StemsManifest::new("session");
    m.tempo_map = vec![TempoPoint { at_ticks: 0, bpm: 120.0 }];
    let payload = b"fake wav bytes".to_vec();
    m.stems.push(StemRef {
        track_id: "t1".into(),
        track_name: "Drums".into(),
        rel_path: "stems/drums.wav".into(),
        media_type: "audio/wav".into(),
        sha256: sha(&payload),
        bytes: payload.len() as u64,
        length_ticks: 960_000,
        length_seconds: 2.0,
        sample_rate: SR,
        channels: 2,
    });
    stage_bundle(
        dir.path(),
        &m,
        &[("stems/drums.wav".to_string(), payload.clone())],
    )
    .unwrap();
    let loaded = load_bundle(dir.path()).unwrap();
    assert_eq!(loaded.stems.len(), 1);
    assert_eq!(loaded.stems[0].sha256, m.stems[0].sha256);
    // tamper → rejected
    std::fs::write(dir.path().join("stems/drums.wav"), b"tampered").unwrap();
    assert!(load_bundle(dir.path()).is_err());
    // canonical json stable
    assert_eq!(m.to_json().unwrap(), m.to_json().unwrap());
}

#[test]
fn t76_registry_compatibility_statuses() {
    let mut reg = PluginRegistry::new();
    let vst3 = desc(PluginFormat::Vst3, "com.voidco.verb", "VoidVerb");
    reg.upsert(RegistryEntry {
        descriptor: vst3.clone(),
        hosted_on: vec!["linux-x86_64".into(), "macos-arm64".into()],
        quarantined: false,
        quarantine_reason: None,
        notes: Vec::new(),
    });
    assert_eq!(
        reg.compatibility(&vst3, "linux-x86_64").status,
        CompatibilityStatus::Compatible
    );
    assert_eq!(
        reg.compatibility(&vst3, "windows-x86_64").status,
        CompatibilityStatus::NotObservedOnPlatform
    );
    // arm-only arch mismatch
    let arm_only = PluginDescriptor {
        arch: vec!["arm64".into()],
        ..vst3.clone()
    };
    assert_eq!(
        reg.compatibility(&arm_only, "linux-x86_64").status,
        CompatibilityStatus::ArchMismatch
    );
    // AAX always unhostable
    let aax = desc(PluginFormat::Aax, "com.avid.x", "X");
    assert_eq!(
        reg.compatibility(&aax, "macos-arm64").status,
        CompatibilityStatus::FormatNotHostable
    );
    // AU off-macos unhostable
    let au = desc(PluginFormat::Au, "com.apple.x", "X");
    assert_eq!(
        reg.compatibility(&au, "linux-x86_64").status,
        CompatibilityStatus::FormatNotHostable
    );
    // AU on macOS, never registered, arch undeclared → honest Unknown
    // (declared-arch descriptors get the stronger ArchMismatch verdict
    // before the registry is even consulted).
    let au_any = PluginDescriptor { arch: Vec::new(), ..au.clone() };
    assert_eq!(
        reg.compatibility(&au_any, "macos-arm64").status,
        CompatibilityStatus::Unknown
    );
    // quarantine
    reg.mark_quarantined(&vst3, "crashed twice at scan");
    assert_eq!(
        reg.compatibility(&vst3, "linux-x86_64").status,
        CompatibilityStatus::Quarantined
    );
    // never-registered descriptor → Unknown
    let unseen = desc(PluginFormat::Clap, "com.acme.synth", "Synth");
    assert_eq!(
        reg.compatibility(&unseen, "linux-x86_64").status,
        CompatibilityStatus::Unknown
    );
    // registry json round-trip is stable
    let bytes = reg.to_json().unwrap();
    let parsed = PluginRegistry::parse(&bytes).unwrap();
    assert_eq!(parsed.to_json().unwrap(), bytes);
}

#[test]
fn t76_missing_plugin_state_survives_reopen_via_fake_descriptor() {
    let blob = vec![7u8; 64];
    let stored = PreservedPluginState {
        instance_id: "synth-9".into(),
        track_id: "tr-1".into(),
        slot: 0,
        descriptor: desc(PluginFormat::Clap, "com.acme.synth", "AcmeSynth"),
        state_hex: Some(void_exchange::preserve::encode_hex(&blob)),
        state_sha256: Some(sha(&blob)),
        state_format_version: Some("1".into()),
        resolved: false,
    };
    let mut store = MissingPluginStore::new();
    store.preserve(stored);
    // serialize → "reopen"
    let bytes = store.to_json().unwrap();
    let reopened = MissingPluginStore::parse(&bytes).unwrap();
    assert_eq!(reopened.missing().count(), 1);

    // plugin absent → StillMissing::NotInstalled
    let none: Vec<PluginDescriptor> = Vec::new();
    assert!(matches!(
        reopened.resolve("synth-9", &none),
        Some(Resolution::StillMissing(_))
    ));

    // plugin reappears (fake descriptor — no code runs, this is the model)
    let live = desc(PluginFormat::Clap, "com.acme.synth", "AcmeSynth");
    match reopened.resolve("synth-9", &[live]) {
        Some(Resolution::Rehydrate { descriptor, state }) => {
            assert_eq!(descriptor.plugin_uid, "com.acme.synth");
            assert_eq!(state, Some(blob));
        }
        other => panic!("expected rehydrate, got {other:?}"),
    }

    // state format mismatch refuses honestly
    let mut newer = desc(PluginFormat::Clap, "com.acme.synth", "AcmeSynth");
    newer.state_format_version = Some("2".into());
    assert!(matches!(
        reopened.resolve("synth-9", &[newer]),
        Some(Resolution::StillMissing(
            void_exchange::StillMissing::StateVersionMismatch { .. }
        ))
    ));

    // mark resolved → no longer missing
    let mut m = MissingPluginStore::parse(&store.to_json().unwrap()).unwrap();
    m.mark_resolved("synth-9");
    assert_eq!(m.missing().count(), 0);
}

#[test]
fn t75_isolation_policy_validates_and_rejects() {
    // good isolated policy passes
    assert!(IsolationPolicy::isolated_default().validate().is_ok());
    // honest in-process policy: zeroed budgets required
    assert!(IsolationPolicy::in_process().validate().is_ok());
    // in-process claiming bridge budgets = a lie → rejected
    let mut fake = IsolationPolicy::in_process();
    fake.bridge.audio_buffer_frames = 512;
    assert!(fake.validate().is_err());
    // out-of-bounds isolated values rejected, all errors listed
    let mut bad = IsolationPolicy::isolated_default();
    bad.bridge.event_queue_capacity = 0;
    bad.bridge.audio_buffer_frames = 999_999;
    bad.deadline.budget_ns = 0;
    bad.restart.max_restarts = 0;
    let errs = bad.validate().unwrap_err();
    assert!(errs.len() >= 4, "expected >=4 violations, got {errs:?}");
    // bounded restart budget required — crash-loop must not restart forever
    let mut unbounded = IsolationPolicy::isolated_default();
    unbounded.restart.max_restarts = 10_000;
    assert!(unbounded.validate().is_err());
}

#[test]
fn t75_isolation_unavailable_is_typed_and_honest() {
    let aax = desc(PluginFormat::Aax, "com.avid.x", "X");
    assert!(matches!(
        requested_policy(&aax, "linux-x86_64"),
        Err(IsolationUnavailable::FormatNotHostable { .. })
    ));
    let au = desc(PluginFormat::Au, "com.apple.x", "X");
    assert!(matches!(
        requested_policy(&au, "linux-x86_64"),
        Err(IsolationUnavailable::PlatformNotSupported { .. })
    ));
    let vst3 = desc(PluginFormat::Vst3, "com.voidco.verb", "VoidVerb");
    let policy = requested_policy(&vst3, "linux-x86_64").expect("vst3 policy");
    assert!(policy.validate().is_ok());
    // the returned policy is a request the native host enforces — bounded
    assert!(policy.restart.max_restarts <= 64);
    assert!(policy.deadline.budget_ns <= 1_000_000_000);
    // builtins stay in-process, honestly
    let builtin = desc(PluginFormat::Builtin, "void.eq", "EQ");
    assert_eq!(
        requested_policy(&builtin, "linux-x86_64").unwrap().mode,
        void_exchange::isolation::IsolationMode::InProcess
    );
}

#[test]
fn t77_wire_plan_emits_ops_and_honest_loss() {
    let (doc, _files) = fixture_doc();
    let plan = void_exchange::plan_import(&doc);
    // ops exist for transport + both tracks + clips + notes + plugin
    let names: Vec<&str> = plan.ops.iter().map(|o| o.op.as_str()).collect();
    assert!(names.contains(&"SetTempoOp"));
    assert!(names.contains(&"CreateTrackOp"));
    assert!(names.contains(&"CreateClipOp"));
    assert!(names.contains(&"InsertNoteOp"));
    assert!(names.contains(&"InsertPluginOp"));
    // wire gaps recorded: fades, note channel, note release, plugin state
    let aspects: Vec<&str> = plan.loss.entries.iter().map(|e| e.aspect.as_str()).collect();
    assert!(aspects.contains(&"fades"), "fade loss missing: {aspects:?}");
    assert!(aspects.contains(&"channel"), "channel loss missing");
    assert!(aspects.contains(&"releaseVelocity"));
    assert!(aspects.contains(&"state"));
    // marker has no wire op
    assert!(plan
        .loss
        .entries
        .iter()
        .any(|e| e.aspect == "markers" && e.kind == LossKind::Dropped));
    // string-int64: every *Ticks payload field is a JSON string
    for op in &plan.ops {
        let v = &op.payload;
        for k in ["startTicks", "lengthTicks", "atTicks"] {
            if let Some(x) = v.get(k) {
                assert!(x.is_string(), "op {} field {k} not a string: {x}", op.op);
            }
        }
    }
}

#[test]
fn t77_import_rejects_unsafe_and_malformed() {
    // unsafe zip member
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    use zip::write::SimpleFileOptions;
    use std::io::Write;
    zw.start_file("../evil.xml", SimpleFileOptions::default()).unwrap();
    zw.write_all(b"<x/>").unwrap();
    zw.start_file("project.xml", SimpleFileOptions::default()).unwrap();
    zw.write_all(b"<Project version=\"1.0\"><Application name=\"x\" version=\"1\"/></Project>").unwrap();
    let bytes = zw.finish().unwrap().into_inner();
    assert!(matches!(
        import_dawproject(&bytes, SR),
        Err(void_exchange::ExchangeError::UnsafePath(_))
    ));
    // not a zip
    assert!(import_dawproject(b"not a zip", SR).is_err());
    // not smf
    assert!(import_smf(b"XYZw1234567", "x", SR).is_err());
    // smf type 2 rejected explicitly
    let mut hdr = Vec::new();
    hdr.extend_from_slice(b"MThd");
    hdr.extend_from_slice(&6u32.to_be_bytes());
    hdr.extend_from_slice(&2u16.to_be_bytes()); // format 2
    hdr.extend_from_slice(&1u16.to_be_bytes());
    hdr.extend_from_slice(&480u16.to_be_bytes());
    hdr.extend_from_slice(b"MTrk");
    hdr.extend_from_slice(&4u32.to_be_bytes());
    hdr.extend_from_slice(&[0, 0xFF, 0x2F, 0x00]);
    assert!(matches!(
        import_smf(&hdr, "x", SR),
        Err(void_exchange::ExchangeError::Unsupported(_))
    ));
}

#[test]
fn t77_document_validation_collects_all_errors() {
    let mut doc = ExchangeDocument::new("bad", SR);
    doc.tempo_map = vec![
        TempoPoint { at_ticks: 5, bpm: 120.0 },
        TempoPoint { at_ticks: 5, bpm: 90.0 }, // duplicate position
    ];
    doc.tracks.push(Track {
        id: "t".into(),
        name: String::new(),
        color: None,
        kind: TrackKind::MIDI,
        gain_linear: f64::NAN, // non-finite
        pan: 0.0,
        muted: false,
        soloed: false,
        clips: vec![Clip {
            id: "c".into(),
            name: String::new(),
            start_ticks: -1, // negative
            length_ticks: 0, // zero
            offset_ticks: 0,
            enabled: true,
            fade_in_ticks: None,
            fade_out_ticks: None,
            content: ClipContent::Notes { notes: Vec::new() },
        }],
        plugins: Vec::new(),
    });
    let errs = doc.validate().unwrap_err();
    assert!(errs.len() >= 3, "expected multiple errors, got {errs:?}");
}
