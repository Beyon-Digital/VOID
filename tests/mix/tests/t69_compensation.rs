//! T69 — per-node delay compensation. The model must (a) compute the
//! accumulated latency of every path, (b) emit per-edge delay values
//! that align late arrivals with the node's latest input, and (c) make
//! the impulse-alignment invariant verifiable: an impulse injected at
//! any source lands on the SAME output sample.

use void_mix::{
    compute_compensation, path_latency, simulate_impulse,
    simulate_raw_arrival, EdgeKind, MixError, Node, NodeKind,
    RoutingGraph, Send, ChannelLayout,
};
use void_mix_tests::support::*;

#[test]
fn path_latency_accumulates_each_nodes_declared_samples() {
    let g = two_path_graph(120, 0, 30);
    // ta: 120 (itself) + master(0) along the direct path
    assert_eq!(path_latency(&g, &["ta".into(), "master".into()]).unwrap(), 120);
    // tb(0) + aux(30) + master(0)
    assert_eq!(
        path_latency(&g, &["tb".into(), "aux".into(), "master".into()])
            .unwrap(),
        30
    );
    // an invented path errors instead of fabricating a number
    match path_latency(&g, &["ta".into(), "aux".into()]) {
        Err(MixError::InvalidSend(_)) => {}
        other => panic!("non-edge path must fail, got {other:?}"),
    }
}

#[test]
fn compensation_aligns_shorter_path_to_the_longest() {
    let g = two_path_graph(120, 0, 30);
    let plan = compute_compensation(&g).unwrap();
    // The direct path (120) dominates; the aux path (30) is 90 late.
    assert_eq!(plan.output_latency, 120);
    assert_eq!(plan.delay_for("tb", "aux", EdgeKind::Main), 0);
    assert_eq!(plan.delay_for("aux", "master", EdgeKind::Main), 90);
    assert_eq!(plan.delay_for("ta", "master", EdgeKind::Main), 0);
    assert_eq!(plan.accumulated("aux").unwrap(), 30);
    assert_eq!(plan.accumulated("master").unwrap(), 120);
}

#[test]
fn impulses_from_any_source_land_on_the_same_output_sample() {
    let g = two_path_graph(120, 0, 30);
    let plan = compute_compensation(&g).unwrap();
    let t0 = 10_000;
    let a = simulate_impulse(&g, &plan, "ta", t0).unwrap();
    let b = simulate_impulse(&g, &plan, "tb", t0).unwrap();
    let via_aux = simulate_impulse(&g, &plan, "aux", t0).unwrap();
    assert_eq!(a, t0 + plan.output_latency, "direct-path impulse");
    assert_eq!(b, a, "impulses must be sample-aligned regardless of path");
    // injecting at aux's input lands on the same sample: the impulse
    // enters after aux's own latency base, whose input arrival is 0
    // here (tb has no latency), so the aligned edge still sums to the
    // plan's output latency.
    assert_eq!(via_aux, t0 + plan.output_latency);
    // Raw (uncompensated) arrivals expose the misalignment being fixed.
    let raw_a = simulate_raw_arrival(&g, "ta", t0).unwrap();
    let raw_b = simulate_raw_arrival(&g, "tb", t0).unwrap();
    assert_eq!(raw_a, t0 + 120);
    assert_eq!(raw_b, t0 + 30, "raw path is 90 samples early — the bug PDC fixes");
    assert_eq!(raw_b + 90, raw_a);
}

#[test]
fn deep_chain_compensation_matches_hand_math() {
    // s1(10) -> m1(20) -> master(5); s2(50) -> master — longest wins.
    let mut g = RoutingGraph::new();
    g.add_node(Node::track("s1", "s1", ChannelLayout::Stereo).with_latency(10))
        .unwrap();
    g.add_node(Node::track("s2", "s2", ChannelLayout::Stereo).with_latency(50))
        .unwrap();
    g.add_node(
        Node::new("m1", NodeKind::AuxBus, "m1", ChannelLayout::Stereo)
            .with_latency(20),
    )
    .unwrap();
    g.add_node(
        Node::new("master", NodeKind::Master, "master", ChannelLayout::Stereo)
            .with_latency(5),
    )
    .unwrap();
    g.add_send(Send::post("s1", "m1")).unwrap();
    g.add_send(Send::post("m1", "master")).unwrap();
    g.add_send(Send::post("s2", "master")).unwrap();

    let plan = compute_compensation(&g).unwrap();
    // s1 chain: 10+20 = 30 arrives at master input; s2: 50. master waits for 50.
    assert_eq!(plan.accumulated("m1").unwrap(), 30);
    assert_eq!(plan.output_latency, 55); // 50 + master 5
    assert_eq!(plan.delay_for("m1", "master", EdgeKind::Main), 20);
    assert_eq!(plan.delay_for("s2", "master", EdgeKind::Main), 0);

    let a = simulate_impulse(&g, &plan, "s1", 0).unwrap();
    let b = simulate_impulse(&g, &plan, "s2", 0).unwrap();
    assert_eq!(a, 55);
    assert_eq!(b, 55);
}

#[test]
fn unmeasured_external_io_is_flagged_not_faked() {
    let g = external_io_graph(64, false);
    let plan = compute_compensation(&g).unwrap();
    // The plan still computes a number (declared value) but reports the
    // path as unmeasured — compensation is claimed only where measured.
    assert_eq!(plan.output_latency, 64);
    assert_eq!(plan.unmeasured_nodes, vec!["hw-loop".to_string()]);

    let g2 = external_io_graph(64, true);
    let plan2 = compute_compensation(&g2).unwrap();
    assert!(plan2.unmeasured_nodes.is_empty());
}

#[test]
fn compensation_rejects_cyclic_graphs_instead_of_hanging() {
    let mut g = basic_graph();
    g.add_node(Node::new(
        "x",
        NodeKind::AuxBus,
        "x",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("x", "x").downmix(false)).ok(); // self-loop rejected already
    g.add_send(Send::post("t1", "x")).unwrap();
    g.add_send(Send::post("x", "t1")).unwrap(); // t1 -> x -> t1 cycle
    match compute_compensation(&g) {
        Err(MixError::Loop(p)) => assert!(p.contains("t1") && p.contains('x')),
        other => panic!("expected Loop, got {other:?}"),
    }
}
