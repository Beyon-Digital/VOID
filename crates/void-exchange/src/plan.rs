//! Document → wire-op plan (T77 application leg).
//!
//! `plan_import` walks an imported `ExchangeDocument` and emits the
//! ordered `void_control` ops a coordinator would send, PLUS a loss
//! report covering everything the wire cannot express today — plugin
//! state restore (InsertPluginOp carries no state field), fades
//! (NEEDS #18), note channel/release velocity, clip-enable, per-clip
//! offset, markers, loop range. The plan is a data structure, not a
//! stream: the coordinator applies it inside its usual single-transaction
//! boundaries.

use crate::document::*;
use crate::loss::{ExchangeDirection, LossReport};
use uuid::Uuid;

/// Deterministic id for imported elements (uuid v5 on a path string) —
/// the same source document must produce the same ids.
pub fn deterministic_id(scope: &str, kind: &str, ordinal: usize) -> String {
    const NS: Uuid = Uuid::from_u128(0x764f_4944_0000_5000_8000_0000_0000_0000); // "vOID" namespace
    Uuid::new_v5(&NS, format!("{scope}/{kind}/{ordinal}").as_bytes()).to_string()
}

/// A planned wire op — the coordinator's apply order. Strings mirror the
/// `void_control.fbs` union member names; payloads are JSON values so the
/// plan is testable without linking void-protocol.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedOp {
    /// fbs union member, e.g. "CreateTrackOp".
    pub op: String,
    /// Wire-shaped payload (string-int64 fields already stringified).
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportPlan {
    pub ops: Vec<PlannedOp>,
    pub loss: LossReport,
}

/// Emit the ordered op plan + honest loss for `doc`. Order: transport
/// setup → tracks → clips → notes → plugins → params. Multi-edit segments
/// (a track's clips+notes) are contiguous so the coordinator can wrap
/// them in one transaction.
pub fn plan_import(doc: &ExchangeDocument) -> ImportPlan {
    let mut ops: Vec<PlannedOp> = Vec::new();
    let mut loss = LossReport::new(ExchangeDirection::Import);
    fn lid(loss: &mut LossReport, element: String, aspect: &str, why: impl Into<String>) {
        loss.drop(element, aspect.to_string(), why.into());
    }

    // Transport: SetTempoOp per tempo-map point; SetTimeSignatureOp.
    for tp in &doc.tempo_map {
        ops.push(PlannedOp {
            op: "SetTempoOp".into(),
            payload: serde_json::json!({
                "atTicks": tp.at_ticks.to_string(),
                "bpm": tp.bpm,
            }),
        });
    }
    for ts in &doc.time_signatures {
        ops.push(PlannedOp {
            op: "SetTimeSignatureOp".into(),
            payload: serde_json::json!({
                "atTicks": ts.at_ticks.to_string(),
                "numerator": ts.numerator,
                "denominator": ts.denominator,
            }),
        });
    }
    if let Some(lr) = &doc.loop_range {
        if lr.enabled {
            lid(
                &mut loss,
                "document".into(),
                "loopRange",
                "no wire op for loop range (transport loop set is NEEDS-level)",
            );
        }
    }
    for m in &doc.markers {
        lid(
            &mut loss,
            format!("marker {:?}", m.name),
            "markers",
            "no wire op for markers",
        );
    }

    for tr in &doc.tracks {
        ops.push(PlannedOp {
            op: "CreateTrackOp".into(),
            payload: serde_json::json!({
                "trackId": tr.id,
                "name": tr.name,
                "kind": format!("{:?}", tr.kind).to_uppercase(),
            }),
        });
        if tr.color.is_some() {
            lid(
                &mut loss,
                format!("track {:?}", tr.id),
                "color",
                "no wire op for track color",
            );
        }
        if tr.gain_linear != 1.0 {
            ops.push(PlannedOp {
                op: "SetGainOp".into(),
                payload: serde_json::json!({
                    "trackId": tr.id, "gainLinear": tr.gain_linear,
                }),
            });
        }
        if tr.pan != 0.0 {
            ops.push(PlannedOp {
                op: "SetPanOp".into(),
                payload: serde_json::json!({
                    "trackId": tr.id, "pan": tr.pan,
                }),
            });
        }
        if tr.muted {
            ops.push(PlannedOp {
                op: "SetMuteOp".into(),
                payload: serde_json::json!({
                    "trackId": tr.id, "muted": true,
                }),
            });
        }
        if tr.soloed {
            ops.push(PlannedOp {
                op: "SetSoloOp".into(),
                payload: serde_json::json!({
                    "trackId": tr.id, "soloed": true,
                }),
            });
        }

        for c in &tr.clips {
            ops.push(PlannedOp {
                op: "CreateClipOp".into(),
                payload: serde_json::json!({
                    "trackId": tr.id,
                    "clipId": c.id,
                    "startTicks": c.start_ticks.to_string(),
                    "lengthTicks": c.length_ticks.to_string(),
                }),
            });
            if !c.name.is_empty() {
                lid(
                    &mut loss,
                    format!("clip {:?}", c.id),
                    "name",
                    "no wire op for clip name",
                );
            }
            if c.offset_ticks != 0 {
                lid(
                    &mut loss,
                    format!("clip {:?}", c.id),
                    "offsetTicks",
                    "no wire op for clip content offset",
                );
            }
            if !c.enabled {
                lid(
                    &mut loss,
                    format!("clip {:?}", c.id),
                    "enabled",
                    "no wire op for clip enable/disable",
                );
            }
            if c.fade_in_ticks.is_some() || c.fade_out_ticks.is_some() {
                lid(
                    &mut loss,
                    format!("clip {:?}", c.id),
                    "fades",
                    "no wire op for clip fades (NEEDS #18)",
                );
            }
            match &c.content {
                ClipContent::Audio { asset_id } => {
                    let missing = doc
                        .assets
                        .iter()
                        .find(|a| a.asset_id == *asset_id)
                        .map(|a| a.missing)
                        .unwrap_or(true);
                    if missing {
                        lid(
                            &mut loss,
                            format!("clip {:?}", c.id),
                            "audioAsset",
                            format!("referenced asset {asset_id:?} absent — clip shell only"),
                        );
                    } else {
                        ops.push(PlannedOp {
                            op: "AttachAudioOp".into(),
                            payload: serde_json::json!({
                                "clipId": c.id, "assetId": asset_id,
                            }),
                        });
                    }
                }
                ClipContent::Notes { notes } => {
                    for n in notes {
                        ops.push(PlannedOp {
                            op: "InsertNoteOp".into(),
                            payload: serde_json::json!({
                                "clipId": c.id,
                                "noteId": n.id,
                                "pitch": n.pitch,
                                "velocity": n.velocity,
                                "startTicks": n.start_ticks.to_string(),
                                "lengthTicks": n.length_ticks.to_string(),
                            }),
                        });
                        if n.channel != 0 {
                            loss.approximate(
                                format!("note {:?}", n.id),
                                "channel",
                                format!(
                                    "channel {} not expressible on wire InsertNoteOp",
                                    n.channel
                                ),
                            );
                        }
                        if let Some(rv) = n.release_velocity {
                            loss.approximate(
                                format!("note {:?}", n.id),
                                "releaseVelocity",
                                format!("release velocity {rv} has no wire field"),
                            );
                        }
                    }
                }
            }
        }

        for p in &tr.plugins {
            // InsertPluginOp has no state field: the descriptor goes over,
            // the preserved blob stays in the missing-plugin store and is
            // rehydrated when a native host exists (T77 leg).
            ops.push(PlannedOp {
                op: "InsertPluginOp".into(),
                payload: serde_json::json!({
                    "trackId": tr.id,
                    "slot": p.slot,
                    "pluginInstanceId": p.instance_id,
                    "format": p.descriptor.format.as_str(),
                    "pluginUid": p.descriptor.plugin_uid,
                }),
            });
            if p.state.is_some() {
                loss.approximate(
                    format!("plugin {:?}", p.instance_id),
                    "state",
                    "InsertPluginOp carries no state blob — preserved via missing-plugin store until wire support lands (NEEDS)",
                );
            }
            if !p.enabled {
                lid(
                    &mut loss,
                    format!("plugin {:?}", p.instance_id),
                    "enabled",
                    "no wire op for plugin enable state at insert",
                );
            }
            for prm in &p.parameters {
                match &prm.value {
                    PluginParamValue::Real(v) => ops.push(PlannedOp {
                        op: "SetPluginParamOp".into(),
                        payload: serde_json::json!({
                            "pluginInstanceId": p.instance_id,
                            "paramId": prm.param_id,
                            "value": v,
                        }),
                    }),
                    other => {
                        loss.approximate(
                            format!("plugin {:?} param {:?}", p.instance_id, prm.param_id),
                            "paramType",
                            format!(
                                "SetPluginParamOp is real-only on the wire; {other:?} approximated"
                            ),
                        );
                    }
                }
            }
        }
    }

    ImportPlan {
        ops,
        loss: loss.sorted(),
    }
}
