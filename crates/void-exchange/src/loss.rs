//! Deterministic loss reporting (T77).
//!
//! Every import/export emits a `LossReport`: a sorted, per-element list of
//! what was dropped entirely and what was approximated (kept, but not
//! bit-for-bit). Determinism is contractual — the same input must produce
//! a byte-identical canonical report — so callers can diff, gate and
//! surface it without heuristics.
//!
//! The serialized shape is camelCase JSON mirroring the studio DTO in
//! `packages/void-studio/src/exchange/` (string-int64 rules do not apply —
//! reports carry no int64 fields).

use serde::{Deserialize, Serialize};

/// Direction the report describes. `import` = foreign → VOID document,
/// `export` = VOID document → foreign format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExchangeDirection {
    Import,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LossKind {
    /// Element could not be represented at all.
    Dropped,
    /// Element was represented with reduced fidelity (rounding, unit or
    /// semantic mapping); the reason says exactly what changed.
    Approximated,
}

/// One line of the report. `element` is a stable identifier — the source
/// element's id when it has one, else a structural path like
/// `structure/track[3]/devices/device[0]`. Never a raw filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LossEntry {
    pub element: String,
    /// Which aspect of the element was lost (e.g. "sends", "warps",
    /// "note.releaseVelocity", "clip.enabled").
    pub aspect: String,
    pub kind: LossKind,
    /// Human-readable, deterministic reason — no timestamps.
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LossReport {
    #[serde(default)]
    pub direction: Option<ExchangeDirection>,
    /// Sorted canonically on emit — see `sorted()`.
    pub entries: Vec<LossEntry>,
}

impl LossReport {
    pub const FORMAT: &'static str = "void-loss/1";

    pub fn new(direction: ExchangeDirection) -> Self {
        Self {
            direction: Some(direction),
            entries: Vec::new(),
        }
    }

    pub fn drop(
        &mut self,
        element: impl Into<String>,
        aspect: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.entries.push(LossEntry {
            element: element.into(),
            aspect: aspect.into(),
            kind: LossKind::Dropped,
            reason: reason.into(),
        });
    }

    pub fn approximate(
        &mut self,
        element: impl Into<String>,
        aspect: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.entries.push(LossEntry {
            element: element.into(),
            aspect: aspect.into(),
            kind: LossKind::Approximated,
            reason: reason.into(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn dropped_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind == LossKind::Dropped)
            .count()
    }

    pub fn approximated_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind == LossKind::Approximated)
            .count()
    }

    /// Canonically sorted copy (element, aspect, kind, reason). Emitted
    /// reports are always produced through this ordering.
    pub fn sorted(&self) -> Self {
        let mut out = self.clone();
        out.entries.sort_by(|a, b| {
            (&a.element, &a.aspect, a.kind, &a.reason)
                .cmp(&(&b.element, &b.aspect, b.kind, &b.reason))
        });
        out
    }

    /// Canonical JSON bytes — stable key order via the struct layout and
    /// canonically sorted entries. Two runs over equal input MUST produce
    /// byte-identical output (T77).
    pub fn canonical_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec_pretty(&self.sorted())
    }

    /// Parse a report produced by `canonical_json` (or a peer lane's
    /// equivalent writer). Unknown fields are ignored; malformed input is
    /// an `Err`, never silently an empty report.
    pub fn parse(bytes: &[u8]) -> crate::error::Result<Self> {
        let mut report: LossReport = serde_json::from_slice(bytes).map_err(|e| {
            crate::error::ExchangeError::Malformed(format!("loss report json: {e}"))
        })?;
        // Re-sort so a non-canonical source still yields a canonical report.
        report.entries.sort_by(|a, b| {
            (&a.element, &a.aspect, a.kind, &a.reason)
                .cmp(&(&b.element, &b.aspect, b.kind, &b.reason))
        });
        Ok(report)
    }
}
