//! Fixture builders for the T69 routing/compensation suite.

use void_mix::{
    ChannelLayout, Node, NodeKind, RoutingGraph, Send,
};

/// A minimal valid graph: two stereo tracks + master.
pub fn basic_graph() -> RoutingGraph {
    let mut g = RoutingGraph::new();
    g.add_node(Node::track("t1", "drums", ChannelLayout::Stereo))
        .unwrap();
    g.add_node(Node::track("t2", "bass", ChannelLayout::Stereo))
        .unwrap();
    g.add_node(Node::new(
        "master",
        NodeKind::Master,
        "master",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("t1", "master")).unwrap();
    g.add_send(Send::post("t2", "master")).unwrap();
    g
}

/// Graph with a parallel aux path of different latency:
///   tA(latency a) ──► master
///   tB(latency b) ──► aux(latency c) ──► master
/// `aux` gets its own node latency so the two paths disagree.
pub fn two_path_graph(a: i64, b: i64, c: i64) -> RoutingGraph {
    let mut g = RoutingGraph::new();
    g.add_node(
        Node::track("ta", "A", ChannelLayout::Stereo).with_latency(a),
    )
    .unwrap();
    g.add_node(
        Node::track("tb", "B", ChannelLayout::Stereo).with_latency(b),
    )
    .unwrap();
    g.add_node(
        Node::new("aux", NodeKind::AuxBus, "verb", ChannelLayout::Stereo)
            .with_latency(c),
    )
    .unwrap();
    g.add_node(Node::new(
        "master",
        NodeKind::Master,
        "master",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("ta", "master")).unwrap();
    g.add_send(Send::post("tb", "aux")).unwrap();
    g.add_send(Send::post("aux", "master")).unwrap();
    g
}

/// Graph with a declared-but-unmeasured external I/O node.
pub fn external_io_graph(latency: i64, measured: bool) -> RoutingGraph {
    let mut g = RoutingGraph::new();
    let mut ext = Node::new(
        "hw-loop",
        NodeKind::ExternalIo,
        "hw loop",
        ChannelLayout::Stereo,
    )
    .with_latency(latency);
    if measured {
        ext = ext.measured();
    }
    g.add_node(ext).unwrap();
    g.add_node(Node::new(
        "master",
        NodeKind::Master,
        "master",
        ChannelLayout::Stereo,
    ))
    .unwrap();
    g.add_send(Send::post("hw-loop", "master")).unwrap();
    g
}
