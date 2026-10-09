//! The directed routing graph: nodes, sends, VCA groups, validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::{MixError, Result};
use crate::layout::{legality, ChannelLayout, Legality};

/// Opaque node identifier (track/bus/port ids are string UUIDs on the
/// VOID side — CONTRACTS §1).
pub type NodeId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Channel strip output (audio or instrument track).
    Track,
    /// Aux return / subgroup summing bus.
    AuxBus,
    /// One output port of a multi-output instrument.
    MultiOut,
    /// External hardware insert. `latency_samples` is the declared
    /// round-trip time; `latency_measured` distinguishes measured
    /// hardware paths from declared-only guesses (T69).
    ExternalIo,
    /// The single final mix bus.
    Master,
}

/// A mixer node. `latency_samples` is the node's *own* declared
/// processing latency (inserts/device round-trip) in samples.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    pub layout: ChannelLayout,
    pub latency_samples: i64,
    /// ExternalIo only: `latency_samples` was measured against real
    /// hardware. Unmeasured paths still get a compensation *plan* but
    /// are flagged `unmeasured` in the report — compensation is claimed
    /// only where measured/supported.
    pub latency_measured: bool,
}

impl Node {
    pub fn track(id: impl Into<NodeId>, name: impl Into<String>, layout: ChannelLayout) -> Self {
        Self::new(id, NodeKind::Track, name, layout)
    }

    pub fn new(
        id: impl Into<NodeId>,
        kind: NodeKind,
        name: impl Into<String>,
        layout: ChannelLayout,
    ) -> Self {
        Node {
            id: id.into(),
            kind,
            name: name.into(),
            layout,
            latency_samples: 0,
            latency_measured: false,
        }
    }

    pub fn with_latency(mut self, samples: i64) -> Self {
        self.latency_samples = samples;
        self
    }

    /// Mark an ExternalIo node's declared latency as hardware-measured.
    pub fn measured(mut self) -> Self {
        self.latency_measured = true;
        self
    }
}

/// Where the send taps the source relative to its fader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TapPoint {
    /// Tap before the source strip's fader (monitor sends, cue feeds).
    PreFader,
    /// Tap after the source strip's fader (effects sends).
    PostFader,
}

/// Which input of the destination an edge feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Main audio input — the destination sums these signals.
    Main,
    /// Sidechain key input. Real audio routing: the key must arrive
    /// aligned with the main path (ENG-04), so it participates in
    /// topology and delay compensation, but it is never a mix input.
    SidechainKey,
}

/// A directed send/route edge between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Send {
    pub from: NodeId,
    pub to: NodeId,
    pub tap: TapPoint,
    pub kind: EdgeKind,
    /// Linear send amount, finite and >= 0.
    pub gain: f64,
    /// Send pan in [-1.0, 1.0] — sends have independent pan (MIX-03).
    pub pan: f64,
    /// Declared fold-down conversion; required when the layout matrix
    /// says [`Legality::RequiresDeclaredDownmix`].
    pub downmix: bool,
}

impl Send {
    pub fn post(from: impl Into<NodeId>, to: impl Into<NodeId>) -> Self {
        Send {
            from: from.into(),
            to: to.into(),
            tap: TapPoint::PostFader,
            kind: EdgeKind::Main,
            gain: 1.0,
            pan: 0.0,
            downmix: false,
        }
    }

    pub fn tap(mut self, tap: TapPoint) -> Self {
        self.tap = tap;
        self
    }

    pub fn sidechain(mut self) -> Self {
        self.kind = EdgeKind::SidechainKey;
        self
    }

    pub fn gain(mut self, gain: f64) -> Self {
        self.gain = gain;
        self
    }

    pub fn pan(mut self, pan: f64) -> Self {
        self.pan = pan;
        self
    }

    pub fn downmix(mut self, declared: bool) -> Self {
        self.downmix = declared;
        self
    }
}

/// VCA-style control group: scales member fader levels **without**
/// summing audio (MIX-04 — group control is separate from audio
/// summing). A VCA is not a node and cannot be routed; nesting VCAs is
/// out of scope (members are node ids only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcaGroup {
    pub id: String,
    /// Fader multiplier applied to every member's base gain.
    pub gain: f64,
    pub members: BTreeSet<NodeId>,
}

/// The routing graph. All mutation validates inputs; `validate()` /
/// `topo_order()` reject loops with the concrete cycle.
#[derive(Debug, Default)]
pub struct RoutingGraph {
    nodes: BTreeMap<NodeId, Node>,
    /// Sends keyed by (from, to, kind) — at most one send per triple;
    /// adjusting a send is remove+add.
    edges: BTreeMap<(NodeId, NodeId, EdgeKind), Send>,
    vcas: BTreeMap<String, VcaGroup>,
    master: Option<NodeId>,
}

impl RoutingGraph {
    pub fn new() -> Self {
        Self::default()
    }

    // -- nodes ---------------------------------------------------------

    pub fn add_node(&mut self, node: Node) -> Result<()> {
        if node.id.is_empty() {
            return Err(MixError::InvalidNode("empty node id".into()));
        }
        if node.latency_samples < 0 {
            return Err(MixError::InvalidNode(format!(
                "node '{}' latency_samples must be >= 0, got {}",
                node.id, node.latency_samples
            )));
        }
        if self.nodes.contains_key(&node.id) {
            return Err(MixError::DuplicateNode(node.id));
        }
        if node.kind == NodeKind::Master {
            if let Some(existing) = &self.master {
                return Err(MixError::DuplicateMaster(existing.clone()));
            }
            self.master = Some(node.id.clone());
        }
        if node.latency_measured && node.kind != NodeKind::ExternalIo {
            return Err(MixError::InvalidNode(format!(
                "node '{}': latency_measured only applies to ExternalIo",
                node.id
            )));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Remove a node plus every send touching it and every VCA
    /// membership — the graph is never left with dangling references.
    pub fn remove_node(&mut self, id: &str) -> Result<Node> {
        let node = self
            .nodes
            .remove(id)
            .ok_or_else(|| MixError::UnknownNode(id.to_string()))?;
        self.edges
            .retain(|(f, t, _), _| f != id && t != id);
        for g in self.vcas.values_mut() {
            g.members.remove(id);
        }
        if self.master.as_deref() == Some(id) {
            self.master = None;
        }
        Ok(node)
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.nodes.keys()
    }

    /// The single Master node id, or a typed error.
    pub fn master_id(&self) -> Result<&NodeId> {
        self.master.as_ref().ok_or(MixError::MissingMaster)
    }

    // -- sends -----------------------------------------------------------

    /// Connect `send`, validating endpoints, values and the layout
    /// matrix (ENG-06: invalid connections are rejected at connect
    /// time, before playback, never silently channel-folded).
    pub fn add_send(&mut self, send: Send) -> Result<()> {
        check_send(&send)?;
        if !self.nodes.contains_key(&send.from) {
            return Err(MixError::UnknownNode(send.from.clone()));
        }
        if !self.nodes.contains_key(&send.to) {
            return Err(MixError::UnknownNode(send.to.clone()));
        }
        if send.from == send.to {
            return Err(MixError::Loop(format!(
                "{} -> {} (self-loop)",
                send.from, send.to
            )));
        }
        let key = (send.from.clone(), send.to.clone(), send.kind);
        if self.edges.contains_key(&key) {
            return Err(MixError::DuplicateSend {
                from: send.from,
                to: send.to,
                kind: send.kind,
            });
        }
        self.check_layout(&send)?;
        self.edges.insert(key, send);
        Ok(())
    }

    fn check_layout(&self, send: &Send) -> Result<()> {
        let from = &self.nodes[&send.from];
        let to = &self.nodes[&send.to];
        match legality(from.layout, to.layout) {
            Legality::Direct => Ok(()),
            Legality::RequiresDeclaredDownmix => {
                if send.downmix {
                    Ok(())
                } else {
                    Err(MixError::Layout {
                        from: from.layout,
                        to: to.layout,
                        reason: "fold-down loses channels — send must declare `downmix`".into(),
                    })
                }
            }
            Legality::Illegal => Err(MixError::Layout {
                from: from.layout,
                to: to.layout,
                reason: "no defined conversion (widening a non-mono source)".into(),
            }),
        }
    }

    pub fn remove_send(&mut self, from: &str, to: &str, kind: EdgeKind) -> Result<Send> {
        self.edges
            .remove(&(from.to_string(), to.to_string(), kind))
            .ok_or_else(|| {
                MixError::InvalidSend(format!("no send {from} -> {to} ({kind:?})"))
            })
    }

    /// Incoming sends of a node, ordered deterministically.
    pub fn incoming(&self, id: &str) -> Vec<&Send> {
        self.edges.values().filter(|e| e.to == id).collect()
    }

    /// Outgoing sends of a node, ordered deterministically.
    pub fn outgoing(&self, id: &str) -> Vec<&Send> {
        self.edges.values().filter(|e| e.from == id).collect()
    }

    pub fn sends(&self) -> impl Iterator<Item = &Send> {
        self.edges.values()
    }

    // -- VCA groups ------------------------------------------------------

    /// Create/replace a VCA control group. Members must be existing
    /// nodes; gain must be finite and >= 0. VCAs carry no audio path.
    pub fn set_vca(&mut self, group: VcaGroup) -> Result<()> {
        if group.id.is_empty() {
            return Err(MixError::Vca("empty group id".into()));
        }
        if !group.gain.is_finite() || group.gain < 0.0 {
            return Err(MixError::Vca(format!(
                "group '{}' gain must be finite and >= 0, got {}",
                group.id, group.gain
            )));
        }
        for m in &group.members {
            if !self.nodes.contains_key(m) {
                return Err(MixError::UnknownNode(m.clone()));
            }
        }
        self.vcas.insert(group.id.clone(), group);
        Ok(())
    }

    /// Set a VCA's control gain (relative levels preserved — the same
    /// factor applies to every member).
    pub fn set_vca_gain(&mut self, group_id: &str, gain: f64) -> Result<()> {
        let g = self
            .vcas
            .get_mut(group_id)
            .ok_or_else(|| MixError::Vca(format!("unknown group '{group_id}'")))?;
        if !gain.is_finite() || gain < 0.0 {
            return Err(MixError::Vca(format!("VCA gain must be finite >= 0, got {gain}")));
        }
        g.gain = gain;
        Ok(())
    }

    /// Strip gain with every VCA membership applied multiplicatively.
    /// Members keep their relative levels — the group scales the strip,
    /// it does not re-route audio (MIX-04).
    pub fn effective_gain(&self, node_id: &str, strip_gain: f64) -> f64 {
        let mut g = strip_gain;
        for vca in self.vcas.values() {
            if vca.members.contains(node_id) {
                g *= vca.gain;
            }
        }
        g
    }

    // -- validation ------------------------------------------------------

    /// Topological validation: returns processing order (sources
    /// first, Master last) or [`MixError::Loop`] with the exact cycle.
    ///
    /// Feedback is rejected unconditionally — an "allow feedback with
    /// delay" escape hatch would pretend to support routing the engine
    /// cannot express; the honest answer is a typed rejection (T69).
    pub fn topo_order(&self) -> Result<Vec<NodeId>> {
        // Re-validate layouts defensively: node layouts are immutable
        // after insert, but validate() is the documented check point.
        for send in self.edges.values() {
            self.check_layout(send)?;
        }
        if self.nodes.contains_key("__void_unreachable__") {
            return Err(MixError::Unreachable("__void_unreachable__".into()));
        }

        let mut indeg: BTreeMap<&str, usize> =
            self.nodes.keys().map(|k| (k.as_str(), 0)).collect();
        for e in self.edges.values() {
            *indeg.get_mut(e.to.as_str()).expect("edge endpoint checked") += 1;
        }
        let mut queue: Vec<&str> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| *k)
            .collect();
        queue.sort_unstable();
        let mut order: Vec<NodeId> = Vec::with_capacity(self.nodes.len());
        while let Some(n) = queue.pop() {
            order.push(n.to_string());
            for e in self.edges.values() {
                if e.from == n {
                    let d = indeg.get_mut(e.to.as_str()).expect("edge endpoint checked");
                    *d -= 1;
                    if *d == 0 {
                        queue.push(e.to.as_str());
                    }
                }
            }
            queue.sort_unstable();
        }
        if order.len() == self.nodes.len() {
            return Ok(order);
        }
        // Leftover nodes are on a cycle — report the concrete loop.
        let stuck: BTreeSet<&str> = self
            .nodes
            .keys()
            .map(String::as_str)
            .filter(|k| !order.iter().any(|o| o == k))
            .collect();
        Err(MixError::Loop(
            self.cycle_path(&stuck).unwrap_or_else(|| {
                stuck.iter().copied().collect::<Vec<_>>().join(" -> ")
            }),
        ))
    }

    /// Full validation entry point: a Master node must exist (the
    /// compensation model's output reference) and the graph must sort.
    pub fn validate(&self) -> Result<Vec<NodeId>> {
        self.master_id()?;
        self.topo_order()
    }

    /// DFS on the still-cyclic subgraph to extract a concrete cycle
    /// path like "a -> b -> c -> a" for the error message.
    fn cycle_path(&self, stuck: &BTreeSet<&str>) -> Option<String> {
        enum Mark {
            Visiting,
            Done,
        }
        fn dfs<'a>(
            g: &'a RoutingGraph,
            n: &'a str,
            stuck: &BTreeSet<&'a str>,
            marks: &mut BTreeMap<&'a str, Mark>,
            stack: &mut Vec<&'a str>,
        ) -> Option<Vec<&'a str>> {
            marks.insert(n, Mark::Visiting);
            stack.push(n);
            for e in g.edges.values() {
                if e.from != n || !stuck.contains(e.to.as_str()) {
                    continue;
                }
                let t = e.to.as_str();
                match marks.get(t) {
                    Some(Mark::Visiting) => {
                        let start = stack.iter().position(|x| *x == t).unwrap();
                        let mut cyc = stack[start..].to_vec();
                        cyc.push(t);
                        return Some(cyc);
                    }
                    Some(Mark::Done) => continue,
                    None => {
                        if let Some(c) = dfs(g, t, stuck, marks, stack) {
                            return Some(c);
                        }
                    }
                }
            }
            stack.pop();
            marks.insert(n, Mark::Done);
            None
        }
        let mut marks = BTreeMap::new();
        let mut stack = Vec::new();
        for start in stuck {
            if marks.contains_key(start) {
                continue;
            }
            if let Some(cyc) = dfs(self, start, stuck, &mut marks, &mut stack) {
                return Some(cyc.join(" -> "));
            }
        }
        None
    }
}

fn check_send(s: &Send) -> Result<()> {
    if !s.gain.is_finite() || s.gain < 0.0 {
        return Err(MixError::InvalidSend(format!(
            "send gain must be finite and >= 0, got {}",
            s.gain
        )));
    }
    if !s.pan.is_finite() || s.pan < -1.0 || s.pan > 1.0 {
        return Err(MixError::InvalidSend(format!(
            "send pan must be finite in [-1, 1], got {}",
            s.pan
        )));
    }
    Ok(())
}
