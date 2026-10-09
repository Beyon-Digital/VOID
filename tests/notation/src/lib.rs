//! Lane V / W25 verification suite — MusicXML round-trips, stable ids
//! through edits, transactional ops + undo, anchors surviving tempo
//! changes, tab/lyric binding, loss-report determinism (T90/T91).

use std::path::PathBuf;

use void_exchange::{ExchangeDirection, LossKind};
use void_notation::*;

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}"))
}

fn el<'a>(s: &'a Score, id: &str) -> &'a Element {
    s.element(&ElementId(id.to_string()))
        .unwrap_or_else(|| panic!("missing element {id}"))
}

fn eid(s: &str) -> ElementId {
    ElementId(s.to_string())
}

// ---------------------------------------------------------------------------
// T91 — MusicXML import shape
// ---------------------------------------------------------------------------

#[test]
fn import_etude_shape() {
    let (score, loss) = import_musicxml(&fixture("etude-c.musicxml")).unwrap();
    assert!(loss.entries.is_empty(), "clean fixture imports lossless: {loss:?}");

    assert_eq!(score.title.as_deref(), Some("Etude in C"));
    assert_eq!(score.movement.as_deref(), Some("Allegro"));
    assert_eq!(score.composer.as_deref(), Some("J. Lane"));

    assert_eq!(score.parts.len(), 1);
    let p = &score.parts[0];
    assert_eq!(p.id, "P1");
    assert_eq!(p.name, "Piano");
    assert_eq!(p.abbreviation.as_deref(), Some("Pno."));
    assert_eq!(p.measures.len(), 2);

    let attrs = p.measures[0].attributes.as_ref().unwrap();
    assert_eq!(attrs.key.as_ref().unwrap().fifths, 0);
    assert_eq!(attrs.key.as_ref().unwrap().mode.as_deref(), Some("major"));
    assert_eq!(attrs.time.as_ref().unwrap().beats, 4);
    assert_eq!(attrs.time.as_ref().unwrap().beat_type, 4);
    assert_eq!(attrs.staves, Some(1));
    assert_eq!(attrs.clefs[0].sign, ClefSign::G);
    assert_eq!(attrs.clefs[0].line, Some(2));

    // divisions=24 → 1 unit = 960000/24 = 40000 ticks; quarter = 960000.
    let n1a = el(&score, "n1a");
    let pos = n1a.position.unwrap();
    assert_eq!(pos.offset_ticks, 0);
    assert_eq!(pos.duration_ticks, 960_000);
    assert_eq!(pos.voice, 1);
    match &n1a.kind {
        ElementKind::Note(n) => {
            assert_eq!(n.pitch.step, Step::C);
            assert_eq!(n.pitch.octave, 4);
            assert_eq!(n.note_type, Some(NoteType::Quarter));
        }
        k => panic!("n1a kind {k:?}"),
    }

    // n1b at eighth offset: 24 units → 960000.
    let n1b = el(&score, "n1b");
    assert_eq!(n1b.position.unwrap().offset_ticks, 960_000);
    assert_eq!(n1b.position.unwrap().duration_ticks, 480_000);

    // Staccato attached to n1c as an element.
    let stacc = score
        .parts[0]
        .measures[0]
        .elements
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Articulation(a) if a.host == eid("n1c")))
        .expect("staccato on n1c");
    assert!(matches!(
        stacc.kind,
        ElementKind::Articulation(ArticulationData {
            kind: ArticulationKind::Staccato,
            ..
        })
    ));

    // Ties.
    match &el(&score, "n1d").kind {
        ElementKind::Note(n) => assert!(n.tie.start && !n.tie.stop),
        _ => panic!(),
    }
    match &el(&score, "n2a").kind {
        ElementKind::Note(n) => {
            assert!(n.tie.stop && !n.tie.start);
        }
        _ => panic!(),
    }

    // Lyric bound to n2a.
    let lyr = score.parts[0].measures[1]
        .elements
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Lyric(l) if l.host == eid("n2a")))
        .expect("lyric on n2a");
    if let ElementKind::Lyric(l) = &lyr.kind {
        assert_eq!(l.number, "1");
        assert_eq!(l.syllabic, Syllabic::Single);
        assert_eq!(l.text, "la");
    }

    // Tempo map: <sound tempo="132"/> at offset 0 of measure 1.
    assert_eq!(score.tempo_map.len(), 1);
    assert_eq!(score.tempo_map[0].at_ticks, 0);
    assert!((score.tempo_map[0].bpm - 132.0).abs() < 1e-9);

    // Directions: metronome + words both landed as elements.
    let dirs: Vec<_> = p.measures[0]
        .elements
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Direction(_)))
        .collect();
    assert!(dirs.iter().any(|e| matches!(
        &e.kind,
        ElementKind::Direction(DirectionData {
            kind: DirectionKind::Metronome { per_minute, .. }
        }) if (*per_minute - 132.0).abs() < 1e-9
    )));
    assert!(dirs.iter().any(|e| matches!(
        &e.kind,
        ElementKind::Direction(DirectionData {
            kind: DirectionKind::Words { text }
        }) if text == "Allegro con brio"
    )));

    score.validate().unwrap();
}

#[test]
fn import_voices_tuplet_chord_beam_slur() {
    let (score, loss) = import_musicxml(&fixture("strings-voices.musicxml")).unwrap();
    // Only expected loss: `<sound dynamics>` playback hint.
    assert_eq!(
        loss.entries.len(),
        1,
        "unexpected losses: {:?}",
        loss.entries
    );
    assert!(loss.entries[0].aspect.contains("sound"));

    assert_eq!(score.parts.len(), 2);
    let v1 = &score.parts[0];
    assert_eq!(v1.group_name.as_deref(), Some("Strings"));

    // Two voices via backup: v1e at offset 0 voice 2.
    let v1e = el(&score, "v1e");
    assert_eq!(v1e.position.unwrap().offset_ticks, 0);
    assert_eq!(v1e.position.unwrap().voice, 2);
    assert_eq!(v1e.position.unwrap().duration_ticks, 960_000);

    // Slur v1a→v1c as an element.
    let slur = v1.measures[0]
        .elements
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Slur(_)))
        .expect("slur element");
    match &slur.kind {
        ElementKind::Slur(s) => {
            assert_eq!(s.start, eid("v1a"));
            assert_eq!(s.end, eid("v1c"));
            assert_eq!(s.number, 1);
        }
        _ => panic!(),
    }

    // Beam over v1a,v1b.
    let beam = v1.measures[0]
        .elements
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Beam(_)))
        .expect("beam element");
    match &beam.kind {
        ElementKind::Beam(b) => {
            assert_eq!(b.number, 1);
            assert_eq!(b.members.len(), 2);
            assert_eq!(b.members[0].element, eid("v1a"));
            assert_eq!(b.members[0].role, BeamRole::Begin);
            assert_eq!(b.members[1].role, BeamRole::End);
        }
        _ => panic!(),
    }

    // Cello: triplet = Tuplet element over v2a..v2c, 3-in-2, each
    // duration 16 units → 16*40000 = 640000 ticks each, total 1920000
    // (= half note at divisions→ticks, 3 eighths-of-normal = 2*960000).
    let v2 = &score.parts[1];
    let tup = v2.measures[0]
        .elements
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Tuplet(_)))
        .expect("tuplet element");
    match &tup.kind {
        ElementKind::Tuplet(t) => {
            assert_eq!(t.actual, 3);
            assert_eq!(t.normal, 2);
            assert_eq!(t.normal_type, Some(NoteType::Eighth));
            assert_eq!(
                t.members,
                vec![eid("v2a"), eid("v2b"), eid("v2c")]
            );
        }
        _ => panic!(),
    }
    assert_eq!(el(&score, "v2a").position.unwrap().duration_ticks, 640_000);
    assert_eq!(el(&score, "v2c").position.unwrap().offset_ticks, 1_280_000);

    // Chord: v2d is a Chord with two pitches (D4 + A4).
    match &el(&score, "v2d").kind {
        ElementKind::Chord(c) => {
            assert_eq!(c.pitches.len(), 2);
            assert_eq!(c.pitches[0].step, Step::D);
            assert_eq!(c.pitches[1].step, Step::A);
            assert_eq!(c.pitches[1].octave, 4);
        }
        k => panic!("v2d kind {k:?}"),
    }

    // Wedges landed as directions.
    let wedges: Vec<_> = v2.measures[0]
        .elements
        .iter()
        .filter(|e| matches!(&e.kind, ElementKind::Direction(d) if matches!(d.kind, DirectionKind::Wedge{..})))
        .collect();
    assert_eq!(wedges.len(), 2);
    score.validate().unwrap();
}

// ---------------------------------------------------------------------------
// T91 — round-trip semantic equality (the heart of the lane)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_semantic_eq() {
    for f in [
        "etude-c.musicxml",
        "strings-voices.musicxml",
        "guitar-lossy.musicxml",
        "odd-divisions.musicxml",
    ] {
        let (a, _l1) = import_musicxml(&fixture(f)).unwrap();
        let (xml, _l2) = export_musicxml(&a).unwrap();
        let (b, _l3) = import_musicxml(&xml).unwrap();
        assert!(a.semantic_eq(&b), "{f} failed round-trip semantic equality\n{xml}");
    }
}

#[test]
fn round_trip_id_stable_through_reimport() {
    let xml = fixture("etude-c.musicxml");
    let (a, _) = import_musicxml(&xml).unwrap();
    let ids: Vec<String> = a.parts[0]
        .measures
        .iter()
        .flat_map(|m| m.elements.iter().map(|e| e.id.0.clone()))
        .collect();
    let (xml2, _) = export_musicxml(&a).unwrap();
    let (b, _) = import_musicxml(&xml2).unwrap();
    let ids2: Vec<String> = b.parts[0]
        .measures
        .iter()
        .flat_map(|m| m.elements.iter().map(|e| e.id.0.clone()))
        .collect();
    assert_eq!(ids, ids2, "element ids must be identical across round-trip");
}

#[test]
fn export_emits_expected_markup() {
    let (a, _) = import_musicxml(&fixture("strings-voices.musicxml")).unwrap();
    let (xml, _) = export_musicxml(&a).unwrap();
    assert!(xml.contains("<score-partwise version=\"4.0\">"));
    assert!(xml.contains("id=\"v1a\""));
    assert!(xml.contains("<actual-notes>3</actual-notes>"));
    assert!(xml.contains("<chord/>"));
    assert!(xml.contains("<backup>"));
    assert!(xml.contains("<wedge type=\"crescendo\"/>"));
    assert!(xml.contains("<slur type=\"start\""));
    assert!(xml.contains("<group-name>Strings</group-name>"));
}

// ---------------------------------------------------------------------------
// T90 — edit ops, stable ids, transactions, undo
// ---------------------------------------------------------------------------

fn etude() -> Score {
    import_musicxml(&fixture("etude-c.musicxml")).unwrap().0
}

#[test]
fn op_insert_move_transpose_duration() {
    let mut s = etude();
    let before = s.clone();

    // Insert a new note at measure 2, offset 0, then move it.
    let new_note = Element {
        id: eid("ins1"),
        position: Some(Position {
            offset_ticks: 0,
            duration_ticks: 480_000,
            voice: 2,
            staff: 1,
        }),
        kind: ElementKind::Note(NoteData {
            pitch: Pitch {
                step: Step::G,
                alter: 0,
                octave: 3,
            },
            note_type: Some(NoteType::Eighth),
            dots: 0,
            tie: Tie::default(),
            accidental: None,
        }),
    };
    let t1 = s
        .apply(&ScoreOp::InsertElement {
            part_id: "P1".into(),
            measure_index: 1,
            element: new_note,
        })
        .unwrap();
    assert!(s.element(&eid("ins1")).is_some());

    // Move n1e (rest in m1) to measure 1 (index 1) at offset 2880000.
    let t2 = s
        .apply(&ScoreOp::MoveElement {
            element_id: eid("n1e"),
            measure_index: 1,
            offset_ticks: "2880000".into(),
            voice: 1,
            staff: 1,
        })
        .unwrap();
    let (pi, mi) = s.locate(&eid("n1e")).unwrap();
    assert_eq!((pi, mi), (0, 1));
    assert_eq!(el(&s, "n1e").position.unwrap().offset_ticks, 2_880_000);

    // Transpose n1a up 7 semitones (C4→G4 in C major).
    let t3 = s
        .apply(&ScoreOp::Transpose {
            scope: TransposeScope::Elements {
                ids: vec![eid("n1a")],
            },
            semitones: 7,
        })
        .unwrap();
    match &el(&s, "n1a").kind {
        ElementKind::Note(n) => {
            assert_eq!(n.pitch.step, Step::G);
            assert_eq!(n.pitch.octave, 4);
        }
        _ => panic!(),
    }

    // Duration change.
    let t4 = s
        .apply(&ScoreOp::SetDuration {
            element_id: eid("n1b"),
            duration_ticks: "240000".into(),
        })
        .unwrap();
    assert_eq!(el(&s, "n1b").position.unwrap().duration_ticks, 240_000);

    // ids unchanged through every edit.
    for id in ["n1a", "n1b", "n1e", "ins1"] {
        assert!(s.element(&eid(id)).is_some(), "id {id} lost through edit");
    }

    // Undo all four — exact restore.
    s.undo(&t4).unwrap();
    s.undo(&t3).unwrap();
    s.undo(&t2).unwrap();
    s.undo(&t1).unwrap();
    assert!(s.semantic_eq(&before), "undo must restore exact prior state");
}

#[test]
fn transaction_rolls_back_on_failure() {
    let mut s = etude();
    let before = s.clone();
    let res = s.apply_transaction(&[
        ScoreOp::SetDuration {
            element_id: eid("n1a"),
            duration_ticks: "480000".into(),
        },
        ScoreOp::MoveElement {
            element_id: eid("does-not-exist"),
            measure_index: 0,
            offset_ticks: "0".into(),
            voice: 1,
            staff: 1,
        },
    ]);
    assert!(res.is_err());
    assert!(
        s.semantic_eq(&before),
        "failed transaction must leave the score untouched"
    );
}

#[test]
fn cascade_delete_and_undo() {
    let (mut s, _) = import_musicxml(&fixture("strings-voices.musicxml")).unwrap();
    let before = s.clone();

    // Delete v1a — cascade must remove the slur (v1a→v1c) and shrink
    // the beam (v1a,v1b → 1 member < 2 → beam removed too).
    let tok = s
        .apply(&ScoreOp::DeleteElement {
            element_id: eid("v1a"),
        })
        .unwrap();
    assert!(s.element(&eid("v1a")).is_none());
    assert!(
        s.parts[0]
            .measures[0]
            .elements
            .iter()
            .all(|e| !matches!(e.kind, ElementKind::Slur(_))),
        "slur referencing deleted host must be removed"
    );
    assert!(
        s.parts[0]
            .measures[0]
            .elements
            .iter()
            .all(|e| !matches!(e.kind, ElementKind::Beam(_))),
        "beam reduced below 2 members must be removed"
    );
    assert!(s.element(&eid("v1b")).is_some());

    s.undo(&tok).unwrap();
    assert!(s.semantic_eq(&before), "cascade undo must restore slur+beam");
}

#[test]
fn lyric_and_tab_binding_upsert_stable_ids() {
    let (mut s, _) = import_musicxml(&fixture("guitar-lossy.musicxml")).unwrap();

    // g1 already carries tab 6/0 from the fixture — record its id.
    let tab_before = s.parts[0].measures[0]
        .elements
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Tab(t) if t.host == eid("g1")))
        .expect("tab on g1");
    let tab_id = tab_before.id.clone();

    // Re-bind g1 to fret 2 — upsert keeps the element id.
    s.apply(&ScoreOp::BindTab {
        host_id: eid("g1"),
        tab_id: None,
        member: 0,
        string: 6,
        fret: 2,
    })
    .unwrap();
    let tab_after = s.parts[0].measures[0]
        .elements
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Tab(t) if t.host == eid("g1")))
        .unwrap();
    assert_eq!(tab_after.id, tab_id, "tab upsert must keep stable id");
    if let ElementKind::Tab(t) = &tab_after.kind {
        assert_eq!(t.string, 6);
        assert_eq!(t.fret, 2);
        assert_eq!(t.member, 0);
    }

    // Assign a lyric to g2, then rewrite it — id stable.
    let t1 = s
        .apply(&ScoreOp::AssignLyric {
            host_id: eid("g2"),
            lyric_id: Some(eid("lyr-g2")),
            number: "1".into(),
            syllabic: Syllabic::Begin,
            text: "hel".into(),
        })
        .unwrap();
    let lyr = s.parts[0].measures[0]
        .elements
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Lyric(l) if l.host == eid("g2")))
        .unwrap();
    assert_eq!(lyr.id, eid("lyr-g2"));

    s.apply(&ScoreOp::AssignLyric {
        host_id: eid("g2"),
        lyric_id: None,
        number: "1".into(),
        syllabic: Syllabic::End,
        text: "lo".into(),
    })
    .unwrap();
    let lyr2 = s.parts[0].measures[0]
        .elements
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Lyric(l) if l.host == eid("g2")))
        .unwrap();
    assert_eq!(lyr2.id, eid("lyr-g2"), "lyric upsert keeps id");
    if let ElementKind::Lyric(l) = &lyr2.kind {
        assert_eq!(l.text, "lo");
        assert_eq!(l.syllabic, Syllabic::End);
    }
    s.undo(&t1).unwrap(); // lyric removed entirely (was an insert)
    assert!(s.parts[0].measures[0]
        .elements
        .iter()
        .all(|e| !matches!(&e.kind, ElementKind::Lyric(_))));
}

#[test]
fn extract_part_preserves_ids() {
    let (s, _) = import_musicxml(&fixture("strings-voices.musicxml")).unwrap();
    let v1 = s.extract_part("V1").unwrap();
    assert_eq!(v1.parts.len(), 1);
    assert_eq!(v1.parts[0].id, "V1");
    for id in ["v1a", "v1b", "v1c", "v1d", "v1e", "v1f", "v1g"] {
        assert!(v1.element(&eid(id)).is_some());
    }
    // Exported extracted part still round-trips.
    let (xml, _) = export_musicxml(&v1).unwrap();
    let (back, _) = import_musicxml(&xml).unwrap();
    assert!(v1.semantic_eq(&back));
}

// ---------------------------------------------------------------------------
// T91 — anchors + declared timecode modes
// ---------------------------------------------------------------------------

#[test]
fn anchors_survive_tempo_map_change() {
    let mut s = Score::new("Film cue");
    s.parts.push(Part {
        id: "P1".into(),
        name: "Cues".into(),
        abbreviation: None,
        group_name: None,
        measures: vec![Measure {
            number: "1".into(),
            attributes: Some(MeasureAttributes {
                time: Some(MeasureTimeSignature {
                    beats: 4,
                    beat_type: 4,
                }),
                ..Default::default()
            }),
            elements: Vec::new(),
        }],
    });
    s.tempo_map = vec![TempoPoint {
        at_ticks: 0,
        bpm: 120.0,
    }];
    s.timecode = Some(TimecodeMode::FPS_2997_DF);
    s.anchors = vec![
        Anchor {
            id: "hit-door".into(),
            label: "door slam".into(),
            kind: Some(AnchorKind::Hit),
            at_seconds: Rat::i64(2),
            timecode_hint: None,
        },
        Anchor {
            id: "hit-window".into(),
            label: "window".into(),
            kind: Some(AnchorKind::Hit),
            at_seconds: Rat::new(5, 2),
            timecode_hint: None,
        },
    ];

    // At 120 bpm: 1 beat = 0.5s; tick = 960000/quarter → 2s = 4 quarters.
    let ticks = s.anchor_ticks(120.0);
    assert_eq!(ticks[0], ("hit-door".into(), 4 * 960_000));
    assert_eq!(ticks[1], ("hit-window".into(), 5 * 960_000));

    // Change tempo to 60 bpm via op — seconds must not move.
    let tok = s
        .apply(&ScoreOp::SetTempoMap {
            points: vec![
                TempoPoint {
                    at_ticks: 0,
                    bpm: 60.0,
                },
                TempoPoint {
                    at_ticks: 960_000,
                    bpm: 90.0,
                },
            ],
        })
        .unwrap();
    assert_eq!(s.anchors[0].at_seconds, Rat::i64(2));
    assert_eq!(s.anchors[1].at_seconds, Rat::new(5, 2));

    // Tick positions rederived: 60bpm → 1s/quarter → 2s = 2 quarters;
    // second segment at 90bpm → 960000 ticks = 1.5s? no: 90bpm quarter=
    // 2/3 s. hit-window 2.5s: first 960000 ticks cover 0..1s at 60bpm,
    // remaining 1.5s at 90bpm = 1.5/(2/3)=2.25 quarters = 2160000 ticks.
    // hit-door 2s: 1s at 60bpm (960000 ticks) + 1s at 90bpm (1.5q =
    // 1440000) = 2400000. hit-window 2.5s: 960000 + 1.5s at 90bpm
    // (2.25q = 2160000) = 3120000.
    let ticks2 = s.anchor_ticks(120.0);
    assert_eq!(ticks2[0], ("hit-door".into(), 960_000 + 1_440_000));
    assert_eq!(ticks2[1], ("hit-window".into(), 960_000 + 2_160_000));

    // Undo restores the tempo map AND derived ticks.
    s.undo(&tok).unwrap();
    assert_eq!(s.anchor_ticks(120.0), ticks);
}

#[test]
fn anchors_timecode_modes() {
    // Declared modes exist and produce canonical labels.
    let df = TimecodeMode::FPS_2997_DF;
    // 1 frame at 29.97 DF = frame label 00:00:00;01.
    assert_eq!(format_timecode(1, &df), "00:00:00;01");
    // Non-drop uses ':' separators.
    assert_eq!(format_timecode(24 * 60 - 1, &TimecodeMode::FPS_24), "00:00:59:23");
    // Drop-frame math: at real-time 60s (frame 1798), the label is the
    // last frame of minute 0 — the "01:00:00/01" labels get skipped.
    let frame = timecode_frame(Rat::i64(60), &df);
    assert_eq!(format_timecode(frame, &df), "00:00:59;28");
    // Frame 1800 jumps over the two dropped labels → 01:00:02.
    assert_eq!(format_timecode(1800, &df), "00:01:00;02");
    // frame_seconds is exact per-frame; seconds→frame floors the
    // remainder, so a whole-frame boundary round-trips exactly.
    assert_eq!(timecode_frame(frame_seconds(1800, &df), &df), 1800);
    assert!(frame_seconds(frame, &df) < Rat::i64(60));
    // 59.94 DF drops 4.
    assert_eq!(format_timecode(1, &TimecodeMode::FPS_5994_DF), "00:00:00;01");
}

#[test]
fn timecode_persists_on_score() {
    let mut s = Score::new("TC");
    s.timecode = Some(TimecodeMode::FPS_25);
    let json = s.canonical_json().unwrap();
    let back: Score = serde_json::from_slice(&json).unwrap();
    assert_eq!(back.timecode, Some(TimecodeMode::FPS_25));
}

// ---------------------------------------------------------------------------
// Loss reports — deterministic, honest
// ---------------------------------------------------------------------------

#[test]
fn loss_report_entries_for_unsupported() {
    let (_s, loss) = import_musicxml(&fixture("guitar-lossy.musicxml")).unwrap();
    let aspects: Vec<String> = loss
        .entries
        .iter()
        .map(|e| format!("{}:{}", e.element, e.aspect))
        .collect();
    let text = aspects.join("\n");
    for expected in ["grace", "print", "bar-style", "repeat", "harmony", "trill-mark", "ornaments"] {
        assert!(text.contains(expected), "missing loss entry for {expected}:\n{text}");
    }
    assert!(loss.dropped_count() > 0);
}

#[test]
fn loss_report_deterministic() {
    let xml = fixture("guitar-lossy.musicxml");
    let (_a, l1) = import_musicxml(&xml).unwrap();
    let (_b, l2) = import_musicxml(&xml).unwrap();
    assert_eq!(
        l1.canonical_json().unwrap(),
        l2.canonical_json().unwrap(),
        "loss reports must be byte-identical for identical input"
    );
    // And export loss on an anchors-bearing score is deterministic too.
    let mut s = Score::new("x");
    s.anchors.push(Anchor {
        id: "a1".into(),
        label: String::new(),
        kind: None,
        at_seconds: Rat::i64(1),
        timecode_hint: None,
    });
    let (_x1, e1) = export_musicxml(&s).unwrap();
    let (_x2, e2) = export_musicxml(&s).unwrap();
    assert_eq!(e1.canonical_json().unwrap(), e2.canonical_json().unwrap());
    assert_eq!(e1.direction, Some(ExchangeDirection::Export));
    assert!(
        e1.entries.iter().any(|e| e.aspect.contains("anchor")),
        "anchors have no MusicXML carrier — must be reported lost"
    );
}

#[test]
fn odd_divisions_produce_approximation_loss() {
    let (_s, loss) = import_musicxml(&fixture("odd-divisions.musicxml")).unwrap();
    assert!(
        loss.entries
            .iter()
            .any(|e| e.kind == LossKind::Approximated && e.aspect.contains("duration")),
        "inexact divisions→ticks conversion must be reported approximate: {loss:?}"
    );
}

#[test]
fn proprietary_formats_stay_blocked() {
    // T91: unsupported proprietary conversion stays blocked — nothing
    // silently "succeeds". Binary garbage / non-XML input errors out.
    let blob: Vec<u8> = vec![0x00, 0x1e, 0x66, 0x6a, 0xff, 0x00, 0x42];
    let res = import_musicxml(std::str::from_utf8(&blob).unwrap_or("garbage"));
    // from_utf8 of that blob fails; pass a definitely-invalid string.
    let res2 = import_musicxml("PK\x03\x04 this is not xml at all <unclosed");
    assert!(res.is_err() || res2.is_err());
    assert!(res2.is_err());
}

// ---------------------------------------------------------------------------
// Serde / canonical form
// ---------------------------------------------------------------------------

#[test]
fn canonical_json_deterministic_and_stable() {
    let (a, _) = import_musicxml(&fixture("etude-c.musicxml")).unwrap();
    let j1 = a.canonical_json().unwrap();
    let j2 = a.canonical_json().unwrap();
    assert_eq!(j1, j2);
    let back: Score = serde_json::from_slice(&j1).unwrap();
    assert!(a.semantic_eq(&back));
    // String-int64: position ticks serialize as strings.
    let text = String::from_utf8(j1).unwrap();
    assert!(text.contains("\"offsetTicks\": \"0\""), "string-int64 contract");
}
