//! void-mix — W18 mixer routing, delay compensation and channel-layout
//! validation (T69).
//!
//! The crate models the mixer as a **directed routing graph**:
//!
//! - [`NodeKind::Track`] — a channel strip (audio/instrument output).
//! - [`NodeKind::AuxBus`] — an aux return / subgroup summing bus.
//! - [`NodeKind::MultiOut`] — one output port of a multi-output
//!   instrument (each port routes independently, MIX-04).
//! - [`NodeKind::ExternalIo`] — an external-hardware insert; its
//!   `latency_samples` is the declared round-trip latency and is only
//!   *trusted* when `latency_measured` is set (T69: bridge/device paths
//!   are compensated only where measured/supported).
//! - [`NodeKind::Master`] — the single final mix bus.
//!
//! [`Send`] edges carry pre/post-fader taps, gain/pan and an optional
//! declared downmix. Sidechain key inputs are first-class edges: they
//! participate in topology and delay alignment because key timing is
//! real audio routing (ENG-04). VCA groups are **control-only** —
//! they scale member gains and never sum audio, so they are not nodes.
//!
//! Validation is explicit, never silent:
//!
//! - [`RoutingGraph::topo_order`] rejects feedback/loops with a typed
//!   [`MixError::Loop`] carrying the actual cycle (T69).
//! - [`legality`] is the mono/stereo/surround connection matrix;
//!   illegal connections fail `add_send` and `validate` with
//!   [`MixError::Layout`] (ENG-06: no silent channel loss).
//! - [`compute_compensation`] produces a [`CompensationPlan`]: the
//!   declared per-edge delay needed so every signal arrives
//!   sample-aligned at every summing node. [`simulate_impulse`] lets
//!   tests verify alignment numerically — an impulse injected at any
//!   source lands on the same output sample.

mod error;
mod graph;
mod latency;
mod layout;

pub use error::{MixError, Result};
pub use graph::{
    EdgeKind, Node, NodeId, NodeKind, RoutingGraph, Send, TapPoint, VcaGroup,
};
pub use latency::{
    compute_compensation, path_latency, simulate_impulse, simulate_raw_arrival,
    CompensationPlan, EdgeDelay, NodeCompensation,
};
pub use layout::{legality, ChannelLayout, Legality};
