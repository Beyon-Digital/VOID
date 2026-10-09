//! MusicXML 4.0 partwise interchange (T91-side).
//!
//! Scope — what this implementation honestly is:
//! - Parses/serializes `<score-partwise>` documents: part-list with
//!   part-groups, measures, attributes (divisions/key/time/clefs/staves),
//!   notes (pitch/rest/chord/tie/dots/type/accidental/voice/staff),
//!   backup/forward cursor movement, tuplets (nested), beams (per
//!   number), slurs (numbered start/stop), articulations, technical
//!   string/fret (tab), lyrics, and the direction subset listed on
//!   [`DirectionKind`].
//! - `<sound tempo>` becomes a score [`TempoPoint`] — absolute ticks,
//!   so tempo information round-trips through MusicXML.
//! - Stable ids: `<note id>` attributes are preserved; everything else
//!   derives deterministic ids (`derive_id`) — a byte-identical file
//!   re-imports identical ids, and ids we export come back unchanged.
//!
//! What it is not (all recorded as loss entries, never silent):
//! `score-timewise` is rejected up front; harmony/figured-bass/barline
//! styles/print layout/instrument assignment/notation ornaments etc.
//! are dropped with entries. `divisions` values that don't map exactly
//! onto the 960,000-tick quarter are *approximated* with entries, never
//! fudged.

use crate::anchors::Rat;
use crate::error::{NotationError, Result};
use crate::model::*;
use crate::pitch::{Accidental, Pitch, Step};
use quick_xml::escape::escape;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use void_exchange::{ExchangeDirection, LossReport};

// ---------------------------------------------------------------------------
// Tiny DOM (same pattern as void-exchange::dawproject)
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// Direct-text content (entity-decoded, trimmed).
    fn value(&self) -> &str {
        self.text.trim()
    }
    /// Text of the named child.
    fn val(&self, name: &str) -> Option<&str> {
        self.child(name).map(|c| c.value())
    }
}

fn parse_dom(xml: &str) -> Result<Node> {
    let mut r = Reader::from_str(xml);
    r.config_mut().expand_empty_elements = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let mut attrs = BTreeMap::new();
                for a in e.attributes() {
                    let a = a.map_err(|er| NotationError::Xml(er.to_string()))?;
                    let k = a.key.as_ref().to_string();
                    let v = a
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map(|c| c.into_owned())
                        .map_err(|er| NotationError::Xml(er.to_string()))?;
                    attrs.insert(k, v);
                }
                stack.push(Node {
                    name: e.name().as_ref().to_string(),
                    attrs,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            Ok(Event::Text(t)) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml10_content());
                }
            }
            Ok(Event::End(_)) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| NotationError::Malformed("unbalanced close".into()))?;
                match stack.last_mut() {
                    Some(top) => top.children.push(node),
                    None => {
                        root = Some(node);
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(NotationError::Xml(e.to_string())),
        }
    }
    root.ok_or_else(|| NotationError::Malformed("no root element".into()))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn divs_to_ticks(duration: i64, divisions: i64) -> Option<i64> {
    let num = duration as i128 * TICKS_PER_QUARTER as i128;
    if divisions == 0 {
        return None;
    }
    if num % divisions as i128 == 0 {
        Some((num / divisions as i128) as i64)
    } else {
        None
    }
}

fn divs_to_ticks_lossy(duration: i64, divisions: i64) -> (i64, bool) {
    let num = duration as i128 * TICKS_PER_QUARTER as i128;
    if divisions == 0 {
        return (0, true);
    }
    let den = divisions as i128;
    let q = num / den;
    let r = num % den;
    // Round-half-away per CONTRACTS.
    let approx = r != 0;
    let rounded = if 2 * r.abs() >= den.abs() {
        q + num.signum()
    } else {
        q
    };
    (rounded as i64, approx)
}

fn note_type_str(t: NoteType) -> &'static str {
    match t {
        NoteType::Longa => "longa",
        NoteType::Breve => "breve",
        NoteType::Whole => "whole",
        NoteType::Half => "half",
        NoteType::Quarter => "quarter",
        NoteType::Eighth => "eighth",
        NoteType::N16 => "16th",
        NoteType::N32 => "32nd",
        NoteType::N64 => "64th",
        NoteType::N128 => "128th",
        NoteType::N256 => "256th",
        NoteType::N512 => "512th",
        NoteType::N1024 => "1024th",
    }
}

fn parse_note_type(s: &str) -> Option<NoteType> {
    Some(match s.trim() {
        "longa" => NoteType::Longa,
        "breve" => NoteType::Breve,
        "whole" => NoteType::Whole,
        "half" => NoteType::Half,
        "quarter" => NoteType::Quarter,
        "eighth" => NoteType::Eighth,
        "16th" => NoteType::N16,
        "32nd" => NoteType::N32,
        "64th" => NoteType::N64,
        "128th" => NoteType::N128,
        "256th" => NoteType::N256,
        "512th" => NoteType::N512,
        "1024th" => NoteType::N1024,
        _ => return None,
    })
}

fn articulation_tag(k: &ArticulationKind) -> Option<(&'static str, Option<String>)> {
    Some(match k {
        ArticulationKind::Accent => ("accent", None),
        ArticulationKind::StrongAccent => ("strong-accent", None),
        ArticulationKind::Staccato => ("staccato", None),
        ArticulationKind::Tenuto => ("tenuto", None),
        ArticulationKind::DetachedLegato => ("detached-legato", None),
        ArticulationKind::Staccatissimo => ("staccatissimo", None),
        ArticulationKind::Spiccato => ("spiccato", None),
        ArticulationKind::Scoop => ("scoop", None),
        ArticulationKind::Plop => ("plop", None),
        ArticulationKind::Doit => ("doit", None),
        ArticulationKind::Falloff => ("falloff", None),
        ArticulationKind::BreathMark => ("breath-mark", None),
        ArticulationKind::Caesura => ("caesura", None),
        ArticulationKind::Stress => ("stress", None),
        ArticulationKind::Unstress => ("unstress", None),
        ArticulationKind::Fermata => return None, // emitted under <notations> directly
        ArticulationKind::Other { value } => ("other-articulation", Some(value.clone())),
    })
}

fn articulation_from_tag(tag: &str, text: &str) -> Option<ArticulationKind> {
    Some(match tag {
        "accent" => ArticulationKind::Accent,
        "strong-accent" => ArticulationKind::StrongAccent,
        "staccato" => ArticulationKind::Staccato,
        "tenuto" => ArticulationKind::Tenuto,
        "detached-legato" => ArticulationKind::DetachedLegato,
        "staccatissimo" => ArticulationKind::Staccatissimo,
        "spiccato" => ArticulationKind::Spiccato,
        "scoop" => ArticulationKind::Scoop,
        "plop" => ArticulationKind::Plop,
        "doit" => ArticulationKind::Doit,
        "falloff" => ArticulationKind::Falloff,
        "breath-mark" => ArticulationKind::BreathMark,
        "caesura" => ArticulationKind::Caesura,
        "stress" => ArticulationKind::Stress,
        "unstress" => ArticulationKind::Unstress,
        "other-articulation" => ArticulationKind::Other {
            value: text.trim().to_string(),
        },
        _ => return None,
    })
}

fn accidental_to_str(a: Accidental) -> &'static str {
    match a {
        Accidental::Sharp => "sharp",
        Accidental::Natural => "natural",
        Accidental::Flat => "flat",
        Accidental::DoubleSharp => "double-sharp",
        Accidental::SharpSharp => "sharp-sharp",
        Accidental::FlatFlat => "flat-flat",
        Accidental::NaturalSharp => "natural-sharp",
        Accidental::NaturalFlat => "natural-flat",
        Accidental::QuarterFlat => "quarter-flat",
        Accidental::QuarterSharp => "quarter-sharp",
        Accidental::ThreeQuartersFlat => "three-quarters-flat",
        Accidental::ThreeQuartersSharp => "three-quarters-sharp",
        Accidental::TripleSharp => "triple-sharp",
        Accidental::TripleFlat => "triple-flat",
        Accidental::SharpDown => "sharp-down",
        Accidental::SharpUp => "sharp-up",
        Accidental::NaturalDown => "natural-down",
        Accidental::NaturalUp => "natural-up",
        Accidental::FlatDown => "flat-down",
        Accidental::FlatUp => "flat-up",
        Accidental::DoubleSharpDown => "double-sharp-down",
        Accidental::DoubleSharpUp => "double-sharp-up",
        Accidental::FlatFlatDown => "flat-flat-down",
        Accidental::FlatFlatUp => "flat-flat-up",
    }
}

fn accidental_from_str(s: &str) -> Option<Accidental> {
    Some(match s.trim() {
        "sharp" => Accidental::Sharp,
        "natural" => Accidental::Natural,
        "flat" => Accidental::Flat,
        "double-sharp" => Accidental::DoubleSharp,
        "sharp-sharp" => Accidental::SharpSharp,
        "flat-flat" => Accidental::FlatFlat,
        "natural-sharp" => Accidental::NaturalSharp,
        "natural-flat" => Accidental::NaturalFlat,
        "quarter-flat" => Accidental::QuarterFlat,
        "quarter-sharp" => Accidental::QuarterSharp,
        "three-quarters-flat" => Accidental::ThreeQuartersFlat,
        "three-quarters-sharp" => Accidental::ThreeQuartersSharp,
        "triple-sharp" => Accidental::TripleSharp,
        "triple-flat" => Accidental::TripleFlat,
        "sharp-down" => Accidental::SharpDown,
        "sharp-up" => Accidental::SharpUp,
        "natural-down" => Accidental::NaturalDown,
        "natural-up" => Accidental::NaturalUp,
        "flat-down" => Accidental::FlatDown,
        "flat-up" => Accidental::FlatUp,
        "double-sharp-down" => Accidental::DoubleSharpDown,
        "double-sharp-up" => Accidental::DoubleSharpUp,
        "flat-flat-down" => Accidental::FlatFlatDown,
        "flat-flat-up" => Accidental::FlatFlatUp,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

struct PartCtx<'a> {
    part_id: &'a str,
    divisions: i64,
    cursor: i64,
    abs_base: i64,
    seq: u32,
    loss: &'a mut LossReport,
    /// open slurs: MusicXML number → start host id
    slurs: BTreeMap<u32, ElementId>,
    /// tuplet stack (nesting = outer→inner)
    tuplets: Vec<TupletBuilder>,
    /// beams under construction: number → members
    beams: BTreeMap<u32, Vec<BeamMembership>>,
    /// last pitched element for <chord/> attachment
    prev_pitched: Option<ElementId>,
    /// tempo points collected in this part (abs ticks → bpm)
    tempo_points: BTreeMap<i64, f64>,
    /// which non-note children already produced a loss entry (dedup)
    reported: std::collections::HashSet<String>,
}

struct TupletBuilder {
    number: u32,
    actual: u32,
    normal: u32,
    normal_type: Option<NoteType>,
    members: Vec<ElementId>,
}

impl<'a> PartCtx<'a> {
    fn elem_path(&self, measure_no: &str, tag: &str) -> String {
        format!("part/{}/measure[{}]/{}", self.part_id, measure_no, tag)
    }
    fn drop_once(&mut self, measure_no: &str, tag: &str, aspect: &str, reason: &str) {
        let key = format!("{}|{}", tag, aspect);
        if self.reported.insert(key) {
            self.loss
                .drop(self.elem_path(measure_no, tag), aspect, reason);
        }
    }
    fn dur_ticks(&mut self, dur: i64, measure_no: &str, tag: &str) -> i64 {
        match divs_to_ticks(dur, self.divisions) {
            Some(t) => t,
            None => {
                let (t, _) = divs_to_ticks_lossy(dur, self.divisions);
                self.loss.approximate(
                    self.elem_path(measure_no, tag),
                    "duration",
                    format!(
                        "divisions={} cannot represent duration {} exactly on the 960000-tick quarter; rounded to {} ticks",
                        self.divisions, dur, t
                    ),
                );
                t
            }
        }
    }
}

/// Import a MusicXML document (score-partwise only). Returns the score
/// and the deterministic import loss report.
pub fn import_musicxml(xml: &str) -> Result<(Score, LossReport)> {
    let root = parse_dom(xml)?;
    match root.name.as_str() {
        "score-partwise" => {}
        "score-timewise" => {
            return Err(NotationError::Unsupported(
                "score-timewise documents are not supported; convert to partwise first".into(),
            ))
        }
        other => {
            return Err(NotationError::Malformed(format!(
                "expected <score-partwise> root, found <{other}>"
            )))
        }
    }
    let mut loss = LossReport::new(ExchangeDirection::Import);
    let mut score = Score::new("");
    score.title = None; // work-title fills this when present

    // Head metadata.
    for c in &root.children {
        match c.name.as_str() {
            "work" => {
                if let Some(t) = c.val("work-title") {
                    score.title = Some(t.to_string());
                }
            }
            "movement-title" => score.movement = Some(c.value().to_string()),
            "movement-number" => {
                loss.drop("movement-number", "metadata", "not modelled");
            }
            "identification" => {
                for ic in &c.children {
                    if ic.name == "creator" {
                        match ic.attr("type").unwrap_or("") {
                            "composer" => score.composer = Some(ic.value().to_string()),
                            t => loss.drop(
                                format!(
                                    "identification/creator[{}]",
                                    if t.is_empty() { "unknown" } else { t }
                                ),
                                "metadata",
                                "only composer is carried",
                            ),
                        }
                    } else if ic.name != "encoding" {
                        loss.drop(
                            format!("identification/{}", ic.name),
                            "metadata",
                            "not modelled",
                        );
                    }
                }
            }
            "defaults" | "credit" | "miscellaneous" => {
                loss.drop(
                    c.name.clone(),
                    "layout",
                    "engraving defaults/credits not carried",
                );
            }
            _ => {}
        }
    }

    // Part list.
    struct PartMeta {
        name: String,
        abbr: Option<String>,
        group: Option<String>,
    }
    let mut metas: Vec<(String, PartMeta)> = Vec::new();
    let mut open_groups: BTreeMap<String, String> = BTreeMap::new();
    if let Some(pl) = root.child("part-list") {
        for c in &pl.children {
            match c.name.as_str() {
                "part-group" => {
                    let num = c.attr("number").unwrap_or("1").to_string();
                    match c.attr("type") {
                        Some("start") => {
                            let name = c.val("group-name").unwrap_or("").to_string();
                            open_groups.insert(num, name);
                            if c.val("group-symbol").is_some() || c.attr("type").is_some() {
                                // group-symbol/barline details are engraving
                            }
                        }
                        Some("stop") => {
                            open_groups.remove(&num);
                        }
                        _ => {}
                    }
                }
                "score-part" => {
                    let id = c.attr("id").unwrap_or("").to_string();
                    let name = c.val("part-name").unwrap_or("").to_string();
                    let abbr = c.val("part-abbreviation").map(|s| s.to_string());
                    let group = open_groups.values().next().cloned();
                    for extra in &c.children {
                        match extra.name.as_str() {
                            "part-name"
                            | "part-abbreviation"
                            | "part-name-display"
                            | "part-abbreviation-display" => {}
                            "score-instrument" | "midi-instrument" | "midi-device"
                            | "virtual-instrument" | "part-symbol" | "group" | "player"
                            | "ensemble" => {
                                loss.drop(
                                    format!("part-list/score-part[{}]/{}", id, extra.name),
                                    "instrumentation",
                                    "instrument/playback assignment is not carried in the score model",
                                );
                            }
                            _ => {}
                        }
                    }
                    metas.push((id, PartMeta { name, abbr, group }));
                }
                _ => {}
            }
        }
    }

    // Parts.
    let mut global_tempo: BTreeMap<i64, f64> = BTreeMap::new();
    for pnode in root.kids("part") {
        let pid = pnode.attr("id").unwrap_or("").to_string();
        let meta_idx = metas.iter().position(|(id, _)| id == &pid);
        let meta = meta_idx.map(|i| metas.remove(i).1);
        let (name, abbr, group) = match meta {
            Some(m) => (m.name, m.abbr, m.group),
            None => {
                loss.drop(
                    format!("part[{}]", pid),
                    "part-list",
                    "part has no score-part entry; imported with empty name",
                );
                (String::new(), None, None)
            }
        };
        let mut part = Part {
            id: pid.clone(),
            name,
            abbreviation: abbr,
            group_name: group,
            measures: Vec::new(),
        };
        let mut ctx = PartCtx {
            part_id: &pid,
            divisions: 1,
            cursor: 0,
            abs_base: 0,
            seq: 0,
            loss: &mut loss,
            slurs: BTreeMap::new(),
            tuplets: Vec::new(),
            beams: BTreeMap::new(),
            prev_pitched: None,
            tempo_points: BTreeMap::new(),
            reported: Default::default(),
        };
        for mnode in pnode.kids("measure") {
            import_measure(&mut ctx, mnode, &mut part)?;
        }
        // Unclosed slurs/tuplets at part end.
        for (num, start) in std::mem::take(&mut ctx.slurs) {
            ctx.loss.drop(
                format!("part/{}/slur[number={}]", pid, num),
                "slur",
                format!("slur starting at {start} has no stop"),
            );
        }
        for tb in &ctx.tuplets {
            ctx.loss.drop(
                format!("part/{}/tuplet[number={}]", pid, tb.number),
                "tuplet",
                "tuplet has no stop",
            );
        }
        for (t, b) in ctx.tempo_points {
            global_tempo.insert(t, b);
        }
        score.parts.push(part);
    }
    score.tempo_map = global_tempo
        .into_iter()
        .map(|(at_ticks, bpm)| TempoPoint { at_ticks, bpm })
        .collect();
    score.normalize();
    Ok((score, loss.sorted()))
}

fn import_measure(ctx: &mut PartCtx, mnode: &Node, part: &mut Part) -> Result<()> {
    ctx.cursor = 0;
    ctx.seq = 0;
    ctx.beams.clear();
    ctx.prev_pitched = None;
    let number = mnode
        .attr("number")
        .map(|s| s.to_string())
        .unwrap_or_else(|| (part.measures.len() + 1).to_string());
    let mi = part.measures.len();
    let mut measure = Measure {
        number: number.clone(),
        attributes: None,
        elements: Vec::new(),
    };
    // The measure is pushed eagerly only at the end, but attachments
    // (slur/tuplet/beam/lyric elements) need somewhere to land while we
    // scan — stage them here and append at the end.
    let mut attached: Vec<Element> = Vec::new();
    let mut max_end: i64 = 0;

    for ch in &mnode.children {
        match ch.name.as_str() {
            "attributes" => import_attributes(ctx, ch, &mut measure, &number),
            "note" => {
                import_note(ctx, ch, &mut measure, &mut attached, &number)?;
            }
            "direction" => import_direction(ctx, ch, &mut measure, &number)?,
            "backup" => {
                let d = ch
                    .val("duration")
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                let t = ctx.dur_ticks(d, &number, "backup");
                ctx.cursor -= t;
                if ctx.cursor < 0 {
                    ctx.loss.approximate(
                        ctx.elem_path(&number, "backup"),
                        "cursor",
                        "backup past measure start; clamped to 0",
                    );
                    ctx.cursor = 0;
                }
            }
            "forward" => {
                let d = ch
                    .val("duration")
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                ctx.cursor += ctx.dur_ticks(d, &number, "forward");
            }
            "barline" => {
                for bc in &ch.children {
                    match bc.name.as_str() {
                        "bar-style" | "repeat" | "ending" | "segno" | "coda" | "fermata"
                        | "wavy-line" => {
                            ctx.drop_once(
                                &number,
                                "barline",
                                &bc.name,
                                "barline markup not carried",
                            );
                        }
                        _ => {}
                    }
                }
            }
            "harmony" => ctx.drop_once(&number, "harmony", "harmony", "chord symbols not modelled"),
            "figured-bass" => {
                ctx.drop_once(&number, "figured-bass", "figured-bass", "not modelled")
            }
            "print" => ctx.drop_once(&number, "print", "layout", "print layout not carried"),
            "sound" => {
                if let Some(tp) = ch.attr("tempo").and_then(|s| s.parse::<f64>().ok()) {
                    ctx.tempo_points.insert(ctx.abs_base + ctx.cursor, tp);
                }
                let other = ch
                    .attrs
                    .keys()
                    .any(|k| k != "tempo" && k != "dynamics" && k != "pizzicato");
                if other {
                    ctx.drop_once(
                        &number,
                        "sound",
                        "sound-attrs",
                        "non-tempo sound attributes not carried",
                    );
                }
            }
            "grouping" | "link" | "bookmark" | "measure-numbering" | "measure-repeat" => {
                ctx.drop_once(&number, &ch.name, &ch.name, "not modelled")
            }
            _ => {}
        }
    }
    // Finalize beams: each number with >= 2 members becomes an element;
    // degenerate one-member beams get a loss entry.
    for (num, members) in std::mem::take(&mut ctx.beams) {
        if members.len() < 2 {
            ctx.loss.drop(
                ctx.elem_path(&number, "beam"),
                "beam",
                format!("beam number {num} has a single member; not a beam"),
            );
            continue;
        }
        let id = derive_id(&format!(
            "beam|{}|{}|{}",
            ctx.part_id, members[0].element, num
        ));
        attached.push(Element {
            id,
            position: None,
            kind: ElementKind::Beam(BeamData {
                number: num,
                members,
            }),
        });
    }
    // Attached entities land after timed content in the same measure.
    measure.elements.extend(attached);
    // Track content extent for abs_base advance.
    for e in &measure.elements {
        if let Some(p) = &e.position {
            max_end = max_end.max(p.offset_ticks + p.duration_ticks);
        }
    }
    // Advance absolute base by the effective measure length: declared
    // time-signature capacity when present, else content extent.
    let capacity = {
        let mut ts = None;
        for m in part.measures.iter() {
            if let Some(a) = &m.attributes {
                if let Some(t) = &a.time {
                    ts = Some(t.clone());
                }
            }
        }
        if let Some(a) = &measure.attributes {
            if let Some(t) = &a.time {
                ts = Some(t.clone());
            }
        }
        ts.map(|t| t.beats as i64 * TICKS_PER_QUARTER * 4 / t.beat_type.max(1) as i64)
    };
    ctx.abs_base += capacity.unwrap_or(max_end).max(0);
    part.measures.push(measure);
    let _ = mi;
    Ok(())
}

fn import_attributes(ctx: &mut PartCtx, anode: &Node, measure: &mut Measure, mno: &str) {
    let attrs = measure.attributes.get_or_insert_with(Default::default);
    let mut have_any = false;
    if let Some(d) = anode.val("divisions").and_then(|s| s.parse::<i64>().ok()) {
        if d <= 0 {
            ctx.drop_once(
                mno,
                "divisions",
                "divisions",
                "non-positive divisions ignored",
            );
        } else {
            ctx.divisions = d;
        }
    }
    let keys: Vec<&Node> = anode.kids("key").collect();
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            ctx.drop_once(
                mno,
                "key",
                "key",
                "multiple key signatures in one block; first carried",
            );
            break;
        }
        if let Some(f) = k.val("fifths").and_then(|s| s.parse::<i32>().ok()) {
            attrs.key = Some(KeySignature {
                fifths: f,
                mode: k.val("mode").map(|s| s.to_string()),
            });
            have_any = true;
        }
        for extra in &k.children {
            match extra.name.as_str() {
                "fifths" | "mode" | "cancel" => {}
                _ => ctx.drop_once(mno, "key", &extra.name, "key sub-element not carried"),
            }
        }
    }
    let times: Vec<&Node> = anode.kids("time").collect();
    for (i, t) in times.iter().enumerate() {
        if i > 0 {
            ctx.drop_once(
                mno,
                "time",
                "time",
                "compound time signature; first carried",
            );
            break;
        }
        match (
            t.val("beats").and_then(|s| s.parse::<u32>().ok()),
            t.val("beat-type").and_then(|s| s.parse::<u32>().ok()),
        ) {
            (Some(beats), Some(beat_type)) => {
                attrs.time = Some(MeasureTimeSignature { beats, beat_type });
                have_any = true;
            }
            _ => ctx.drop_once(mno, "time", "time", "unparseable time signature"),
        }
    }
    if let Some(s) = anode.val("staves").and_then(|s| s.parse::<u32>().ok()) {
        attrs.staves = Some(s);
        have_any = true;
    }
    for c in anode.kids("clef") {
        if let Some(sign) = c.val("sign").and_then(ClefSign::parse) {
            attrs.clefs.push(Clef {
                sign,
                line: c.val("line").and_then(|s| s.parse::<u32>().ok()),
                octave_change: c
                    .val("clef-octave-change")
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(0),
            });
            have_any = true;
        }
    }
    for c in &anode.children {
        match c.name.as_str() {
            "divisions" | "key" | "time" | "staves" | "clef" | "part-symbol" | "footnote"
            | "level" => {}
            "instruments" | "staff-details" | "transpose" | "directive" | "measure-style"
            | "part-transpose" | "for-part" | "accord" => {
                ctx.drop_once(mno, &c.name, &c.name, "attributes sub-element not carried");
            }
            _ => {}
        }
    }
    if !have_any
        && attrs.key.is_none()
        && attrs.time.is_none()
        && attrs.clefs.is_empty()
        && attrs.staves.is_none()
    {
        measure.attributes = None;
    }
}

fn import_note(
    ctx: &mut PartCtx,
    n: &Node,
    measure: &mut Measure,
    attached: &mut Vec<Element>,
    mno: &str,
) -> Result<()> {
    ctx.seq += 1;
    let is_chord_member = n.child("chord").is_some();
    let is_grace = n.child("grace").is_some();
    let is_cue = n.child("cue").is_some();
    if is_grace || is_cue {
        ctx.drop_once(
            mno,
            "note",
            if is_grace { "grace" } else { "cue" },
            "grace/cue notes carry no notated duration in the model; dropped",
        );
        return Ok(());
    }
    if n.child("unpitched").is_some() {
        ctx.drop_once(
            mno,
            "note",
            "unpitched",
            "unpitched notes not modelled; dropped",
        );
        // still advance the cursor so following notes land correctly
        if let Some(d) = n.val("duration").and_then(|s| s.parse::<i64>().ok()) {
            ctx.cursor += ctx.dur_ticks(d, mno, "note");
        }
        return Ok(());
    }
    let dur_divs = n
        .val("duration")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let dur = ctx.dur_ticks(dur_divs, mno, "note");
    let voice = n
        .val("voice")
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1);
    let staff = n
        .val("staff")
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1);
    if n.val("voice")
        .map(|s| s.trim().parse::<u32>().is_err())
        .unwrap_or(false)
    {
        ctx.loss.approximate(
            ctx.elem_path(mno, "note"),
            "voice",
            "non-numeric voice mapped to 1",
        );
    }
    let dots = n.kids("dot").count() as u8;
    let note_type = n.val("type").and_then(parse_note_type);
    if n.val("type").is_some() && note_type.is_none() {
        ctx.drop_once(mno, "type", "type", "unrecognized note type value");
    }
    let mut tie = Tie::default();
    for t in n.kids("tie") {
        match t.attr("type") {
            Some("start") => tie.start = true,
            Some("stop") => tie.stop = true,
            _ => {}
        }
    }
    let accidental = n.val("accidental").and_then(accidental_from_str);
    if n.val("accidental").is_some() && accidental.is_none() {
        ctx.drop_once(
            mno,
            "accidental",
            "accidental",
            "unrecognized accidental value",
        );
    }

    // Pitch / rest.
    let rest = n.child("rest");
    let pitch = n.child("pitch").map(|p| {
        let step = p
            .val("step")
            .and_then(|s| s.trim().chars().next())
            .and_then(Step::from_letter)
            .unwrap_or(Step::C);
        let alter = p
            .val("alter")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        let octave = p
            .val("octave")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(4);
        Pitch {
            step,
            alter,
            octave,
        }
    });
    if rest.is_none() && pitch.is_none() {
        ctx.drop_once(
            mno,
            "note",
            "note",
            "note has neither pitch nor rest; dropped",
        );
        if dur > 0 {
            ctx.cursor += dur;
        }
        return Ok(());
    }

    // Attach as chord member when <chord/> says so.
    let host_id: ElementId;
    let mut member_index: u32 = 0;
    if is_chord_member && n.attr("id").is_some() {
        ctx.loss.approximate(
            ctx.elem_path(mno, "note"),
            "id",
            "chord member note id absorbed into the chord element's id",
        );
    }
    if is_chord_member {
        match ctx.prev_pitched.clone() {
            Some(pid) => {
                let el = measure
                    .elements
                    .iter_mut()
                    .find(|e| e.id == pid)
                    .expect("prev_pitched in this measure");
                match &mut el.kind {
                    ElementKind::Note(nd) => {
                        let p = pitch.expect("chord member needs pitch");
                        let chord = ChordData {
                            pitches: vec![nd.pitch, p],
                            note_type: nd.note_type.or(note_type),
                            dots: nd.dots.max(dots),
                            tie: nd.tie,
                        };
                        *el = Element {
                            id: el.id.clone(),
                            position: el.position,
                            kind: ElementKind::Chord(chord),
                        };
                        member_index = 1;
                        host_id = pid;
                    }
                    ElementKind::Chord(cd) => {
                        cd.pitches.push(pitch.expect("chord member needs pitch"));
                        member_index = cd.pitches.len() as u32 - 1;
                        host_id = pid;
                    }
                    _ => {
                        return Err(NotationError::Malformed(
                            "chord member follows non-pitched element".into(),
                        ))
                    }
                }
            }
            None => {
                ctx.loss.approximate(
                    ctx.elem_path(mno, "note"),
                    "chord",
                    "<chord/> with no preceding note; treated as a separate note",
                );
                host_id = new_note_element(
                    ctx, n, measure, pitch, rest, dur, voice, staff, note_type, dots, tie,
                    accidental, mno,
                )?;
            }
        }
    } else {
        host_id = new_note_element(
            ctx, n, measure, pitch, rest, dur, voice, staff, note_type, dots, tie, accidental, mno,
        )?;
    }

    // Tuplet pre-pass: `tuplet start` opens a builder BEFORE this note
    // joins it — the start note is the first member.
    for nots in n.kids("notations") {
        for nc in &nots.children {
            if nc.name == "tuplet" && nc.attr("type") == Some("start") {
                let num = nc
                    .attr("number")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                ctx.tuplets.push(TupletBuilder {
                    number: num,
                    actual: 0,
                    normal: 0,
                    normal_type: None,
                    members: Vec::new(),
                });
            }
        }
    }
    // This note joins the innermost open tuplet (chord members share
    // their head's membership — not separate members).
    if !is_chord_member {
        if let Some(tb) = ctx.tuplets.last_mut() {
            tb.members.push(host_id.clone());
        }
    }
    // Time-modification fills the innermost open tuplet (MusicXML puts
    // the ratio on member notes, conventionally all of them).
    if let Some(tm) = n.child("time-modification") {
        let actual = tm
            .val("actual-notes")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let normal = tm
            .val("normal-notes")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let nt = tm.val("normal-type").and_then(parse_note_type);
        if let Some(tb) = ctx.tuplets.last_mut() {
            if tb.actual == 0 {
                tb.actual = actual;
                tb.normal = normal;
                tb.normal_type = nt;
            }
        } else {
            ctx.loss.drop(
                ctx.elem_path(mno, "time-modification"),
                "tuplet",
                "time-modification with no open tuplet",
            );
        }
    }

    // Notations: slurs / tuplet stops / articulations / technical /
    // beams / ornaments-loss. `<beam>` elements sit directly under <note>.
    for be in n.kids("beam") {
        let num = be
            .attr("number")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let role = match be.value() {
            "begin" => BeamRole::Begin,
            "continue" => BeamRole::Continue,
            "end" => BeamRole::End,
            "forward hook" => BeamRole::ForwardHook,
            "backward hook" => BeamRole::BackwardHook,
            _ => {
                ctx.drop_once(mno, "beam", "beam", "unrecognized beam value");
                continue;
            }
        };
        ctx.beams.entry(num).or_default().push(BeamMembership {
            element: host_id.clone(),
            role,
        });
    }
    for nots in n.kids("notations") {
        for nc in &nots.children {
            match nc.name.as_str() {
                "tied" => {}
                "slur" => match nc.attr("type") {
                    Some("start") => {
                        let num = nc
                            .attr("number")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        ctx.slurs.insert(num, host_id.clone());
                    }
                    Some("stop") => {
                        let num = nc
                            .attr("number")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        if let Some(start) = ctx.slurs.remove(&num) {
                            let id = derive_id(&format!("slur|{}|{}|{}", start, host_id, num));
                            attached.push(Element {
                                id,
                                position: None,
                                kind: ElementKind::Slur(SlurData {
                                    start,
                                    end: host_id.clone(),
                                    number: num,
                                }),
                            });
                        } else {
                            ctx.loss.drop(
                                ctx.elem_path(mno, "slur"),
                                "slur",
                                "slur stop with no open start",
                            );
                        }
                    }
                    _ => {}
                },
                "tuplet" => match nc.attr("type") {
                    Some("start") => {} // handled in the pre-pass above
                    Some("stop") => {
                        let num = nc
                            .attr("number")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        let pos = ctx
                            .tuplets
                            .iter()
                            .rposition(|t| t.number == num)
                            .unwrap_or_else(|| ctx.tuplets.len().saturating_sub(1));
                        if ctx.tuplets.is_empty() {
                            ctx.loss.drop(
                                ctx.elem_path(mno, "tuplet"),
                                "tuplet",
                                "tuplet stop with no start",
                            );
                            continue;
                        }
                        let tb = ctx.tuplets.remove(pos);
                        if tb.members.len() < 2 {
                            ctx.loss.drop(
                                ctx.elem_path(mno, "tuplet"),
                                "tuplet",
                                "tuplet with fewer than 2 members",
                            );
                            continue;
                        }
                        let actual = if tb.actual > 0 {
                            tb.actual
                        } else {
                            tb.members.len() as u32
                        };
                        let normal = if tb.normal > 0 { tb.normal } else { 2 };
                        let id = derive_id(&format!(
                            "tuplet|{}|{}/{}|{}",
                            tb.members[0],
                            actual,
                            normal,
                            tb.members.len()
                        ));
                        // Nested: inner tuplet id becomes a member of the
                        // enclosing tuplet.
                        if let Some(outer) = ctx.tuplets.last_mut() {
                            outer.members.push(id.clone());
                        }
                        attached.push(Element {
                            id,
                            position: None,
                            kind: ElementKind::Tuplet(TupletData {
                                actual,
                                normal,
                                normal_type: tb.normal_type,
                                members: tb.members,
                            }),
                        });
                    }
                    _ => {}
                },
                "articulations" => {
                    for a in &nc.children {
                        if let Some(kind) = articulation_from_tag(&a.name, &a.text) {
                            let placement = match a.attr("placement") {
                                Some("above") => Some(Placement::Above),
                                Some("below") => Some(Placement::Below),
                                _ => None,
                            };
                            let key = serde_json::to_string(&kind).unwrap_or_default();
                            let id = derive_id(&format!("art|{}|{}", host_id, key));
                            let mut id = id;
                            let mut seq = 2;
                            while measure.elements.iter().any(|e| e.id == id)
                                || attached.iter().any(|e| e.id == id)
                            {
                                id = derive_id(&format!("art|{}|{}|{}", host_id, key, seq));
                                seq += 1;
                            }
                            attached.push(Element {
                                id,
                                position: None,
                                kind: ElementKind::Articulation(ArticulationData {
                                    host: host_id.clone(),
                                    kind,
                                    placement,
                                }),
                            });
                        } else {
                            ctx.drop_once(
                                mno,
                                &a.name,
                                "articulation",
                                "articulation not modelled",
                            );
                        }
                    }
                }
                "technical" => {
                    let mut string = None;
                    let mut fret = None;
                    for tc in &nc.children {
                        match tc.name.as_str() {
                            "string" => string = tc.value().parse::<u32>().ok(),
                            "fret" => fret = tc.value().parse::<u32>().ok(),
                            "fingering" | "fretboards" => {}
                            _ => ctx.drop_once(
                                mno,
                                &tc.name,
                                "technical",
                                "technical marking not modelled",
                            ),
                        }
                    }
                    if string.is_some() || fret.is_some() {
                        let s = string.unwrap_or(0);
                        let f = fret.unwrap_or(0);
                        if s == 0 {
                            ctx.loss.drop(
                                ctx.elem_path(mno, "technical"),
                                "tab",
                                "string element missing/0; tab binding dropped",
                            );
                        } else {
                            let id = derive_id(&format!("tab|{}|{}", host_id, member_index));
                            attached.push(Element {
                                id,
                                position: None,
                                kind: ElementKind::Tab(TabData {
                                    host: host_id.clone(),
                                    member: member_index,
                                    string: s,
                                    fret: f,
                                }),
                            });
                        }
                    }
                }
                "fermata" => {
                    let placement = match nc.attr("type") {
                        Some("inverted") => Some(Placement::Below),
                        _ => match nc.attr("placement") {
                            Some("above") => Some(Placement::Above),
                            Some("below") => Some(Placement::Below),
                            _ => None,
                        },
                    };
                    let id = derive_id(&format!("art|{}|fermata", host_id));
                    attached.push(Element {
                        id,
                        position: None,
                        kind: ElementKind::Articulation(ArticulationData {
                            host: host_id.clone(),
                            kind: ArticulationKind::Fermata,
                            placement,
                        }),
                    });
                }
                "ornaments" => {
                    if nc.children.is_empty() {
                        ctx.drop_once(mno, "ornaments", "notation", "empty ornaments block");
                    } else {
                        for oc in &nc.children {
                            ctx.drop_once(mno, &oc.name, "ornaments", "ornament not modelled");
                        }
                    }
                }
                "dynamics" | "glissando" | "slide" | "arpeggiate" | "non-arpeggiate"
                | "accidental-mark" | "other-notation" | "tuplet-actual" | "tuplet-normal" => {
                    ctx.drop_once(mno, &nc.name, "notation", "notation not modelled");
                }
                _ => {}
            }
        }
    }

    // Lyrics.
    for ly in n.kids("lyric") {
        let num = ly
            .attr("number")
            .map(|s| s.to_string())
            .or_else(|| ly.attr("name").map(|s| s.to_string()))
            .unwrap_or_default();
        let syllabic = match ly.val("syllabic") {
            Some("begin") => Syllabic::Begin,
            Some("middle") => Syllabic::Middle,
            Some("end") => Syllabic::End,
            _ => Syllabic::Single,
        };
        let text = ly.val("text").unwrap_or("").to_string();
        if ly.child("extend").is_some() || ly.child("elision").is_some() {
            ctx.loss.approximate(
                ctx.elem_path(mno, "lyric"),
                "lyric",
                "elision/extend collapsed to text",
            );
        }
        let id = derive_id(&format!("lyr|{}|{}", host_id, num));
        attached.push(Element {
            id,
            position: None,
            kind: ElementKind::Lyric(LyricData {
                host: host_id.clone(),
                number: num,
                syllabic,
                text,
            }),
        });
    }

    if n.child("instrument").is_some() {
        ctx.drop_once(
            mno,
            "instrument",
            "instrument",
            "per-note instrument not carried",
        );
    }
    if !is_chord_member {
        ctx.cursor += dur.max(0);
    }
    Ok(())
}

fn new_note_element(
    ctx: &mut PartCtx,
    _n: &Node,
    measure: &mut Measure,
    pitch: Option<Pitch>,
    rest: Option<&Node>,
    dur: i64,
    voice: u32,
    staff: u32,
    note_type: Option<NoteType>,
    dots: u8,
    tie: Tie,
    accidental: Option<Accidental>,
    mno: &str,
) -> Result<ElementId> {
    let id = match _n.attr("id") {
        Some(s) => ElementId(s.to_string()),
        None => derive_id(&format!(
            "note|{}|{}|{}|{}",
            ctx.part_id, mno, ctx.seq, ctx.cursor
        )),
    };
    let position = Some(Position {
        offset_ticks: ctx.cursor,
        duration_ticks: dur.max(0),
        voice,
        staff,
    });
    let kind = if let Some(r) = rest {
        ElementKind::Rest(RestData {
            measure_rest: r.attr("measure").map(|v| v == "yes").unwrap_or(false),
        })
    } else {
        ctx.prev_pitched = Some(id.clone());
        ElementKind::Note(NoteData {
            pitch: pitch.expect("checked above"),
            note_type,
            dots,
            tie,
            accidental,
        })
    };
    measure.elements.push(Element {
        id: id.clone(),
        position,
        kind,
    });
    Ok(id)
}

fn import_direction(ctx: &mut PartCtx, d: &Node, measure: &mut Measure, mno: &str) -> Result<()> {
    let mut kinds: Vec<DirectionKind> = Vec::new();
    let mut emitted = 0usize;
    for dt in d.kids("direction-type") {
        for c in &dt.children {
            let kind = match c.name.as_str() {
                "words" => Some(DirectionKind::Words {
                    text: c.value().to_string(),
                }),
                "metronome" => {
                    let units: Vec<NoteType> = c
                        .kids("beat-unit")
                        .filter_map(|b| parse_note_type(b.value()))
                        .collect();
                    let dots = c.kids("beat-unit-dot").count() as u8;
                    let pm = c.val("per-minute").and_then(|s| s.parse::<f64>().ok());
                    match (units.first().copied(), pm) {
                        (Some(bu), Some(pm)) => Some(DirectionKind::Metronome {
                            beat_unit: bu,
                            extra_units: units[1..].to_vec(),
                            beat_unit_dots: dots,
                            per_minute: pm,
                        }),
                        _ => {
                            ctx.drop_once(
                                mno,
                                "metronome",
                                "metronome",
                                "unparseable/relation metronome mark",
                            );
                            None
                        }
                    }
                }
                "dynamics" => c.children.first().map(|mk| DirectionKind::Dynamics {
                    mark: mk.name.clone(),
                }),
                "wedge" => match c.attr("type") {
                    Some("crescendo") => Some(DirectionKind::Wedge {
                        wedge: WedgeKind::Crescendo,
                    }),
                    Some("diminuendo") => Some(DirectionKind::Wedge {
                        wedge: WedgeKind::Diminuendo,
                    }),
                    Some("stop") => Some(DirectionKind::Wedge {
                        wedge: WedgeKind::Stop,
                    }),
                    Some("continue") => Some(DirectionKind::Wedge {
                        wedge: WedgeKind::Continue,
                    }),
                    _ => None,
                },
                "rehearsal" => Some(DirectionKind::Rehearsal {
                    text: c.value().to_string(),
                }),
                "segno" => Some(DirectionKind::Segno),
                "coda" => Some(DirectionKind::Coda),
                "bracket"
                | "dashes"
                | "pedal"
                | "octave-shift"
                | "harp-pedals"
                | "damp"
                | "damp-all"
                | "eyeglasses"
                | "string-mute"
                | "scordatura"
                | "image"
                | "principal-voice"
                | "accordion-registration"
                | "percussion"
                | "staff-divide"
                | "other-direction" => {
                    ctx.drop_once(mno, &c.name, "direction", "direction-type not modelled");
                    None
                }
                _ => None,
            };
            if let Some(k) = kind {
                kinds.push(k);
            }
        }
    }
    let voice = d
        .val("voice")
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1);
    let staff = d
        .val("staff")
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1);
    let offset_shift = d
        .val("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|o| ctx.dur_ticks(o, mno, "offset"))
        .unwrap_or(0);
    for k in kinds.drain(..) {
        emitted += 1;
        let tag = match &k {
            DirectionKind::Words { .. } => "words",
            DirectionKind::TempoMark { .. } => "tempo",
            DirectionKind::Metronome { .. } => "metronome",
            DirectionKind::Dynamics { .. } => "dynamics",
            DirectionKind::Wedge { .. } => "wedge",
            DirectionKind::Rehearsal { .. } => "rehearsal",
            DirectionKind::Segno => "segno",
            DirectionKind::Coda => "coda",
        };
        let id = derive_id(&format!(
            "dir|{}|{}|{}|{}|{}",
            ctx.part_id,
            mno,
            ctx.cursor + offset_shift,
            tag,
            emitted
        ));
        measure.elements.push(Element {
            id,
            position: Some(Position {
                offset_ticks: (ctx.cursor + offset_shift).max(0),
                duration_ticks: 0,
                voice,
                staff,
            }),
            kind: ElementKind::Direction(DirectionData { kind: k }),
        });
    }
    // `<sound tempo>` → tempo map point at the direction's position.
    if let Some(snd) = d.child("sound") {
        if let Some(bpm) = snd.attr("tempo").and_then(|s| s.parse::<f64>().ok()) {
            ctx.tempo_points
                .insert(ctx.abs_base + (ctx.cursor + offset_shift).max(0), bpm);
        }
        let other = snd.attrs.keys().any(|k| k.as_str() != "tempo");
        if other {
            ctx.drop_once(
                mno,
                "sound",
                "sound-attrs",
                "non-tempo sound attributes (dynamics/pan/pizzicato/…) are playback hints, not carried",
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

fn xattr(buf: &mut String, name: &str, val: &str) {
    buf.push(' ');
    buf.push_str(name);
    buf.push_str("=\"");
    buf.push_str(&escape(val));
    buf.push('"');
}

fn tag(buf: &mut String, ind: usize, name: &str, val: &str) {
    buf.push_str(&"  ".repeat(ind));
    buf.push('<');
    buf.push_str(name);
    buf.push('>');
    buf.push_str(&escape(val));
    buf.push_str("</");
    buf.push_str(name);
    buf.push_str(">\n");
}

fn open_tag(buf: &mut String, ind: usize, name: &str, attrs: &str) {
    buf.push_str(&"  ".repeat(ind));
    buf.push('<');
    buf.push_str(name);
    buf.push_str(attrs);
    buf.push_str(">\n");
}

fn close_tag(buf: &mut String, ind: usize, name: &str) {
    buf.push_str(&"  ".repeat(ind));
    buf.push_str("</");
    buf.push_str(name);
    buf.push_str(">\n");
}

fn empty_tag(buf: &mut String, ind: usize, name: &str, attrs: &str) {
    buf.push_str(&"  ".repeat(ind));
    buf.push('<');
    buf.push_str(name);
    buf.push_str(attrs);
    buf.push_str("/>\n");
}

fn fmt_f64(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn gcd_i64(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

/// Export the score as a MusicXML 4.0 `<score-partwise>` document plus
/// the deterministic export loss report.
pub fn export_musicxml(score: &Score) -> Result<(String, LossReport)> {
    let mut loss = LossReport::new(ExchangeDirection::Export);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    open_tag(&mut out, 0, "score-partwise", " version=\"4.0\"");

    if let Some(t) = &score.title {
        open_tag(&mut out, 1, "work", "");
        tag(&mut out, 2, "work-title", t);
        close_tag(&mut out, 1, "work");
    }
    if let Some(mv) = &score.movement {
        tag(&mut out, 1, "movement-title", mv);
    }
    if let Some(comp) = &score.composer {
        open_tag(&mut out, 1, "identification", "");
        let mut ln = String::new();
        tag(&mut ln, 0, "creator", comp);
        // splice the type attribute into the creator tag
        let ln = ln.replacen("<creator>", "<creator type=\"composer\">", 1);
        out.push_str(&ln);
        close_tag(&mut out, 1, "identification");
    }

    // part-list with groups.
    open_tag(&mut out, 1, "part-list", "");
    let mut open_group = false;
    let mut group_no = 0u32;
    for (i, p) in score.parts.iter().enumerate() {
        let next_group = p.group_name.clone();
        if let Some(g) = &next_group {
            if !open_group {
                group_no += 1;
                open_tag(
                    &mut out,
                    2,
                    "part-group",
                    &format!(" number=\"{}\" type=\"start\"", group_no),
                );
                tag(&mut out, 3, "group-name", g);
                close_tag(&mut out, 2, "part-group");
                open_group = true;
            }
        }
        let mut sa = String::new();
        xattr(&mut sa, "id", &p.id);
        open_tag(&mut out, 2, "score-part", &sa);
        tag(&mut out, 3, "part-name", &p.name);
        if let Some(a) = &p.abbreviation {
            tag(&mut out, 3, "part-abbreviation", a);
        }
        close_tag(&mut out, 2, "score-part");
        let next_differs = score
            .parts
            .get(i + 1)
            .map(|n| n.group_name != next_group)
            .unwrap_or(true);
        if open_group && next_differs {
            open_tag(
                &mut out,
                2,
                "part-group",
                &format!(" number=\"{}\" type=\"stop\"", group_no),
            );
            close_tag(&mut out, 2, "part-group");
            open_group = false;
        }
    }
    close_tag(&mut out, 1, "part-list");

    // Tempo-map directions for part 0 insertion, absolute → (measure,
    // offset). Bounds use the same advance rule as import: declared
    // time-signature capacity, else content extent.
    let tempo_events: Vec<(usize, i64, f64)> = if let Some(p0) = score.parts.first() {
        let mut bounds: Vec<(i64, i64)> = Vec::new(); // start,end per measure
        let mut acc = 0i64;
        for (mi, m) in p0.measures.iter().enumerate() {
            let content_end = m
                .elements
                .iter()
                .filter_map(|e| {
                    e.position
                        .as_ref()
                        .map(|p| p.offset_ticks + p.duration_ticks)
                })
                .max()
                .unwrap_or(0);
            let len = p0.measure_len_ticks(mi).unwrap_or(content_end).max(0);
            bounds.push((acc, acc + len));
            acc += len;
        }
        score
            .tempo_map
            .iter()
            .map(|tp| {
                let pos = bounds
                    .iter()
                    .position(|(s, e)| tp.at_ticks >= *s && tp.at_ticks < *e);
                match pos {
                    Some(mi) => Ok((mi, tp.at_ticks - bounds[mi].0, tp.bpm)),
                    None => {
                        if tp.at_ticks == acc && acc > 0 {
                            Ok((
                                bounds.len() - 1,
                                bounds.last().map(|b| b.1 - b.0).unwrap_or(0),
                                tp.bpm,
                            ))
                        } else {
                            Err(tp.at_ticks)
                        }
                    }
                }
            })
            .filter(|r| match r {
                Ok(_) => true,
                Err(t) => {
                    loss.drop(
                        "tempoMap",
                        "tempo",
                        format!("tempo point at ticks {t} beyond score length; dropped"),
                    );
                    false
                }
            })
            .map(|r| r.unwrap())
            .collect()
    } else {
        if !score.tempo_map.is_empty() {
            loss.drop("tempoMap", "tempo", "no part to carry tempo map; dropped");
        }
        Vec::new()
    };

    if !score.anchors.is_empty() {
        loss.approximate(
            "score/anchors",
            "anchors",
            "MusicXML has no absolute-time anchor concept; anchors not exported",
        );
    }
    if score.timecode.is_some() {
        loss.drop(
            "score/timecode",
            "timecode",
            "MusicXML carries no picture timecode mode; dropped",
        );
    }

    for (pi, p) in score.parts.iter().enumerate() {
        // Per-part divisions: gcd over all timed offsets+durations and
        // TPQ so every duration lands on an integer.
        let mut unit = TICKS_PER_QUARTER;
        for m in &p.measures {
            for e in &m.elements {
                if let Some(pos) = &e.position {
                    unit = gcd_i64(unit, pos.offset_ticks);
                    unit = gcd_i64(unit, pos.duration_ticks.max(1));
                }
            }
        }
        if pi == 0 {
            for (_, off, _) in &tempo_events {
                unit = gcd_i64(unit, *off.max(&0));
            }
        }
        let divisions = TICKS_PER_QUARTER / unit;
        let to_units = |ticks: i64| ticks / unit;
        let mut divisions_emitted = false;

        // Part-level attachment lookup tables — a slur's endpoints may
        // sit in different measures, so attachments are indexed over the
        // whole part, not per measure.
        let mut slur_start: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        let mut slur_stop: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
        let mut beam_of: BTreeMap<&str, Vec<(u32, BeamRole)>> = BTreeMap::new();
        let mut arts_of: BTreeMap<&str, Vec<&ArticulationData>> = BTreeMap::new();
        let mut tabs_of: BTreeMap<(&str, u32), &TabData> = BTreeMap::new();
        let mut lyrics_of: BTreeMap<&str, Vec<&LyricData>> = BTreeMap::new();
        // member id → [(tuplet element id, member index, member count)]
        let mut tuplet_member_of: BTreeMap<String, Vec<(&ElementId, usize, usize)>> =
            BTreeMap::new();
        let mut tuplet_by_id: BTreeMap<&str, &TupletData> = BTreeMap::new();
        // tuplet element id → MusicXML number (stable across members)
        let mut tuplet_numbers: BTreeMap<&str, u32> = BTreeMap::new();
        for m in &p.measures {
            for e in &m.elements {
                match &e.kind {
                    ElementKind::Slur(s) => {
                        slur_start
                            .entry(s.start.as_str())
                            .or_default()
                            .push(s.number);
                        slur_stop.entry(s.end.as_str()).or_default().push(s.number);
                    }
                    ElementKind::Tuplet(t) => {
                        let n = tuplet_numbers.len() as u32 + 1;
                        tuplet_numbers.insert(e.id.as_str(), n);
                        tuplet_by_id.insert(e.id.as_str(), t);
                        for (idx, mem) in t.members.iter().enumerate() {
                            tuplet_member_of
                                .entry(mem.as_str().to_string())
                                .or_default()
                                .push((&e.id, idx, t.members.len()));
                        }
                    }
                    ElementKind::Beam(b) => {
                        for bm in &b.members {
                            beam_of
                                .entry(bm.element.as_str())
                                .or_default()
                                .push((b.number, bm.role));
                        }
                    }
                    ElementKind::Articulation(a) => {
                        arts_of.entry(a.host.as_str()).or_default().push(a);
                    }
                    ElementKind::Tab(t) => {
                        tabs_of.insert((t.host.as_str(), t.member), t);
                    }
                    ElementKind::Lyric(l) => {
                        lyrics_of.entry(l.host.as_str()).or_default().push(l);
                    }
                    _ => {}
                }
            }
        }
        // leaf member id → ordered [(tuplet_id, is_first, is_last)],
        // innermost first. Tuplet members that are themselves tuplets
        // resolve down to their timed leaves.
        fn expand_tuplets<'x>(
            member: &str,
            all: &BTreeMap<String, Vec<(&'x ElementId, usize, usize)>>,
            out: &mut Vec<(&'x ElementId, bool, bool)>,
        ) {
            if let Some(parents) = all.get(member) {
                for (pid, idx, len) in parents {
                    out.push((pid, *idx == 0, *idx + 1 == *len));
                    expand_tuplets(pid.as_str(), all, out);
                }
            }
        }
        let mut leaf_tuplets: BTreeMap<String, Vec<(&ElementId, bool, bool)>> = BTreeMap::new();
        for m in &p.measures {
            for e in &m.elements {
                if e.timed() {
                    let mut v = Vec::new();
                    expand_tuplets(e.id.as_str(), &tuplet_member_of, &mut v);
                    if !v.is_empty() {
                        leaf_tuplets.insert(e.id.as_str().to_string(), v);
                    }
                }
            }
        }

        let mut pa = String::new();
        xattr(&mut pa, "id", &p.id);
        open_tag(&mut out, 1, "part", &pa);
        for (mi, m) in p.measures.iter().enumerate() {
            let mut ma = String::new();
            xattr(&mut ma, "number", &m.number);
            open_tag(&mut out, 2, "measure", &ma);

            // Attributes (first measure always emits divisions).
            let need_attrs = !divisions_emitted || m.attributes.is_some();
            if need_attrs {
                open_tag(&mut out, 3, "attributes", "");
                if !divisions_emitted {
                    tag(&mut out, 4, "divisions", &divisions.to_string());
                    divisions_emitted = true;
                }
                if let Some(a) = &m.attributes {
                    if let Some(k) = &a.key {
                        open_tag(&mut out, 4, "key", "");
                        tag(&mut out, 5, "fifths", &k.fifths.to_string());
                        if let Some(md) = &k.mode {
                            tag(&mut out, 5, "mode", md);
                        }
                        close_tag(&mut out, 4, "key");
                    }
                    if let Some(t) = &a.time {
                        open_tag(&mut out, 4, "time", "");
                        tag(&mut out, 5, "beats", &t.beats.to_string());
                        tag(&mut out, 5, "beat-type", &t.beat_type.to_string());
                        close_tag(&mut out, 4, "time");
                    }
                    if let Some(s) = a.staves {
                        tag(&mut out, 4, "staves", &s.to_string());
                    }
                    for (ci, c) in a.clefs.iter().enumerate() {
                        open_tag(&mut out, 4, "clef", &format!(" number=\"{}\"", ci + 1));
                        tag(&mut out, 5, "sign", c.sign.as_str());
                        if let Some(l) = c.line {
                            tag(&mut out, 5, "line", &l.to_string());
                        }
                        if c.octave_change != 0 {
                            tag(
                                &mut out,
                                5,
                                "clef-octave-change",
                                &c.octave_change.to_string(),
                            );
                        }
                        close_tag(&mut out, 4, "clef");
                    }
                }
                close_tag(&mut out, 3, "attributes");
            }

            // Voice passes.
            let mut voices: Vec<u32> = m
                .elements
                .iter()
                .filter_map(|e| e.position.as_ref().map(|p| p.voice))
                .collect();
            voices.sort_unstable();
            voices.dedup();
            let default_voice = 1u32;
            let pass_voices: Vec<u32> = if voices.is_empty() {
                vec![default_voice]
            } else {
                voices
            };
            let measure_len = p
                .measure_len_ticks(mi)
                .unwrap_or_else(|| {
                    m.elements
                        .iter()
                        .filter_map(|e| {
                            e.position
                                .as_ref()
                                .map(|p| p.offset_ticks + p.duration_ticks)
                        })
                        .max()
                        .unwrap_or(0)
                })
                .max(0);
            for (vi, voice) in pass_voices.iter().enumerate() {
                let mut events: Vec<&Element> = m
                    .elements
                    .iter()
                    .filter(|e| {
                        e.position
                            .as_ref()
                            .map(|p| {
                                p.voice == *voice
                                    && matches!(
                                        e.kind,
                                        ElementKind::Note(_)
                                            | ElementKind::Rest(_)
                                            | ElementKind::Chord(_)
                                    )
                            })
                            .unwrap_or(false)
                    })
                    .collect();
                // Directions ride the first voice pass. Synthetic
                // tempo-map directions are owned elements merged in here.
                let mut dirs: Vec<Element> = Vec::new();
                if vi == 0 {
                    dirs.extend(
                        m.elements
                            .iter()
                            .filter(|e| matches!(e.kind, ElementKind::Direction(_)))
                            .cloned(),
                    );
                    if pi == 0 {
                        for (tmi, off, bpm) in &tempo_events {
                            if *tmi == mi {
                                dirs.push(Element {
                                    id: ElementId::from(format!("__tempo{tmi}")),
                                    position: Some(Position {
                                        offset_ticks: *off,
                                        duration_ticks: 0,
                                        voice: 1,
                                        staff: 1,
                                    }),
                                    kind: ElementKind::Direction(DirectionData {
                                        kind: DirectionKind::TempoMark { bpm: *bpm },
                                    }),
                                });
                            }
                        }
                    }
                }
                events.sort_by_key(|e| e.position.as_ref().unwrap().offset_ticks);
                // Merge: emit in offset order; dirs sorted by offset too.
                let mut cursor = 0i64;
                let mut ei = 0usize;
                let mut di = 0usize;
                dirs.sort_by_key(|e| e.position.as_ref().unwrap().offset_ticks);
                while ei < events.len() || di < dirs.len() {
                    let e_off = events
                        .get(ei)
                        .map(|e| e.position.as_ref().unwrap().offset_ticks)
                        .unwrap_or(i64::MAX);
                    let d_off = dirs
                        .get(di)
                        .map(|d| d.position.as_ref().unwrap().offset_ticks)
                        .unwrap_or(i64::MAX);
                    if d_off < e_off || (ei >= events.len() && di < dirs.len()) {
                        if d_off > cursor {
                            open_tag(&mut out, 3, "forward", "");
                            tag(
                                &mut out,
                                4,
                                "duration",
                                &to_units(d_off - cursor).to_string(),
                            );
                            close_tag(&mut out, 3, "forward");
                        } else if d_off < cursor {
                            open_tag(&mut out, 3, "backup", "");
                            tag(
                                &mut out,
                                4,
                                "duration",
                                &to_units(cursor - d_off).to_string(),
                            );
                            close_tag(&mut out, 3, "backup");
                        }
                        cursor = d_off;
                        emit_direction(&mut out, &dirs[di], &mut loss);
                        di += 1;
                    } else {
                        let e = events[ei];
                        if e_off > cursor {
                            open_tag(&mut out, 3, "forward", "");
                            tag(
                                &mut out,
                                4,
                                "duration",
                                &to_units(e_off - cursor).to_string(),
                            );
                            close_tag(&mut out, 3, "forward");
                        } else if e_off < cursor {
                            open_tag(&mut out, 3, "backup", "");
                            tag(
                                &mut out,
                                4,
                                "duration",
                                &to_units(cursor - e_off).to_string(),
                            );
                            close_tag(&mut out, 3, "backup");
                        }
                        cursor = e_off;
                        let dur_units = to_units(e.position.as_ref().unwrap().duration_ticks);
                        emit_note(
                            &mut out,
                            e,
                            dur_units,
                            &slur_start,
                            &slur_stop,
                            &leaf_tuplets,
                            &tuplet_by_id,
                            &tuplet_numbers,
                            &beam_of,
                            &arts_of,
                            &tabs_of,
                            &lyrics_of,
                        );
                        cursor += e.position.as_ref().unwrap().duration_ticks.max(0);
                        ei += 1;
                    }
                }
                if vi + 1 < pass_voices.len() && cursor > 0 {
                    open_tag(&mut out, 3, "backup", "");
                    tag(&mut out, 4, "duration", &to_units(cursor).to_string());
                    close_tag(&mut out, 3, "backup");
                }
            }
            let _ = measure_len;
            close_tag(&mut out, 2, "measure");
        }
        close_tag(&mut out, 1, "part");
    }
    close_tag(&mut out, 0, "score-partwise");
    Ok((out, loss.sorted()))
}

#[allow(clippy::too_many_arguments)]
fn emit_note(
    out: &mut String,
    e: &Element,
    dur_units: i64,
    slur_start: &BTreeMap<&str, Vec<u32>>,
    slur_stop: &BTreeMap<&str, Vec<u32>>,
    leaf_tuplets: &BTreeMap<String, Vec<(&ElementId, bool, bool)>>,
    tuplet_by_id: &BTreeMap<&str, &TupletData>,
    tuplet_numbers: &BTreeMap<&str, u32>,
    beam_of: &BTreeMap<&str, Vec<(u32, BeamRole)>>,
    arts_of: &BTreeMap<&str, Vec<&ArticulationData>>,
    tabs_of: &BTreeMap<(&str, u32), &TabData>,
    lyrics_of: &BTreeMap<&str, Vec<&LyricData>>,
) {
    let pos = e.position.as_ref().expect("timed element");
    let id_str = e.id.as_str();
    match &e.kind {
        ElementKind::Note(n) => {
            emit_note_head(
                out,
                e,
                &n.pitch,
                false,
                n.note_type,
                n.dots,
                n.tie,
                n.accidental,
                dur_units,
                pos,
                slur_start,
                slur_stop,
                leaf_tuplets,
                tuplet_by_id,
                tuplet_numbers,
                beam_of,
                arts_of,
                tabs_of,
                lyrics_of,
                0,
            );
        }
        ElementKind::Chord(c) => {
            for (mi2, p) in c.pitches.iter().enumerate() {
                emit_note_head(
                    out,
                    e,
                    p,
                    mi2 > 0,
                    c.note_type,
                    c.dots,
                    c.tie,
                    None,
                    dur_units,
                    pos,
                    slur_start,
                    slur_stop,
                    leaf_tuplets,
                    tuplet_by_id,
                    tuplet_numbers,
                    beam_of,
                    arts_of,
                    tabs_of,
                    lyrics_of,
                    mi2 as u32,
                );
            }
        }
        ElementKind::Rest(r) => {
            let mut na = String::new();
            xattr(&mut na, "id", id_str);
            open_tag(out, 3, "note", &na);
            if r.measure_rest {
                empty_tag(out, 4, "rest", " measure=\"yes\"");
            } else {
                empty_tag(out, 4, "rest", "");
            }
            tag(out, 4, "duration", &dur_units.to_string());
            tag(out, 4, "voice", &pos.voice.to_string());
            tag(out, 4, "staff", &pos.staff.to_string());
            // Lyrics can attach to rests in some files — emit them.
            if let Some(ls) = lyrics_of.get(id_str) {
                emit_lyrics(out, ls);
            }
            close_tag(out, 3, "note");
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_note_head(
    out: &mut String,
    e: &Element,
    pitch: &Pitch,
    is_chord_member: bool,
    note_type: Option<NoteType>,
    dots: u8,
    tie: Tie,
    accidental: Option<Accidental>,
    dur_units: i64,
    pos: &Position,
    slur_start: &BTreeMap<&str, Vec<u32>>,
    slur_stop: &BTreeMap<&str, Vec<u32>>,
    leaf_tuplets: &BTreeMap<String, Vec<(&ElementId, bool, bool)>>,
    tuplet_by_id: &BTreeMap<&str, &TupletData>,
    tuplet_numbers: &BTreeMap<&str, u32>,
    beam_of: &BTreeMap<&str, Vec<(u32, BeamRole)>>,
    arts_of: &BTreeMap<&str, Vec<&ArticulationData>>,
    tabs_of: &BTreeMap<(&str, u32), &TabData>,
    lyrics_of: &BTreeMap<&str, Vec<&LyricData>>,
    member: u32,
) {
    let id_str = e.id.as_str();
    let mut na = String::new();
    // Only the chord's head note carries id — note/@id must be unique
    // per document, and members share the element id.
    if !is_chord_member {
        xattr(&mut na, "id", id_str);
    }
    open_tag(out, 3, "note", &na);
    if is_chord_member {
        empty_tag(out, 4, "chord", "");
    }
    open_tag(out, 4, "pitch", "");
    tag(out, 5, "step", &pitch.step.letter().to_string());
    if pitch.alter != 0 {
        tag(out, 5, "alter", &pitch.alter.to_string());
    }
    tag(out, 5, "octave", &pitch.octave.to_string());
    close_tag(out, 4, "pitch");
    tag(out, 4, "duration", &dur_units.to_string());
    for _ in 0..tie.stop as u8 {
        empty_tag(out, 4, "tie", " type=\"stop\"");
    }
    for _ in 0..tie.start as u8 {
        empty_tag(out, 4, "tie", " type=\"start\"");
    }
    tag(out, 4, "voice", &pos.voice.to_string());
    if let Some(t) = note_type {
        tag(out, 4, "type", note_type_str(t));
    }
    for _ in 0..dots {
        empty_tag(out, 4, "dot", "");
    }
    if let Some(a) = accidental {
        tag(out, 4, "accidental", accidental_to_str(a));
    }
    // time-modification precedes staff per the MusicXML schema.
    if let Some(tl) = leaf_tuplets.get(id_str) {
        for (tid, _, _) in tl {
            if let Some(t) = tuplet_by_id.get(tid.as_str()) {
                open_tag(out, 4, "time-modification", "");
                tag(out, 5, "actual-notes", &t.actual.to_string());
                tag(out, 5, "normal-notes", &t.normal.to_string());
                if let Some(nt) = t.normal_type {
                    tag(out, 5, "normal-type", note_type_str(nt));
                }
                close_tag(out, 4, "time-modification");
            }
        }
    }
    tag(out, 4, "staff", &pos.staff.to_string());
    // Beams: direct children of note, with the beam number attribute.
    if let Some(beams) = beam_of.get(id_str) {
        for (num, role) in beams {
            let v = match role {
                BeamRole::Begin => "begin",
                BeamRole::Continue => "continue",
                BeamRole::End => "end",
                BeamRole::ForwardHook => "forward hook",
                BeamRole::BackwardHook => "backward hook",
            };
            open_tag(out, 4, "beam", &format!(" number=\"{num}\""));
            out.push_str(&"  ".repeat(5));
            out.push_str(v);
            out.push('\n');
            close_tag(out, 4, "beam");
        }
    }
    // Notations.
    let has_notations = slur_start.contains_key(id_str)
        || slur_stop.contains_key(id_str)
        || leaf_tuplets
            .get(id_str)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
        || arts_of.get(id_str).map(|v| !v.is_empty()).unwrap_or(false)
        || tabs_of.get(&(id_str, member)).is_some()
        || tie.start
        || tie.stop;
    if has_notations {
        open_tag(out, 4, "notations", "");
        for _ in 0..tie.stop as u8 {
            empty_tag(out, 5, "tied", " type=\"stop\"");
        }
        for _ in 0..tie.start as u8 {
            empty_tag(out, 5, "tied", " type=\"start\"");
        }
        if let Some(v) = slur_start.get(id_str) {
            for num in v {
                empty_tag(out, 5, "slur", &format!(" type=\"start\" number=\"{num}\""));
            }
        }
        if let Some(v) = slur_stop.get(id_str) {
            for num in v {
                empty_tag(out, 5, "slur", &format!(" type=\"stop\" number=\"{num}\""));
            }
        }
        if let Some(tl) = leaf_tuplets.get(id_str) {
            for (tid, first, last) in tl {
                let n = tuplet_numbers.get(tid.as_str()).copied().unwrap_or(1);
                if *first {
                    empty_tag(out, 5, "tuplet", &format!(" type=\"start\" number=\"{n}\""));
                }
                if *last {
                    empty_tag(out, 5, "tuplet", &format!(" type=\"stop\" number=\"{n}\""));
                }
            }
        }
        // Articulations + fermata (schema: technical before
        // articulations? No — MusicXML orders ornaments, technical,
        // articulations, dynamics, fermata... technical goes first).
        if let Some(t) = tabs_of.get(&(id_str, member)) {
            open_tag(out, 5, "technical", "");
            tag(out, 6, "string", &t.string.to_string());
            tag(out, 6, "fret", &t.fret.to_string());
            close_tag(out, 5, "technical");
        }
        if let Some(arts) = arts_of.get(id_str) {
            let mut plain: Vec<(&str, Option<String>, Option<Placement>)> = Vec::new();
            for a in arts {
                if a.kind == ArticulationKind::Fermata {
                    continue; // fermata emits after articulations
                } else if let Some((tagname, txt)) = articulation_tag(&a.kind) {
                    plain.push((tagname, txt, a.placement));
                }
            }
            if !plain.is_empty() {
                open_tag(out, 5, "articulations", "");
                for (tagname, txt, placement) in plain {
                    let mut aa = String::new();
                    if let Some(p) = placement {
                        aa.push_str(&format!(
                            " placement=\"{}\"",
                            match p {
                                Placement::Above => "above",
                                Placement::Below => "below",
                            }
                        ));
                    }
                    match txt {
                        Some(v) => {
                            open_tag(out, 6, tagname, &aa);
                            out.push_str(&"  ".repeat(7));
                            out.push_str(&escape(&v));
                            out.push('\n');
                            close_tag(out, 6, tagname);
                        }
                        None => empty_tag(out, 6, tagname, &aa),
                    }
                }
                close_tag(out, 5, "articulations");
            }
            for a in arts {
                if a.kind == ArticulationKind::Fermata {
                    let t = match a.placement {
                        Some(Placement::Below) => " type=\"inverted\"",
                        _ => " type=\"upright\"",
                    };
                    empty_tag(out, 5, "fermata", t);
                }
            }
        }
        close_tag(out, 4, "notations");
    }
    // Lyrics attach to the chord as a whole (member 0 carries them).
    if member == 0 {
        if let Some(ls) = lyrics_of.get(id_str) {
            emit_lyrics(out, ls);
        }
    }
    close_tag(out, 3, "note");
}

fn emit_lyrics(out: &mut String, ls: &[&LyricData]) {
    let mut sorted: Vec<&LyricData> = ls.to_vec();
    sorted.sort_by(|a, b| a.number.cmp(&b.number));
    for l in sorted {
        let mut la = String::new();
        if !l.number.is_empty() {
            la = format!(" number=\"{}\"", escape(&l.number));
        }
        open_tag(out, 4, "lyric", &la);
        tag(
            out,
            5,
            "syllabic",
            match l.syllabic {
                Syllabic::Single => "single",
                Syllabic::Begin => "begin",
                Syllabic::Middle => "middle",
                Syllabic::End => "end",
            },
        );
        tag(out, 5, "text", &l.text);
        close_tag(out, 4, "lyric");
    }
}

fn emit_direction(out: &mut String, d: &Element, _loss: &mut LossReport) {
    let pos = d.position.as_ref().expect("direction is timed");
    if let ElementKind::Direction(dd) = &d.kind {
        open_tag(out, 3, "direction", "");
        // TempoMark carries no direction-type — it is a bare
        // `<sound tempo>` beat.
        if !matches!(dd.kind, DirectionKind::TempoMark { .. }) {
            open_tag(out, 4, "direction-type", "");
            match &dd.kind {
                DirectionKind::Words { text } => tag(out, 5, "words", text),
                DirectionKind::Metronome {
                    beat_unit,
                    extra_units,
                    beat_unit_dots,
                    per_minute,
                } => {
                    open_tag(out, 5, "metronome", "");
                    tag(out, 6, "beat-unit", note_type_str(*beat_unit));
                    for _ in 0..*beat_unit_dots {
                        empty_tag(out, 6, "beat-unit-dot", "");
                    }
                    for u in extra_units {
                        tag(out, 6, "beat-unit", note_type_str(*u));
                    }
                    tag(out, 6, "per-minute", &fmt_f64(*per_minute));
                    close_tag(out, 5, "metronome");
                }
                DirectionKind::Dynamics { mark } => {
                    open_tag(out, 5, "dynamics", "");
                    empty_tag(out, 6, mark, "");
                    close_tag(out, 5, "dynamics");
                }
                DirectionKind::Wedge { wedge } => {
                    let t = match wedge {
                        WedgeKind::Crescendo => "crescendo",
                        WedgeKind::Diminuendo => "diminuendo",
                        WedgeKind::Stop => "stop",
                        WedgeKind::Continue => "continue",
                    };
                    empty_tag(out, 5, "wedge", &format!(" type=\"{t}\""));
                }
                DirectionKind::Rehearsal { text } => tag(out, 5, "rehearsal", text),
                DirectionKind::Segno => empty_tag(out, 5, "segno", ""),
                DirectionKind::Coda => empty_tag(out, 5, "coda", ""),
                DirectionKind::TempoMark { .. } => {}
            }
            close_tag(out, 4, "direction-type");
            if pos.voice != 1 {
                tag(out, 4, "voice", &pos.voice.to_string());
            }
            if pos.staff != 1 {
                tag(out, 4, "staff", &pos.staff.to_string());
            }
        }
        if let DirectionKind::TempoMark { bpm } = &dd.kind {
            empty_tag(out, 4, "sound", &format!(" tempo=\"{}\"", fmt_f64(*bpm)));
        }
        close_tag(out, 3, "direction");
    }
}

pub fn round_trip(xml: &str) -> Result<(Score, String, LossReport, LossReport)> {
    let (score, iloss) = import_musicxml(xml)?;
    let (out, eloss) = export_musicxml(&score)?;
    Ok((score, out, iloss, eloss))
}

/// Used by tests asserting the rational anchor round-trip through
/// documents that carry seconds.
pub fn rat_to_seconds(r: &Rat) -> f64 {
    r.to_f64()
}
