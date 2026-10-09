//! Score edit ops — every mutation is a typed `ScoreOp` applied inside
//! one transaction boundary, returning an [`UndoToken`] that rewinds it
//! exactly (T90).
//!
//! Semantics contract (mirrors CONTRACTS §5): one user gesture = one
//! transaction; `apply_transaction` is all-or-nothing — a mid-plan
//! validation failure replays the collected inverses and returns the
//! original error, leaving the score untouched. Undo is exact
//! restoration: the token carries inverse records capturing the prior
//! state of every touched element (including cascade-removed
//! attachments), not re-derived approximations.
//!
//! Cascade policy (delete): removing a host element also removes its
//! attached leaves (lyrics/tab/articulations) and every slur anchored to
//! it; tuplet/beam entities lose the member, and are themselves removed
//! when they would drop below the structural minimum (tuplet <1 member
//! left, beam <2 members left — a one-member beam is not a beam).
//! Everything removed is recorded in the token so undo restores the
//! whole cascade.

use crate::error::{NotationError, Result};
use crate::model::*;
use crate::pitch::Pitch;
use serde::{Deserialize, Serialize};

/// What a transpose covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "scope")]
pub enum TransposeScope {
    /// Every pitched element in the score.
    All,
    /// One part.
    Part { part_id: String },
    /// Explicit element ids (notes/chords).
    Elements { ids: Vec<ElementId> },
}

/// A typed score mutation. All fields are validated before apply; an
/// `id: None` element insert derives a deterministic id from its
/// canonical descriptor (`derive_id`) — callers that need known ids
/// (interchange, tests) always pass them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op", content = "params")]
pub enum ScoreOp {
    /// Insert one element into a part's measure. Timed kinds require
    /// `element.position`; attached kinds require their hosts to exist
    /// in the same measure span already (insert hosts first).
    InsertElement {
        part_id: String,
        measure_index: u32,
        element: Element,
    },
    /// Remove an element plus its cascade (see module docs).
    DeleteElement { element_id: ElementId },
    /// Reposition a timed element (same or another measure; voice and
    /// staff are restated explicitly).
    MoveElement {
        element_id: ElementId,
        measure_index: u32,
        offset_ticks: String, // string-int64 contract
        voice: u32,
        staff: u32,
    },
    /// Chromatic transpose by semitones; pitches respell under the
    /// part's key signature (`Pitch::spell`).
    Transpose {
        scope: TransposeScope,
        semitones: i32,
    },
    /// Change a timed element's notated duration.
    SetDuration {
        element_id: ElementId,
        duration_ticks: String,
    },
    /// Exact pitch restore/assign (also the transpose inverse).
    SetPitch { element_id: ElementId, pitch: Pitch },
    /// Assign a lyric line to a note/chord — upserts on
    /// (host, number): an existing lyric keeps its element id (stable
    /// id through lyric rewrites), only fields update.
    AssignLyric {
        host_id: ElementId,
        lyric_id: Option<ElementId>,
        number: String,
        syllabic: Syllabic,
        text: String,
    },
    /// Bind a string/fret pair to a note or chord member — upserts on
    /// (host, member).
    BindTab {
        host_id: ElementId,
        tab_id: Option<ElementId>,
        member: u32,
        string: u32,
        fret: u32,
    },
    /// Attach an articulation to a note/chord — upserts on
    /// (host, kind).
    AttachArticulation {
        host_id: ElementId,
        articulation_id: Option<ElementId>,
        kind: ArticulationKind,
        placement: Option<Placement>,
    },
    /// Create a slur between two timed hosts.
    CreateSlur {
        slur_id: Option<ElementId>,
        start_element_id: ElementId,
        end_element_id: ElementId,
        number: u32,
    },
    /// Group timed members into a tuplet (`actual` in time of `normal`).
    GroupTuplet {
        tuplet_id: Option<ElementId>,
        member_ids: Vec<ElementId>,
        actual: u32,
        normal: u32,
        normal_type: Option<NoteType>,
    },
    /// Beam a set of timed members at `number` (members must not
    /// already sit in a beam at that number).
    Beam {
        beam_id: Option<ElementId>,
        number: u32,
        members: Vec<BeamMembership>,
    },
    /// Replace the whole tempo map (anchors hold absolute time, so
    /// their tick positions rederive — the surviving-tempo-change op).
    SetTempoMap { points: Vec<TempoPoint> },
    /// Upsert an absolute-time anchor (id-keyed).
    UpsertAnchor { anchor: Anchor },
    /// Remove an anchor.
    RemoveAnchor { anchor_id: String },
}

/// Inverse record — internal, captured precisely at apply time so undo
/// restores rather than recomputes. Serde-stable for persisted history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op", content = "params")]
enum Inverse {
    /// Remove these elements (undoing an insert).
    Remove {
        ids: Vec<ElementId>,
    },
    /// Reinsert exact elements at their captured locations (undoing a
    /// delete — restores cascade state across measures).
    Reinsert {
        items: Vec<ReinsertItem>,
    },
    /// Restore a timed element's prior position.
    SetPosition {
        element_id: ElementId,
        position: Position,
    },
    /// Restore exact prior pitch list (note → len 1, chord → N).
    SetPitches {
        element_id: ElementId,
        pitches: Vec<Pitch>,
    },
    SetDuration {
        element_id: ElementId,
        duration_ticks: i64,
    },
    /// Restore a full element (fields + position) — upsert-undo.
    Restore {
        part_id: String,
        measure_index: u32,
        element: Element,
    },
    SetTempoMap {
        points: Vec<TempoPoint>,
    },
    RemoveAnchor {
        anchor_id: String,
    },
    InsertAnchor {
        anchor: Anchor,
    },
}

/// One element's exact prior location — cascaded members can live in
/// different measures of the same part, so locations are captured per
/// element, not per delete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReinsertItem {
    part_id: String,
    measure_index: u32,
    element: Element,
}

/// A transaction's rewind record — ordered inverses; `undo` applies
/// them in reverse. Equality with `apply`ed ops is asserted by tests,
/// not assumed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoToken {
    inverses: Vec<Inverse>,
}

impl UndoToken {
    pub fn is_empty(&self) -> bool {
        self.inverses.is_empty()
    }
    /// Number of captured inverses (observability for history UIs).
    pub fn len(&self) -> usize {
        self.inverses.len()
    }
}

fn rejected(msg: impl Into<String>) -> NotationError {
    NotationError::OpRejected(msg.into())
}

impl Score {
    /// Apply one op transactionally.
    pub fn apply(&mut self, op: &ScoreOp) -> Result<UndoToken> {
        self.apply_transaction(std::slice::from_ref(op))
    }

    /// Apply `ops` as a single all-or-nothing transaction. On any
    /// rejection the already-applied inverses replay in reverse, the
    /// original error is returned, and the score equals its prior state.
    pub fn apply_transaction(&mut self, ops: &[ScoreOp]) -> Result<UndoToken> {
        let mut inverses: Vec<Inverse> = Vec::new();
        for op in ops {
            match self.apply_one(op) {
                Ok(inv) => inverses.extend(inv),
                Err(e) => {
                    for inv in inverses.iter().rev() {
                        self.apply_inverse(inv)
                            .expect("rollback inverse must not fail");
                    }
                    return Err(e);
                }
            }
        }
        Ok(UndoToken { inverses })
    }

    /// Rewind a transaction exactly.
    pub fn undo(&mut self, token: &UndoToken) -> Result<()> {
        for inv in token.inverses.iter().rev() {
            self.apply_inverse(inv)?;
        }
        Ok(())
    }

    /// Extract `part_id` into a standalone single-part score (T90 part
    /// extraction): element ids, measure structure, anchors and the
    /// tempo map carry over unchanged — an extracted part references
    /// the same stable ids, so re-import/diff stays traceable. This is
    /// a derive, not a mutation — no undo needed.
    pub fn extract_part(&self, part_id: &str) -> Result<Score> {
        let part = self
            .parts
            .iter()
            .find(|p| p.id == part_id)
            .ok_or_else(|| rejected(format!("no part {part_id:?}")))?
            .clone();
        let mut out = Score::new(part.name.clone());
        out.movement = self.movement.clone();
        out.composer = self.composer.clone();
        out.tempo_map = self.tempo_map.clone();
        out.timecode = self.timecode;
        out.anchors = self.anchors.clone();
        out.parts = vec![part];
        Ok(out)
    }

    // ------------------------------------------------------------------

    fn apply_one(&mut self, op: &ScoreOp) -> Result<Vec<Inverse>> {
        match op {
            ScoreOp::InsertElement {
                part_id,
                measure_index,
                element,
            } => self.op_insert(part_id, *measure_index, element),
            ScoreOp::DeleteElement { element_id } => self.op_delete(element_id),
            ScoreOp::MoveElement {
                element_id,
                measure_index,
                offset_ticks,
                voice,
                staff,
            } => self.op_move(element_id, *measure_index, offset_ticks, *voice, *staff),
            ScoreOp::Transpose { scope, semitones } => self.op_transpose(scope, *semitones),
            ScoreOp::SetDuration {
                element_id,
                duration_ticks,
            } => self.op_set_duration(element_id, duration_ticks),
            ScoreOp::SetPitch { element_id, pitch } => self.op_set_pitch(element_id, *pitch),
            ScoreOp::AssignLyric {
                host_id,
                lyric_id,
                number,
                syllabic,
                text,
            } => self.op_assign_lyric(host_id, lyric_id, number, *syllabic, text),
            ScoreOp::BindTab {
                host_id,
                tab_id,
                member,
                string,
                fret,
            } => self.op_bind_tab(host_id, tab_id, *member, *string, *fret),
            ScoreOp::AttachArticulation {
                host_id,
                articulation_id,
                kind,
                placement,
            } => self.op_attach_articulation(host_id, articulation_id, kind, *placement),
            ScoreOp::CreateSlur {
                slur_id,
                start_element_id,
                end_element_id,
                number,
            } => self.op_create_slur(slur_id, start_element_id, end_element_id, *number),
            ScoreOp::GroupTuplet {
                tuplet_id,
                member_ids,
                actual,
                normal,
                normal_type,
            } => self.op_group_tuplet(tuplet_id, member_ids, *actual, *normal, *normal_type),
            ScoreOp::Beam {
                beam_id,
                number,
                members,
            } => self.op_beam(beam_id, *number, members),
            ScoreOp::SetTempoMap { points } => {
                let mut pts = points.clone();
                let old = self.tempo_map.clone();
                pts.sort_by_key(|p| p.at_ticks);
                for (i, w) in pts.windows(2).enumerate() {
                    if w[0].at_ticks == w[1].at_ticks {
                        return Err(rejected(format!("tempoMap duplicate at_ticks #{i}")));
                    }
                }
                for (i, p) in pts.iter().enumerate() {
                    if !p.bpm.is_finite() || p.bpm <= 0.0 {
                        return Err(rejected(format!("tempoMap[{i}] bpm not finite-positive")));
                    }
                }
                self.tempo_map = pts;
                Ok(vec![Inverse::SetTempoMap { points: old }])
            }
            ScoreOp::UpsertAnchor { anchor } => {
                if self.anchors.iter().any(|a| a.id == anchor.id) {
                    let idx = self.anchors.iter().position(|a| a.id == anchor.id).unwrap();
                    let old = self.anchors[idx].clone();
                    self.anchors[idx] = anchor.clone();
                    Ok(vec![Inverse::InsertAnchor { anchor: old }])
                } else {
                    if self.anchors.iter().any(|a| a.id == anchor.id) {
                        unreachable!()
                    }
                    self.anchors.push(anchor.clone());
                    Ok(vec![Inverse::RemoveAnchor {
                        anchor_id: anchor.id.clone(),
                    }])
                }
            }
            ScoreOp::RemoveAnchor { anchor_id } => {
                match self.anchors.iter().position(|a| &a.id == anchor_id) {
                    Some(i) => {
                        let old = self.anchors.remove(i);
                        Ok(vec![Inverse::InsertAnchor { anchor: old }])
                    }
                    None => Err(rejected(format!("no anchor {anchor_id:?}"))),
                }
            }
        }
    }

    fn apply_inverse(&mut self, inv: &Inverse) -> Result<()> {
        match inv {
            Inverse::Remove { ids } => {
                for id in ids {
                    self.remove_element(id)
                        .ok_or_else(|| rejected(format!("undo: element {id} absent")))?;
                }
                Ok(())
            }
            Inverse::Reinsert { items } => {
                for item in items {
                    let pi = self
                        .find_part(&item.part_id)
                        .ok_or_else(|| rejected(format!("undo: no part {:?}", item.part_id)))?;
                    let part = &mut self.parts[pi];
                    let m = part
                        .measures
                        .get_mut(item.measure_index as usize)
                        .ok_or_else(|| rejected("undo: measure index out of range"))?;
                    m.elements.push(item.element.clone());
                }
                Ok(())
            }
            Inverse::SetPosition {
                element_id,
                position,
            } => {
                let e = self
                    .element_mut(element_id)
                    .ok_or_else(|| rejected(format!("undo: element {element_id} absent")))?;
                e.position = Some(*position);
                Ok(())
            }
            Inverse::SetPitches {
                element_id,
                pitches,
            } => {
                let e = self
                    .element_mut(element_id)
                    .ok_or_else(|| rejected(format!("undo: element {element_id} absent")))?;
                match &mut e.kind {
                    ElementKind::Note(n) => {
                        if pitches.len() != 1 {
                            return Err(rejected("undo: note pitch arity"));
                        }
                        n.pitch = pitches[0];
                    }
                    ElementKind::Chord(c) => c.pitches = pitches.clone(),
                    _ => return Err(rejected("undo: element is not pitched")),
                }
                Ok(())
            }
            Inverse::SetDuration {
                element_id,
                duration_ticks,
            } => {
                let e = self
                    .element_mut(element_id)
                    .ok_or_else(|| rejected(format!("undo: element {element_id} absent")))?;
                match e.position.as_mut() {
                    Some(p) => {
                        p.duration_ticks = *duration_ticks;
                        Ok(())
                    }
                    None => Err(rejected("undo: element is not timed")),
                }
            }
            Inverse::Restore {
                part_id,
                measure_index,
                element,
            } => {
                // If the element still exists (upsert), overwrite fields;
                // else reinsert.
                let pi = self
                    .find_part(part_id)
                    .ok_or_else(|| rejected(format!("undo: no part {part_id:?}")))?;
                let part = &mut self.parts[pi];
                if let Some((mi, slot)) = part.find_element_mut(&element.id) {
                    let _ = mi;
                    let _ = measure_index;
                    *slot = element.clone();
                } else {
                    let m = part
                        .measures
                        .get_mut(*measure_index as usize)
                        .ok_or_else(|| rejected("undo: measure index out of range"))?;
                    m.elements.push(element.clone());
                }
                Ok(())
            }
            Inverse::SetTempoMap { points } => {
                self.tempo_map = points.clone();
                Ok(())
            }
            Inverse::RemoveAnchor { anchor_id } => {
                self.anchors.retain(|a| &a.id != anchor_id);
                Ok(())
            }
            Inverse::InsertAnchor { anchor } => {
                self.anchors.retain(|a| a.id != anchor.id);
                self.anchors.push(anchor.clone());
                Ok(())
            }
        }
    }

    fn element_mut(&mut self, id: &ElementId) -> Option<&mut Element> {
        for p in &mut self.parts {
            if let Some((_, e)) = p.find_element_mut(id) {
                return Some(e);
            }
        }
        None
    }

    /// Physical removal of one element (no cascade — callers decide).
    fn remove_element(&mut self, id: &ElementId) -> Option<(String, usize, Element)> {
        for p in &mut self.parts {
            for (mi, m) in p.measures.iter_mut().enumerate() {
                if let Some(ei) = m.elements.iter().position(|e| &e.id == id) {
                    let part_id = p.id.clone();
                    let e = m.elements.remove(ei);
                    return Some((part_id, mi, e));
                }
            }
        }
        None
    }

    fn is_timed(&self, id: &ElementId) -> bool {
        self.element(id).map(|e| e.timed()).unwrap_or(false)
    }

    fn kind_is_hostable(&self, id: &ElementId) -> bool {
        matches!(
            self.element(id).map(|e| &e.kind),
            Some(ElementKind::Note(_)) | Some(ElementKind::Chord(_))
        )
    }

    // -- op implementations ------------------------------------------

    fn op_insert(
        &mut self,
        part_id: &str,
        measure_index: u32,
        element: &Element,
    ) -> Result<Vec<Inverse>> {
        if self.element(&element.id).is_some() {
            return Err(rejected(format!("duplicate element id {}", element.id)));
        }
        // Validate BEFORE borrowing: timed kinds need position; attached
        // kinds need existing hosts (anywhere — a slur's far endpoint can
        // sit in a later measure).
        let timed_kind = matches!(
            element.kind,
            ElementKind::Note(_)
                | ElementKind::Rest(_)
                | ElementKind::Chord(_)
                | ElementKind::Direction(_)
        );
        match (&element.kind, &element.position) {
            (k, Some(pos)) if timed_kind => {
                let dur_ok = matches!(k, ElementKind::Direction(_)) || pos.duration_ticks > 0;
                if pos.offset_ticks < 0 || !dur_ok {
                    return Err(rejected(format!("{} has invalid position", element.id)));
                }
                if pos.voice == 0 || pos.staff == 0 {
                    return Err(rejected("voice/staff must be 1-based"));
                }
            }
            (k, None) if !timed_kind => {
                let _ = k;
                for h in element.hosts() {
                    if self.parts.iter().all(|pp| pp.find_element(h).is_none()) {
                        return Err(rejected(format!("{} host {h} does not exist", element.id)));
                    }
                }
            }
            _ => {
                return Err(rejected(format!(
                    "element {} position/kind mismatch",
                    element.id
                )))
            }
        }
        let pi = self
            .find_part(part_id)
            .ok_or_else(|| rejected(format!("no part {part_id:?}")))?;
        let part = &mut self.parts[pi];
        let m = part
            .measures
            .get_mut(measure_index as usize)
            .ok_or_else(|| rejected(format!("no measure {measure_index} in {part_id:?}")))?;
        m.elements.push(element.clone());
        Ok(vec![Inverse::Remove {
            ids: vec![element.id.clone()],
        }])
    }

    /// Cascade-aware delete.
    fn op_delete(&mut self, id: &ElementId) -> Result<Vec<Inverse>> {
        let (part_id, mi, removed) = self
            .remove_element(id)
            .ok_or_else(|| rejected(format!("no element {id}")))?;
        let mut items = vec![ReinsertItem {
            part_id: part_id.clone(),
            measure_index: mi as u32,
            element: removed,
        }];
        // Leaves attached to the removed host go with it.
        let leaf_ids: Vec<ElementId> = self
            .parts
            .iter()
            .flat_map(|p| &p.measures)
            .flat_map(|m| &m.elements)
            .filter(|e| match &e.kind {
                ElementKind::Articulation(a) => &a.host == id,
                ElementKind::Lyric(l) => &l.host == id,
                ElementKind::Tab(t) => &t.host == id,
                ElementKind::Slur(s) => &s.start == id || &s.end == id,
                _ => false,
            })
            .map(|e| e.id.clone())
            .collect();
        for leaf in leaf_ids {
            if let Some((pid, lmi, e)) = self.remove_element(&leaf) {
                items.push(ReinsertItem {
                    part_id: pid,
                    measure_index: lmi as u32,
                    element: e,
                });
            }
        }
        // Container entities: tuplet/beam members referencing the deleted
        // element shrink; containers that drop below the minimum are
        // removed entirely (and captured for undo). Container member
        // shrink is itself undone by reinserting? No — the surviving
        // container keeps fewer members; restoring it needs its prior
        // member list, so capture full prior elements first.
        let mut prior_containers: Vec<ReinsertItem> = Vec::new();
        let mut to_remove: Vec<ElementId> = Vec::new();
        for (pi, p) in self.parts.iter().enumerate() {
            for (mi2, m) in p.measures.iter().enumerate() {
                for e in &m.elements {
                    let touched = match &e.kind {
                        ElementKind::Tuplet(t) => t.members.iter().any(|x| x == id),
                        ElementKind::Beam(b) => b.members.iter().any(|x| &x.element == id),
                        _ => false,
                    };
                    if touched {
                        prior_containers.push(ReinsertItem {
                            part_id: self.parts[pi].id.clone(),
                            measure_index: mi2 as u32,
                            element: e.clone(),
                        });
                        match &e.kind {
                            ElementKind::Tuplet(t)
                                if t.members.iter().filter(|x| *x != id).count() == 0 =>
                            {
                                to_remove.push(e.id.clone())
                            }
                            ElementKind::Beam(b)
                                if b.members.iter().filter(|x| &x.element != id).count() < 2 =>
                            {
                                to_remove.push(e.id.clone())
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        // Shrinking containers: remove then reinsert-truncated is wrong —
        // undo must restore prior membership, so we capture prior state
        // and REPLACE on undo (Restore), not Reinsert. Model shrink as:
        // apply = mutate member list; undo = Restore prior element.
        let mut inverses: Vec<Inverse> = Vec::new();
        for item in prior_containers {
            // Remove the container from the delete-cascade's Reinsert set
            // (it may also be fully removed below — handled there).
            inverses.push(Inverse::Restore {
                part_id: item.part_id.clone(),
                measure_index: item.measure_index,
                element: item.element.clone(),
            });
        }
        for p in self.parts.iter_mut() {
            for m in p.measures.iter_mut() {
                for e in m.elements.iter_mut() {
                    match &mut e.kind {
                        ElementKind::Tuplet(t) => t.members.retain(|x| x != id),
                        ElementKind::Beam(b) => b.members.retain(|x| &x.element != id),
                        _ => {}
                    }
                }
            }
        }
        for cid in to_remove {
            if let Some((pid, cmi, e)) = self.remove_element(&cid) {
                items.push(ReinsertItem {
                    part_id: pid,
                    measure_index: cmi as u32,
                    element: e,
                });
            }
        }
        // Order matters on undo: containers Restore first (while their
        // members exist), then deleted elements Reinsert.
        inverses.push(Inverse::Reinsert { items });
        Ok(inverses)
    }

    fn op_move(
        &mut self,
        id: &ElementId,
        measure_index: u32,
        offset_ticks: &str,
        voice: u32,
        staff: u32,
    ) -> Result<Vec<Inverse>> {
        let offset: i64 = offset_ticks
            .trim()
            .parse()
            .map_err(|_| rejected("offsetTicks not a decimal i64"))?;
        if offset < 0 {
            return Err(rejected("offsetTicks negative"));
        }
        let old = self
            .element(id)
            .ok_or_else(|| rejected(format!("no element {id}")))?
            .position
            .ok_or_else(|| rejected(format!("element {id} is not timed")))?;
        // Move across measures: element physically relocates so its
        // measure membership follows its position.
        let (pi, mi) = self.locate(id).unwrap();
        if mi != measure_index as usize {
            let (part_id, old_mi, element) = self.remove_element(id).unwrap();
            let part = &mut self.parts[pi];
            if measure_index as usize >= part.measures.len() {
                // restore before rejecting
                part.measures[old_mi].elements.push(element);
                return Err(rejected(format!("no measure {measure_index}")));
            }
            let mut e = element.clone();
            e.position = Some(Position {
                offset_ticks: offset,
                duration_ticks: old.duration_ticks,
                voice,
                staff,
            });
            part.measures[measure_index as usize].elements.push(e);
            // Undo applies inverses in reverse: Remove (element leaves
            // the new measure) then Reinsert (restored at prior
            // location + position).
            Ok(vec![
                Inverse::Reinsert {
                    items: vec![ReinsertItem {
                        part_id,
                        measure_index: old_mi as u32,
                        element: Element {
                            position: Some(old),
                            ..element
                        },
                    }],
                },
                Inverse::Remove {
                    ids: vec![id.clone()],
                },
            ])
        } else {
            let e = self.element_mut(id).unwrap();
            e.position = Some(Position {
                offset_ticks: offset,
                duration_ticks: old.duration_ticks,
                voice,
                staff,
            });
            Ok(vec![Inverse::SetPosition {
                element_id: id.clone(),
                position: old,
            }])
        }
    }

    fn op_transpose(&mut self, scope: &TransposeScope, semitones: i32) -> Result<Vec<Inverse>> {
        let mut inverses = Vec::new();
        let targets: Vec<(usize, ElementId)> = match scope {
            TransposeScope::All => self
                .parts
                .iter()
                .enumerate()
                .flat_map(|(pi, p)| {
                    p.measures
                        .iter()
                        .flat_map(|m| m.elements.iter())
                        .filter(|e| matches!(e.kind, ElementKind::Note(_) | ElementKind::Chord(_)))
                        .map(|e| (pi, e.id.clone()))
                        .collect::<Vec<_>>()
                })
                .collect(),
            TransposeScope::Part { part_id } => {
                let pi = self
                    .find_part(part_id)
                    .ok_or_else(|| rejected(format!("no part {part_id:?}")))?;
                self.parts[pi]
                    .measures
                    .iter()
                    .flat_map(|m| m.elements.iter())
                    .filter(|e| matches!(e.kind, ElementKind::Note(_) | ElementKind::Chord(_)))
                    .map(|e| (pi, e.id.clone()))
                    .collect()
            }
            TransposeScope::Elements { ids } => ids
                .iter()
                .map(|id| {
                    let (pi, _) = self
                        .locate(id)
                        .ok_or_else(|| rejected(format!("no element {id}")))?;
                    Ok((pi, id.clone()))
                })
                .collect::<Result<Vec<_>>>()?,
        };
        for (pi, id) in targets {
            let fifths = {
                let (mi, _) = self.parts[pi].find_element(&id).unwrap();
                self.parts[pi].fifths_at(mi)
            };
            let e = self.element_mut(&id).unwrap();
            match &mut e.kind {
                ElementKind::Note(n) => {
                    let old = n.pitch;
                    n.pitch = n.pitch.transposed(semitones, fifths);
                    inverses.push(Inverse::SetPitches {
                        element_id: id.clone(),
                        pitches: vec![old],
                    });
                }
                ElementKind::Chord(c) => {
                    let old = c.pitches.clone();
                    c.pitches = c
                        .pitches
                        .iter()
                        .map(|p| p.transposed(semitones, fifths))
                        .collect();
                    inverses.push(Inverse::SetPitches {
                        element_id: id.clone(),
                        pitches: old,
                    });
                }
                _ => {
                    return Err(rejected(format!("element {id} is not pitched")));
                }
            }
        }
        Ok(inverses)
    }

    fn op_set_duration(&mut self, id: &ElementId, duration_ticks: &str) -> Result<Vec<Inverse>> {
        let d: i64 = duration_ticks
            .trim()
            .parse()
            .map_err(|_| rejected("durationTicks not a decimal i64"))?;
        if d <= 0 {
            return Err(rejected("durationTicks must be positive"));
        }
        let e = self
            .element_mut(id)
            .ok_or_else(|| rejected(format!("no element {id}")))?;
        let pos = e
            .position
            .as_mut()
            .ok_or_else(|| rejected(format!("element {id} is not timed")))?;
        let old = pos.duration_ticks;
        pos.duration_ticks = d;
        Ok(vec![Inverse::SetDuration {
            element_id: id.clone(),
            duration_ticks: old,
        }])
    }

    fn op_set_pitch(&mut self, id: &ElementId, pitch: Pitch) -> Result<Vec<Inverse>> {
        let e = self
            .element_mut(id)
            .ok_or_else(|| rejected(format!("no element {id}")))?;
        match &mut e.kind {
            ElementKind::Note(n) => {
                let old = n.pitch;
                n.pitch = pitch;
                Ok(vec![Inverse::SetPitches {
                    element_id: id.clone(),
                    pitches: vec![old],
                }])
            }
            _ => Err(rejected(format!("element {id} is not a note"))),
        }
    }

    fn op_assign_lyric(
        &mut self,
        host_id: &ElementId,
        lyric_id: &Option<ElementId>,
        number: &str,
        syllabic: Syllabic,
        text: &str,
    ) -> Result<Vec<Inverse>> {
        if !self.kind_is_hostable(host_id) {
            return Err(rejected(format!(
                "lyric host {host_id} is not a note/chord"
            )));
        }
        let (pi, mi) = self.locate(host_id).unwrap();
        let pid = self.parts[pi].id.clone();
        let part = &mut self.parts[pi];
        // Upsert: same host+number rewrites fields in place — the lyric
        // keeps its element id.
        let existing = part.measures.iter_mut().find_map(|m| {
            m.elements.iter_mut().find(|e| match &e.kind {
                ElementKind::Lyric(l) => &l.host == host_id && l.number == number,
                _ => false,
            })
        });
        if let Some(e) = existing {
            let old = e.clone();
            if let ElementKind::Lyric(l) = &mut e.kind {
                l.syllabic = syllabic;
                l.text = text.to_string();
            }
            return Ok(vec![Inverse::Restore {
                part_id: pid,
                measure_index: mi as u32,
                element: old,
            }]);
        }
        let id = lyric_id
            .clone()
            .unwrap_or_else(|| derive_id(&format!("lyric|{host_id}|{number}")));
        let el = Element {
            id: id.clone(),
            position: None,
            kind: ElementKind::Lyric(LyricData {
                host: host_id.clone(),
                number: number.to_string(),
                syllabic,
                text: text.to_string(),
            }),
        };
        part.measures[mi].elements.push(el);
        Ok(vec![Inverse::Remove { ids: vec![id] }])
    }

    fn op_bind_tab(
        &mut self,
        host_id: &ElementId,
        tab_id: &Option<ElementId>,
        member: u32,
        string: u32,
        fret: u32,
    ) -> Result<Vec<Inverse>> {
        let host = self
            .element(host_id)
            .ok_or_else(|| rejected(format!("no element {host_id}")))?;
        let member_count = match &host.kind {
            ElementKind::Note(_) => 1,
            ElementKind::Chord(c) => c.pitches.len() as u32,
            _ => return Err(rejected(format!("tab host {host_id} is not pitched"))),
        };
        if member >= member_count {
            return Err(rejected(format!(
                "tab member {member} >= host member count {member_count}"
            )));
        }
        if string == 0 {
            return Err(rejected("tab string must be 1-based"));
        }
        let (pi, mi) = self.locate(host_id).unwrap();
        let part = &mut self.parts[pi];
        let existing = part.measures.iter_mut().find_map(|m| {
            m.elements.iter_mut().find(|e| match &e.kind {
                ElementKind::Tab(t) => &t.host == host_id && t.member == member,
                _ => false,
            })
        });
        if let Some(e) = existing {
            let old = e.clone();
            if let ElementKind::Tab(t) = &mut e.kind {
                t.string = string;
                t.fret = fret;
            }
            return Ok(vec![Inverse::Restore {
                part_id: self.parts[pi].id.clone(),
                measure_index: mi as u32,
                element: old,
            }]);
        }
        let id = tab_id
            .clone()
            .unwrap_or_else(|| derive_id(&format!("tab|{host_id}|{member}")));
        let el = Element {
            id: id.clone(),
            position: None,
            kind: ElementKind::Tab(TabData {
                host: host_id.clone(),
                member,
                string,
                fret,
            }),
        };
        part.measures[mi].elements.push(el);
        Ok(vec![Inverse::Remove { ids: vec![id] }])
    }

    fn op_attach_articulation(
        &mut self,
        host_id: &ElementId,
        art_id: &Option<ElementId>,
        kind: &ArticulationKind,
        placement: Option<Placement>,
    ) -> Result<Vec<Inverse>> {
        if !self.kind_is_hostable(host_id) {
            return Err(rejected(format!(
                "articulation host {host_id} is not a note/chord"
            )));
        }
        let (pi, mi) = self.locate(host_id).unwrap();
        let part = &mut self.parts[pi];
        let kind_key = serde_json::to_string(kind).unwrap_or_default();
        let existing = part.measures.iter_mut().find_map(|m| {
            m.elements.iter_mut().find(|e| match &e.kind {
                ElementKind::Articulation(a) => {
                    &a.host == host_id
                        && serde_json::to_string(&a.kind).unwrap_or_default() == kind_key
                }
                _ => false,
            })
        });
        if let Some(e) = existing {
            let old = e.clone();
            if let ElementKind::Articulation(a) = &mut e.kind {
                a.placement = placement;
            }
            return Ok(vec![Inverse::Restore {
                part_id: self.parts[pi].id.clone(),
                measure_index: mi as u32,
                element: old,
            }]);
        }
        let id = art_id
            .clone()
            .unwrap_or_else(|| derive_id(&format!("art|{host_id}|{kind_key}")));
        let el = Element {
            id: id.clone(),
            position: None,
            kind: ElementKind::Articulation(ArticulationData {
                host: host_id.clone(),
                kind: kind.clone(),
                placement,
            }),
        };
        part.measures[mi].elements.push(el);
        Ok(vec![Inverse::Remove { ids: vec![id] }])
    }

    fn op_create_slur(
        &mut self,
        slur_id: &Option<ElementId>,
        start: &ElementId,
        end: &ElementId,
        number: u32,
    ) -> Result<Vec<Inverse>> {
        for h in [start, end] {
            if !self.is_timed(h) {
                return Err(rejected(format!(
                    "slur endpoint {h} is not a timed element"
                )));
            }
        }
        let (pi, mi) = self.locate(start).unwrap();
        let id = slur_id
            .clone()
            .unwrap_or_else(|| derive_id(&format!("slur|{start}|{end}|{number}")));
        let el = Element {
            id: id.clone(),
            position: None,
            kind: ElementKind::Slur(SlurData {
                start: start.clone(),
                end: end.clone(),
                number,
            }),
        };
        self.parts[pi].measures[mi].elements.push(el);
        Ok(vec![Inverse::Remove { ids: vec![id] }])
    }

    fn op_group_tuplet(
        &mut self,
        tuplet_id: &Option<ElementId>,
        member_ids: &[ElementId],
        actual: u32,
        normal: u32,
        normal_type: Option<NoteType>,
    ) -> Result<Vec<Inverse>> {
        if actual == 0 || normal == 0 {
            return Err(rejected("tuplet actual/normal must be > 0"));
        }
        if member_ids.len() < 2 {
            return Err(rejected("tuplet needs >= 2 members"));
        }
        for m in member_ids {
            let e = self
                .element(m)
                .ok_or_else(|| rejected(format!("no tuplet member {m}")))?;
            if !e.timed() && !matches!(e.kind, ElementKind::Tuplet(_)) {
                return Err(rejected(format!("tuplet member {m} is not timed/tuplet")));
            }
        }
        let (pi, mi) = self.locate(&member_ids[0]).unwrap();
        let id = tuplet_id
            .clone()
            .unwrap_or_else(|| derive_id(&format!("tuplet|{}|{actual}/{normal}", member_ids[0])));
        let el = Element {
            id: id.clone(),
            position: None,
            kind: ElementKind::Tuplet(TupletData {
                actual,
                normal,
                normal_type,
                members: member_ids.to_vec(),
            }),
        };
        self.parts[pi].measures[mi].elements.push(el);
        Ok(vec![Inverse::Remove { ids: vec![id] }])
    }

    fn op_beam(
        &mut self,
        beam_id: &Option<ElementId>,
        number: u32,
        members: &[BeamMembership],
    ) -> Result<Vec<Inverse>> {
        if number == 0 || number > 8 {
            return Err(rejected("beam number out of 1..8"));
        }
        if members.len() < 2 {
            return Err(rejected("beam needs >= 2 members"));
        }
        for m in members {
            if !self.is_timed(&m.element) {
                return Err(rejected(format!("beam member {} is not timed", m.element)));
            }
        }
        // A member already beamed at this number in this part is a
        // conflict — explicit re-beam is a different gesture (delete the
        // beam first), never silent re-grouping.
        for p in &self.parts {
            for m in &p.measures {
                for e in &m.elements {
                    if let ElementKind::Beam(b) = &e.kind {
                        if b.number == number
                            && b.members
                                .iter()
                                .any(|x| members.iter().any(|y| y.element == x.element))
                        {
                            return Err(rejected(format!(
                                "member already in beam {} at number {number}",
                                e.id
                            )));
                        }
                    }
                }
            }
        }
        let (pi, mi) = self.locate(&members[0].element).unwrap();
        let id = beam_id
            .clone()
            .unwrap_or_else(|| derive_id(&format!("beam|{}|{number}", members[0].element)));
        let el = Element {
            id: id.clone(),
            position: None,
            kind: ElementKind::Beam(BeamData {
                number,
                members: members.to_vec(),
            }),
        };
        self.parts[pi].measures[mi].elements.push(el);
        Ok(vec![Inverse::Remove { ids: vec![id] }])
    }
}
