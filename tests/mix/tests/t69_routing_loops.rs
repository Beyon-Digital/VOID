//! T69 — routing graph: sends/aux/multi-out/external I-O, topological
//! validation, and explicit loop/feedback REJECTION. A cycle must
//! surface as `MixError::Loop` naming the loop — never a silent allow
//! or a hang.

use void_mix::{
    legality, ChannelLayout, EdgeKind, Legality, MixError, Node, NodeKind,
    RoutingGraph, Send, TapPoint, VcaGroup,
};
use void_mix_tests::support::*;

#[test]
fn sends_aux_vca_and_multiout_build_a_valid_graph() {
    let mut g = basic_graph();
    // aux bus fed by both tracks, summed to master
    g.add_node(Node::new(
        "verb",
        NodeKind::AuxBus,
        "verb",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("t1", "verb").tap(TapPoint::PreFader).gain(0.4))
        .unwrap();
    g.add_send(Send::post("t2", "verb").gain(0.25)).unwrap();
    g.add_send(Send::post("verb", "master")).unwrap();
    // multi-out node: one instrument, two outputs (kick out + rest)
    g.add_node(Node::new(
        "drum-inst",
        NodeKind::MultiOut,
        "drum inst",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("drum-inst", "t1")).unwrap();
    // external I/O loop descriptor
    g.add_node(
        Node::new(
            "comp",
            NodeKind::ExternalIo,
            "hw comp",
            ChannelLayout::Stereo,
        )
        .measured(),
    )
    .unwrap();
    g.add_send(Send::post("t1", "comp")).unwrap();
    g.add_send(Send::post("comp", "master")).unwrap();
    // VCA-style group: control only, not a summing node
    g.set_vca(VcaGroup {
        id: "drums-vca".into(),
        gain: 0.8,
        members: ["t1".to_string(), "t2".to_string()].into_iter().collect(),
    })
    .unwrap();

    let order = g.validate().expect("valid graph must sort");
    // every node appears once; aux lands before master (VCA is
    // control-only — it is not a graph node)
    assert_eq!(order.len(), 6);
    assert!(
        order.iter().position(|n| n == "verb").unwrap()
            < order.iter().position(|n| n == "master").unwrap()
    );
    assert!(
        order.iter().position(|n| n == "drum-inst").unwrap()
            < order.iter().position(|n| n == "t1").unwrap()
    );
    // multiplicative VCA on top of strip gain
    assert_eq!(g.effective_gain("t1", 1.0), 0.8);
    assert_eq!(g.effective_gain("t2", 0.5), 0.4);
}

#[test]
fn loop_back_into_an_aux_is_rejected_with_the_cycle_named() {
    let mut g = basic_graph();
    g.add_node(Node::new(
        "a",
        NodeKind::AuxBus,
        "a",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_node(Node::new(
        "b",
        NodeKind::AuxBus,
        "b",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("a", "b")).unwrap();
    g.add_send(Send::post("a", "master")).unwrap();
    g.add_send(Send::post("b", "master")).unwrap();
    // legal so far — now close a feedback ring
    g.add_send(Send::post("b", "a")).unwrap();

    match g.validate() {
        Err(MixError::Loop(path)) => {
            assert!(
                path.contains('a') && path.contains('b'),
                "error must name the loop, got: {path}"
            );
            // cycle reports a path that returns to its start
            let first = path.split(" -> ").next().unwrap();
            assert!(path.ends_with(first), "not a closed path: {path}");
        }
        other => panic!("expected MixError::Loop, got {other:?}"),
    }
}

#[test]
fn self_send_is_a_loop_at_insert_time() {
    let mut g = basic_graph();
    match g.add_send(Send::post("t1", "t1")) {
        Err(MixError::Loop(_)) => {}
        other => panic!("self-send must be a loop rejection, got {other:?}"),
    }
}

#[test]
fn duplicate_send_key_and_unknown_endpoints_error() {
    let mut g = basic_graph();
    match g.add_send(Send::post("t1", "master")) {
        Err(MixError::DuplicateSend { from, to, kind }) => {
            assert_eq!(from, "t1");
            assert_eq!(to, "master");
            assert_eq!(kind, EdgeKind::Main);
        }
        other => panic!("expected DuplicateSend, got {other:?}"),
    }
    // a Main and a SidechainKey send between the same pair are distinct
    g.add_send(Send::post("t1", "master").sidechain()).unwrap();
    match g.add_send(Send::post("t1", "nope")) {
        Err(MixError::UnknownNode(id)) => assert_eq!(id, "nope"),
        other => panic!("expected UnknownNode, got {other:?}"),
    }
}

#[test]
fn gain_pan_and_send_fields_are_validated() {
    let mut g = basic_graph();
    match g.add_send(Send::post("t1", "t2").gain(-0.5)) {
        Err(MixError::InvalidSend(_)) => {}
        other => panic!("negative gain must fail, got {other:?}"),
    }
    match g.add_send(Send::post("t1", "t2").pan(1.5)) {
        Err(MixError::InvalidSend(_)) => {}
        other => panic!("pan outside [-1,1] must fail, got {other:?}"),
    }
    match g.add_send(Send::post("t1", "t2").gain(f64::NAN)) {
        Err(MixError::InvalidSend(_)) => {}
        other => panic!("NaN gain must fail, got {other:?}"),
    }
    g.add_send(Send::post("t1", "t2").gain(0.9).pan(-0.3)).unwrap();
    g.validate().unwrap();
}

#[test]
fn second_master_and_missing_master_error() {
    let mut g = basic_graph();
    match g.add_node(Node::new(
        "master2",
        NodeKind::Master,
        "m2",
        ChannelLayout::Stereo,
    )) {
        Err(MixError::DuplicateMaster(_)) => {}
        other => panic!("second master must fail, got {other:?}"),
    }
    let mut orphan = RoutingGraph::new();
    orphan
        .add_node(Node::track("x", "x", ChannelLayout::Mono))
        .unwrap();
    match orphan.validate() {
        Err(MixError::MissingMaster) => {}
        other => panic!("missing master must fail, got {other:?}"),
    }
}

#[test]
fn channel_layout_matrix_allows_folds_rejects_upmix() {
    // identical layouts pass directly
    for l in [
        ChannelLayout::Mono,
        ChannelLayout::Stereo,
        ChannelLayout::Quad,
        ChannelLayout::Surround51,
        ChannelLayout::Surround71,
    ] {
        assert_eq!(legality(l, l), Legality::Direct, "{l:?} -> {l:?}");
    }
    // mono widens freely (dual-mono placement is a pan decision, legal)
    for to in [
        ChannelLayout::Stereo,
        ChannelLayout::Quad,
        ChannelLayout::Surround51,
        ChannelLayout::Surround71,
    ] {
        assert_eq!(legality(ChannelLayout::Mono, to), Legality::Direct);
    }
    // wider → narrower folds are declared downmixes
    assert_eq!(
        legality(ChannelLayout::Surround51, ChannelLayout::Stereo),
        Legality::RequiresDeclaredDownmix
    );
    assert_eq!(
        legality(ChannelLayout::Surround71, ChannelLayout::Surround51),
        Legality::RequiresDeclaredDownmix
    );
    assert_eq!(
        legality(ChannelLayout::Quad, ChannelLayout::Mono),
        Legality::RequiresDeclaredDownmix
    );
    // inventing channels (stereo into surround) is illegal
    for to in [
        ChannelLayout::Quad,
        ChannelLayout::Surround51,
        ChannelLayout::Surround71,
    ] {
        assert_eq!(
            legality(ChannelLayout::Stereo, to),
            Legality::Illegal,
            "Stereo -> {to:?} must be illegal"
        );
    }
    // quad into 5.1/7.1 (missing LFE/side pair) is illegal
    assert_eq!(
        legality(ChannelLayout::Quad, ChannelLayout::Surround51),
        Legality::Illegal
    );
    assert_eq!(
        legality(ChannelLayout::Quad, ChannelLayout::Surround71),
        Legality::Illegal
    );
}

#[test]
fn illegal_send_layout_is_a_typed_error_at_add_time() {
    let mut g = RoutingGraph::new();
    g.add_node(Node::track("st", "st", ChannelLayout::Stereo))
        .unwrap();
    g.add_node(Node::new(
        "sur",
        NodeKind::AuxBus,
        "sur",
        ChannelLayout::Surround51,
    ))
    .unwrap();
    g.add_node(Node::new(
        "master",
        NodeKind::Master,
        "master",
        ChannelLayout::Surround71,
    ))
    .unwrap();
    match g.add_send(Send::post("st", "sur")) {
        Err(MixError::Layout { from, to, .. }) => {
            assert_eq!(from, ChannelLayout::Stereo);
            assert_eq!(to, ChannelLayout::Surround51);
        }
        other => panic!("expected Layout error, got {other:?}"),
    }
    // ...unless the downmix/widening is declared? No — stereo→5.1 is
    // Illegal outright; declared downmix applies to fold-downs only:
    g.add_node(Node::track("m", "m", ChannelLayout::Mono)).unwrap();
    g.add_send(Send::post("m", "sur")).unwrap(); // mono → surround: Direct
    match g.add_send(Send::post("sur", "m")) {
        Err(MixError::Layout { .. }) => {} // expected: undeclared fold-down
        other => panic!("undeclared fold-down must be a Layout error, got {other:?}"),
    }
    // declared fold-down passes
    g.add_send(Send::post("sur", "m").downmix(true)).unwrap();
}
