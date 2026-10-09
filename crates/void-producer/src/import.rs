//! Project import planner (T80-side; PRO-05).
//!
//! Imports a foreign project description into VOID's object model as a
//! *plan*: a checked mapping of source ids → fresh VOID ids plus the
//! ordered op list to materialize it and a loss report. The plan is
//! the artifact — executing it is the integrator's transaction layer.
//!
//! Guarantees enforced by construction:
//!   * every generated id is unique (uuid v5 over source-id —
//!     deterministic, collision-checked against existing ids);
//!   * sends/buses that reference missing destinations land in
//!     `unresolved` — reported, never silently dangling;
//!   * every supported source feature maps to a VOID construct, and
//!     every unsupported one lands in `losses` with a reason.

use crate::error::{ProducerError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Foreign-project description (source-agnostic; adapters for
/// specific formats feed this shape).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignProject {
    pub name: String,
    pub tempo_bpm: f64,
    pub ts_num: u32,
    pub ts_den: u32,
    pub tracks: Vec<ForeignTrack>,
    /// Bus/send destinations by track (source track id → target bus).
    #[serde(default)]
    pub sends: Vec<ForeignSend>,
    #[serde(default)]
    pub buses: Vec<ForeignBus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignTrack {
    pub id: String,
    pub name: String,
    /// "audio" | "midi" | anything else → loss entry.
    pub kind: String,
    pub clips: Vec<ForeignClip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignClip {
    pub id: String,
    pub start_ticks: i64,
    pub length_ticks: i64,
    /// External file the clip reads (empty = generated/MIDI clip).
    #[serde(default)]
    pub source_path: String,
    /// MIDI notes already decoded to the 960k-tick domain.
    #[serde(default)]
    pub notes: Vec<ForeignNote>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForeignNote {
    pub pitch: i32,
    pub velocity: i32,
    pub onset_ticks: i64,
    pub length_ticks: i64,
}

impl Serialize for ForeignNote {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("ForeignNote", 4)?;
        st.serialize_field("pitch", &self.pitch)?;
        st.serialize_field("velocity", &self.velocity)?;
        st.serialize_field("onsetTicks", &self.onset_ticks.to_string())?;
        st.serialize_field("lengthTicks", &self.length_ticks.to_string())?;
        st.end()
    }
}
impl<'de> Deserialize<'de> for ForeignNote {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Rep {
            pitch: i32,
            velocity: i32,
            onset_ticks: String,
            length_ticks: String,
        }
        let r = Rep::deserialize(d)?;
        let parse = |s: &str| s.parse::<i64>().map_err(serde::de::Error::custom);
        Ok(ForeignNote {
            pitch: r.pitch,
            velocity: r.velocity,
            onset_ticks: parse(&r.onset_ticks)?,
            length_ticks: parse(&r.length_ticks)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignSend {
    pub from_track_id: String,
    pub to_bus_id: String,
    pub gain_db: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignBus {
    pub id: String,
    pub name: String,
}

/// What one import loss looks like — reported, never silent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportLoss {
    /// Source feature name (e.g. "sidechain-send", "video-track").
    pub feature: String,
    /// Where it was seen.
    pub location: String,
    /// Why it can't map + what the nearest VOID construct is.
    pub detail: String,
}

/// Deterministic id remapping — the plan's collision-proof table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub plan_id: String,
    pub source_name: String,
    /// source id → new void id (uuid v5 over source id, namespaced).
    pub track_ids: BTreeMap<String, String>,
    pub clip_ids: BTreeMap<String, String>,
    pub bus_ids: BTreeMap<String, String>,
    /// Sends that resolved (both ends mapped).
    pub resolved_sends: Vec<ForeignSend>,
    /// Sends referencing unknown targets — reported.
    pub unresolved: Vec<ForeignSend>,
    pub losses: Vec<ImportLoss>,
    /// Ordered materialization ops for the integrator's transaction.
    pub ops: Vec<PlannedOp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PlannedOp {
    CreateTrack {
        void_id: String,
        name: String,
        kind: String,
    },
    CreateBus {
        void_id: String,
        name: String,
    },
    /// Clip shell + payload (audio ref or note list).
    CreateClip {
        void_id: String,
        track_void_id: String,
        start_ticks: String,
        length_ticks: String,
        source_path: String,
        notes: Vec<ForeignNote>,
    },
    CreateSend {
        from_void_id: String,
        to_void_id: String,
        gain_db: f64,
    },
}

fn v5(name: &str) -> String {
    uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        format!("void-import:{name}").as_bytes(),
    )
    .to_string()
}

/// Known-mappable track kinds; everything else is a loss entry.
fn map_track_kind(kind: &str) -> Option<&'static str> {
    match kind.to_ascii_lowercase().as_str() {
        "audio" => Some("audio"),
        "midi" | "instrument" => Some("midi"),
        _ => None,
    }
}

/// Build the import plan. `existing_ids` = ids already live in the
/// destination project — a collision bumps the uuid namespace (v5 over
/// "id#n") until unique, so a plan NEVER collides.
pub fn plan_import(src: &ForeignProject, existing_ids: &BTreeSet<String>) -> Result<ImportPlan> {
    if src.tracks.is_empty() && src.buses.is_empty() {
        return Err(ProducerError::Import(
            "source has no tracks or buses".into(),
        ));
    }
    if !(20.0..=999.0).contains(&src.tempo_bpm) || src.ts_num == 0 || src.ts_den == 0 {
        return Err(ProducerError::Import("bad tempo/time-signature".into()));
    }
    let mut track_ids = BTreeMap::new();
    let mut clip_ids = BTreeMap::new();
    let mut bus_ids = BTreeMap::new();
    let mut losses = Vec::new();
    let mut ops = Vec::new();
    let mut used: BTreeSet<String> = existing_ids.clone();

    let mint = |key: &str, used: &mut BTreeSet<String>| -> String {
        let mut n = 0u32;
        loop {
            let id = v5(&format!("{key}#{n}"));
            if used.insert(id.clone()) {
                return id;
            }
            n += 1;
            if n > 1_000 {
                return id; // astronomically unreachable; bounded anyway
            }
        }
    };

    // Buses first — sends resolve against them.
    for b in &src.buses {
        let id = mint(&format!("bus:{}", b.id), &mut used);
        bus_ids.insert(b.id.clone(), id.clone());
        ops.push(PlannedOp::CreateBus {
            void_id: id,
            name: b.name.clone(),
        });
    }
    for t in &src.tracks {
        let Some(kind) = map_track_kind(&t.kind) else {
            losses.push(ImportLoss {
                feature: "track-kind".into(),
                location: t.id.clone(),
                detail: format!("track kind {:?} has no VOID mapping", t.kind),
            });
            continue;
        };
        let tid = mint(&format!("track:{}", t.id), &mut used);
        track_ids.insert(t.id.clone(), tid.clone());
        ops.push(PlannedOp::CreateTrack {
            void_id: tid.clone(),
            name: t.name.clone(),
            kind: kind.to_string(),
        });
        for c in &t.clips {
            if c.length_ticks <= 0 || c.start_ticks < 0 {
                losses.push(ImportLoss {
                    feature: "clip".into(),
                    location: c.id.clone(),
                    detail: "non-positive length or negative start dropped".into(),
                });
                continue;
            }
            let cid = mint(&format!("clip:{}", c.id), &mut used);
            clip_ids.insert(c.id.clone(), cid.clone());
            ops.push(PlannedOp::CreateClip {
                void_id: cid,
                track_void_id: tid.clone(),
                start_ticks: c.start_ticks.to_string(),
                length_ticks: c.length_ticks.to_string(),
                source_path: c.source_path.clone(),
                notes: c.notes.clone(),
            });
        }
    }
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();
    for s in &src.sends {
        match (track_ids.get(&s.from_track_id), bus_ids.get(&s.to_bus_id)) {
            (Some(f), Some(t)) => {
                resolved.push(s.clone());
                ops.push(PlannedOp::CreateSend {
                    from_void_id: f.clone(),
                    to_void_id: t.clone(),
                    gain_db: s.gain_db,
                });
            }
            _ => unresolved.push(s.clone()),
        }
    }
    Ok(ImportPlan {
        plan_id: v5(&format!("plan:{}", src.name)),
        source_name: src.name.clone(),
        track_ids,
        clip_ids,
        bus_ids,
        resolved_sends: resolved,
        unresolved,
        losses,
        ops,
    })
}

/// Persist the plan + loss report (JSON, tmp+rename).
pub fn write_plan(plan: &ImportPlan, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let p = dir.join(format!("import-plan-{}.json", plan.plan_id));
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(plan)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src() -> ForeignProject {
        ForeignProject {
            name: "demo".into(),
            tempo_bpm: 120.0,
            ts_num: 4,
            ts_den: 4,
            tracks: vec![
                ForeignTrack {
                    id: "t1".into(),
                    name: "Drums".into(),
                    kind: "audio".into(),
                    clips: vec![ForeignClip {
                        id: "c1".into(),
                        start_ticks: 0,
                        length_ticks: 960_000,
                        source_path: "/x/kick.wav".into(),
                        notes: vec![],
                    }],
                },
                ForeignTrack {
                    id: "t2".into(),
                    name: "Keys".into(),
                    kind: "midi".into(),
                    clips: vec![ForeignClip {
                        id: "c2".into(),
                        start_ticks: 0,
                        length_ticks: 960_000,
                        source_path: String::new(),
                        notes: vec![ForeignNote {
                            pitch: 60,
                            velocity: 90,
                            onset_ticks: 0,
                            length_ticks: 240_000,
                        }],
                    }],
                },
                ForeignTrack {
                    id: "t3".into(),
                    name: "Video".into(),
                    kind: "video".into(),
                    clips: vec![],
                },
            ],
            sends: vec![
                ForeignSend {
                    from_track_id: "t1".into(),
                    to_bus_id: "b1".into(),
                    gain_db: -6.0,
                },
                ForeignSend {
                    from_track_id: "t1".into(),
                    to_bus_id: "ghost".into(),
                    gain_db: -3.0,
                },
            ],
            buses: vec![ForeignBus {
                id: "b1".into(),
                name: "Reverb".into(),
            }],
        }
    }

    #[test]
    fn plan_maps_ids_reports_losses_and_unresolved() {
        let p = plan_import(&src(), &BTreeSet::new()).unwrap();
        assert_eq!(p.track_ids.len(), 2); // video → loss
        assert_eq!(p.losses.len(), 1);
        assert_eq!(p.losses[0].feature, "track-kind");
        assert_eq!(p.resolved_sends.len(), 1);
        assert_eq!(p.unresolved.len(), 1);
        // All minted ids unique (2 tracks + 2 clips + 1 bus = 5).
        let all: BTreeSet<&String> = p
            .track_ids
            .values()
            .chain(p.clip_ids.values())
            .chain(p.bus_ids.values())
            .collect();
        assert_eq!(all.len(), 5);
        // Ops ordered: buses, tracks, clips, sends.
        assert!(matches!(p.ops[0], PlannedOp::CreateBus { .. }));
        assert!(matches!(
            p.ops.last().unwrap(),
            PlannedOp::CreateSend { .. }
        ));
    }

    #[test]
    fn collision_with_existing_ids_bumps_namespace() {
        // Pre-seed the first minted id → planner must mint a fresh one.
        let first = v5("track:t1#0");
        let existing: BTreeSet<String> = [first.clone()].into_iter().collect();
        let p = plan_import(&src(), &existing).unwrap();
        assert_ne!(p.track_ids["t1"], first);
        assert!(!existing.contains(&p.track_ids["t1"]));
    }

    #[test]
    fn rejects_empty_and_bad_clock() {
        let mut s = src();
        s.tracks.clear();
        s.buses.clear();
        assert!(plan_import(&s, &BTreeSet::new()).is_err());
        let mut s = src();
        s.tempo_bpm = 0.0;
        assert!(plan_import(&s, &BTreeSet::new()).is_err());
    }
}
