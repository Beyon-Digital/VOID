//! Per-node delay compensation (ENG-04 / T69).
//!
//! Model: latency lives on **nodes** — `latency_samples` is the node's
//! own declared processing latency (insert chain, device round-trip).
//! Edges carry no inherent latency; the compensation plan *assigns* a
//! delay to each edge so that every input of every summing node arrives
//! sample-aligned.
//!
//! Walked in topological order, for each node `n`:
//!
//! ```text
//! input_arrival(n)  = max over incoming edges e of arrival(e.from)
//! edge_delay(e)     = input_arrival(n) - arrival(e.from)   (>= 0)
//! arrival(n)        = input_arrival(n) + n.latency_samples  (accumulated
//!                     latency at n's output)
//! output_latency    = arrival(master)
//! ```
//!
//! Delaying every shorter input up to the longest is the classic PDC
//! scheme: inductively, an impulse injected at any node reaches Master
//! at exactly `output_latency` — which is what `simulate_impulse`
//! asserts in tests.
//!
//! ExternalIo nodes with unmeasured latency are still compensated by
//! their *declared* value but listed in `unmeasured_nodes` — T69 only
//! allows compensation claims for measured/supported bridge and device
//! paths.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{MixError, Result};
use crate::graph::{EdgeKind, NodeId, NodeKind, RoutingGraph};

/// Declared delay assigned to one edge by the compensation plan.
#[derive(Debug, Clone)]
pub struct EdgeDelay {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    pub delay_samples: i64,
}

/// Per-node compensation record.
#[derive(Debug, Clone)]
pub struct NodeCompensation {
    pub node: NodeId,
    /// Max incoming arrival — the node's aligned input time.
    pub input_arrival: i64,
    /// The node's own declared latency.
    pub own_latency: i64,
    /// `input_arrival + own_latency` — latency accumulated at output.
    pub accumulated: i64,
}

/// The compensation plan for a validated graph.
#[derive(Debug, Clone)]
pub struct CompensationPlan {
    /// Topological processing order (sources first, Master last).
    pub topo_order: Vec<NodeId>,
    /// One declared delay per send edge (0 allowed — explicit).
    pub edge_delays: Vec<EdgeDelay>,
    /// Per-node aligned input / accumulated output latencies.
    pub nodes: Vec<NodeCompensation>,
    /// Sample offset at which every injected impulse lands on the
    /// Master output after compensation.
    pub output_latency: i64,
    /// ExternalIo nodes whose declared latency was never measured —
    /// their contribution is computed but flagged unverified.
    pub unmeasured_nodes: Vec<NodeId>,
}

impl CompensationPlan {
    pub fn delay_for(&self, from: &str, to: &str, kind: EdgeKind) -> i64 {
        self.edge_delays
            .iter()
            .find(|e| e.from == from && e.to == to && e.kind == kind)
            .map(|e| e.delay_samples)
            .unwrap_or(0)
    }

    pub fn accumulated(&self, node: &str) -> Option<i64> {
        self.nodes
            .iter()
            .find(|n| n.node == node)
            .map(|n| n.accumulated)
    }
}

/// Compute the compensation plan. Fails on loops (propagated
/// [`MixError::Loop`]) or a missing/duplicated Master.
pub fn compute_compensation(g: &RoutingGraph) -> Result<CompensationPlan> {
    let topo = g.topo_order()?;
    let master = g.master_id()?.clone();

    let mut arrival: BTreeMap<NodeId, i64> = BTreeMap::new();
    let mut edge_delays: Vec<EdgeDelay> = Vec::new();
    let mut records: Vec<NodeCompensation> = Vec::new();
    let mut unmeasured: Vec<NodeId> = Vec::new();

    for id in &topo {
        let node = g.node(id).expect("topo ids are graph members");
        if node.kind == NodeKind::ExternalIo && !node.latency_measured {
            unmeasured.push(id.clone());
        }
        let incoming = g.incoming(id);
        let input_arrival = incoming
            .iter()
            .map(|e| arrival[e.from.as_str()])
            .max()
            .unwrap_or(0);
        for e in incoming {
            edge_delays.push(EdgeDelay {
                from: e.from.clone(),
                to: e.to.clone(),
                kind: e.kind,
                delay_samples: input_arrival - arrival[e.from.as_str()],
            });
        }
        let accumulated = input_arrival + node.latency_samples;
        arrival.insert(id.clone(), accumulated);
        records.push(NodeCompensation {
            node: id.clone(),
            input_arrival,
            own_latency: node.latency_samples,
            accumulated,
        });
    }

    Ok(CompensationPlan {
        topo_order: topo,
        edge_delays,
        nodes: records,
        output_latency: arrival[&master],
        unmeasured_nodes: unmeasured,
    })
}

/// Accumulated latency along an explicit node path (sum of node
/// latencies). Every consecutive pair must be connected by an edge —
/// an invented path errors instead of producing a number.
pub fn path_latency(g: &RoutingGraph, path: &[NodeId]) -> Result<i64> {
    if path.is_empty() {
        return Err(MixError::InvalidNode("empty path".into()));
    }
    let mut total = 0i64;
    for w in path.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if g.node(a).is_none() {
            return Err(MixError::UnknownNode(a.clone()));
        }
        if g.node(b).is_none() {
            return Err(MixError::UnknownNode(b.clone()));
        }
        if g.outgoing(a).iter().all(|e| e.to != *b) {
            return Err(MixError::InvalidSend(format!(
                "no send {a} -> {b} — '{b}' is not on a real path"
            )));
        }
        total += g.node(a).map(|n| n.latency_samples).unwrap_or(0);
    }
    total += g.node(&path[path.len() - 1]).unwrap().latency_samples;
    Ok(total)
}

/// Arrival sample at Master for an impulse injected at `injected`'s
/// input at sample `t0`, travelling the **raw** graph (no edge delays —
/// exposes the misalignment the plan corrects).
pub fn simulate_raw_arrival(g: &RoutingGraph, injected: &str, t0: i64) -> Result<i64> {
    simulate(g, injected, t0, |_, _, _| 0)
}

/// Arrival sample at Master for an impulse injected at `injected`'s
/// input at `t0` **with the plan applied**. For every node that reaches
/// Master this equals `plan.output_latency` — the alignment invariant.
pub fn simulate_impulse(
    g: &RoutingGraph,
    plan: &CompensationPlan,
    injected: &str,
    t0: i64,
) -> Result<i64> {
    simulate(g, injected, t0, |f, t, k| plan.delay_for(f, t, k))
}

fn simulate(
    g: &RoutingGraph,
    injected: &str,
    t0: i64,
    edge_delay: impl Fn(&str, &str, EdgeKind) -> i64,
) -> Result<i64> {
    if g.node(injected).is_none() {
        return Err(MixError::UnknownNode(injected.to_string()));
    }
    let topo = g.topo_order()?;
    let master = g.master_id()?.clone();

    // Reachable set: injected node and everything downstream of it.
    let mut reachable: BTreeSet<NodeId> = BTreeSet::new();
    let mut stack = vec![injected.to_string()];
    while let Some(n) = stack.pop() {
        if !reachable.insert(n.clone()) {
            continue;
        }
        for e in g.outgoing(&n) {
            stack.push(e.to.clone());
        }
    }
    if !reachable.contains(&master) {
        return Err(MixError::Unreachable(injected.to_string()));
    }

    let mut arrival: BTreeMap<NodeId, i64> = BTreeMap::new();
    for id in &topo {
        if !reachable.contains(id) {
            continue;
        }
        let node = g.node(id).expect("topo ids are graph members");
        let input = if id == injected {
            // The impulse enters at this node's input — its incoming
            // edges carry other signals, not this impulse.
            t0
        } else {
            g.incoming(id)
                .iter()
                .filter(|e| reachable.contains(&e.from))
                .map(|e| arrival[e.from.as_str()] + edge_delay(&e.from, &e.to, e.kind))
                .max()
                .unwrap_or(t0)
        };
        arrival.insert(id.clone(), input + node.latency_samples);
    }
    Ok(arrival[&master])
}
