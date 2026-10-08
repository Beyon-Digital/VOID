//! Typed mixer errors — failures are surfaced, never clamped into a
//! "supported" state.

use thiserror::Error;

use crate::layout::ChannelLayout;

pub type Result<T> = std::result::Result<T, MixError>;

#[derive(Debug, Error)]
pub enum MixError {
    #[error("duplicate node id '{0}'")]
    DuplicateNode(String),

    #[error("unknown node '{0}'")]
    UnknownNode(String),

    #[error("a Master node already exists ('{0}'); exactly one is allowed")]
    DuplicateMaster(String),

    #[error("no Master node in graph — compensation has no output reference")]
    MissingMaster,

    #[error("routing loop/feedback rejected: {0}")]
    Loop(String),

    #[error("channel-layout mismatch: {from:?} cannot feed {to:?} ({reason})")]
    Layout {
        from: ChannelLayout,
        to: ChannelLayout,
        reason: String,
    },

    #[error("duplicate send {from} -> {to} ({kind:?}) already exists")]
    DuplicateSend {
        from: String,
        to: String,
        kind: crate::graph::EdgeKind,
    },

    #[error("invalid send: {0}")]
    InvalidSend(String),

    #[error("invalid node: {0}")]
    InvalidNode(String),

    #[error("vca group error: {0}")]
    Vca(String),

    #[error("node '{0}' has no route to the Master output")]
    Unreachable(String),
}
