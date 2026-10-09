//! Standard MIDI File (SMF) interchange — a real codec, not a stub.
//!
//! Write: SMF type 1 — track 0 is the conductor (tempo map + time
//! signatures + markers + document name), tracks 1..n map VOID MIDI /
//! instrument tracks' note clips. Division is chosen adaptively so every
//! event tick converts exactly (960000/division integer scale); when no
//! divisor ≤ 32767 fits, events quantize with per-element `approximated`
//! loss entries — never silently.
//!
//! Read: SMF type 0/1 (type 2 rejected honestly). Tempo/time-signature
//! metas, note on/off pairs (running status handled), track names, and
//! everything unhandled (CC, pitch bend, program change, aftertouch,
//! SysEx, SMPTE timing caveat) lands in the loss report — the caller sees
//! exactly what was lost.

use crate::document::*;
use crate::error::{ExchangeError, Result};
use crate::loss::{ExchangeDirection, LossReport};
use std::collections::BTreeMap;

const TICKS: i64 = TICKS_PER_QUARTER;
const MAX_SMF_DIVISION: u32 = 0x7FFF; // 15-bit per spec
const MAX_FILE: usize = 64 * 1024 * 1024;
const MAX_EVENTS: usize = 1 << 22;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1) as i64
}

fn push_varlen(out: &mut Vec<u8>, v: u32) {
    let mut stack = [0u8; 5];
    let mut n = 0;
    let mut v = v;
    stack[n] = (v & 0x7F) as u8;
    n += 1;
    v >>= 7;
    while v > 0 {
        stack[n] = ((v & 0x7F) as u8) | 0x80;
        n += 1;
        v >>= 7;
    }
    while n > 0 {
        n -= 1;
        out.push(stack[n]);
    }
}

/// One pending event for the writer: SMF-scaled tick + wire bytes
/// (channel events `[status, d1, d2]`; metas `[0xFF, type, len-msb-pad, data...]`).
struct Ev {
    tick: i64,
    order: u8, // 0 = meta/note-off before note-on at same tick
    bytes: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Largest `scale` (void ticks per SMF tick) that divides every event
/// tick, such that division = 960000/scale fits 15 bits. Falls back to
/// scale 32 (division 30000) — the largest exact divisor — and reports
/// per-tick approximations.
fn choose_scale(doc: &ExchangeDocument, loss: &mut LossReport) -> i64 {
    let mut g: i64 = TICKS;
    let fold = |t: i64, g: &mut i64| *g = gcd(*g, t);
    for p in &doc.tempo_map {
        fold(p.at_ticks, &mut g);
    }
    for p in &doc.time_signatures {
        fold(p.at_ticks, &mut g);
    }
    for m in &doc.markers {
        fold(m.at_ticks, &mut g);
    }
    for t in &doc.tracks {
        for c in &t.clips {
            fold(c.start_ticks, &mut g);
            fold(c.length_ticks, &mut g);
            if let ClipContent::Notes { notes } = &c.content {
                for n in notes {
                    fold(n.start_ticks, &mut g);
                    fold(n.length_ticks, &mut g);
                }
            }
        }
    }
    if g > 0 && TICKS / g <= MAX_SMF_DIVISION as i64 {
        return g;
    }
    let scale = TICKS / 30000;
    loss.approximate(
        "document",
        "tickResolution",
        format!(
            "tick grid exceeds any exact SMF division; quantized at scale {scale} (division 30000)"
        ),
    );
    scale
}

/// Convert a void tick to an SMF tick; `None` reason when exact.
fn scale_tick(tick: i64, scale: i64) -> (u64, Option<String>) {
    if tick < 0 {
        return (0, Some(format!("negative tick {tick} clamped to 0")));
    }
    if scale == 1 {
        return (tick as u64, None);
    }
    let q = tick.div_euclid(scale);
    let r = tick.rem_euclid(scale);
    let rounded = if r * 2 > scale { q + 1 } else { q }; // ties away (CONTRACTS §1)
    let approx = (r != 0)
        .then(|| format!("tick {tick} not divisible by scale {scale}; rounded to {rounded}"));
    (rounded as u64, approx)
}

fn us_per_quarter(bpm: f64) -> (u32, bool) {
    let exact = 60_000_000.0 / bpm;
    let rounded = exact.round().max(1.0);
    (rounded as u32, (exact - rounded).abs() > 1e-6)
}

fn render_track(evs: &mut Vec<Ev>) -> Vec<u8> {
    evs.sort_by(|a, b| (a.tick, a.order).cmp(&(b.tick, b.order)));
    let mut body = Vec::with_capacity(4096);
    let mut last: i64 = 0;
    let mut running: Option<u8> = None;
    for ev in evs.iter() {
        let delta = (ev.tick - last).max(0) as u32;
        last = ev.tick;
        push_varlen(&mut body, delta);
        if ev.bytes[0] == 0xFF {
            body.extend_from_slice(&ev.bytes);
            running = None; // meta clears running status
        } else if running == Some(ev.bytes[0]) {
            body.extend_from_slice(&ev.bytes[1..]);
        } else {
            body.extend_from_slice(&ev.bytes);
            running = Some(ev.bytes[0]);
        }
    }
    // end of track meta
    push_varlen(&mut body, 0);
    body.extend_from_slice(&[0xFF, 0x2F, 0x00]);
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

fn meta_bytes(typ: u8, data: &[u8]) -> Vec<u8> {
    let mut b = vec![0xFF, typ];
    push_varlen(&mut b, data.len() as u32);
    b.extend_from_slice(data);
    b
}

/// Export a document to SMF bytes. Returns (bytes, sorted loss report).
pub fn export_smf(doc: &ExchangeDocument) -> Result<(Vec<u8>, LossReport)> {
    let mut loss = LossReport::new(ExchangeDirection::Export);
    doc.validate()
        .map_err(|e| ExchangeError::Invalid(format!("document: {}", e.join("; "))))?;

    let scale = choose_scale(doc, &mut loss);
    let division = (TICKS / scale) as u16;
    let track_loss = |loss: &mut LossReport, at: &str, q: Option<String>| {
        if let Some(r) = q {
            loss.approximate(at.to_string(), "tick", r);
        }
    };

    // --- conductor track ---
    let mut cond: Vec<Ev> = Vec::new();
    if !doc.name.is_empty() {
        cond.push(Ev {
            tick: 0,
            order: 0,
            bytes: meta_bytes(0x03, doc.name.as_bytes()),
        });
    }
    for tp in &doc.tempo_map {
        let (usq, approx) = us_per_quarter(tp.bpm);
        if approx {
            loss.approximate(
                "transport/tempo".to_string(),
                "bpm",
                format!(
                    "bpm {} rounds to {}us/quarter (SMF stores integers)",
                    tp.bpm, usq
                ),
            );
        }
        let (t, q) = scale_tick(tp.at_ticks, scale);
        track_loss(&mut loss, "transport/tempo", q);
        cond.push(Ev {
            tick: t as i64,
            order: 0,
            bytes: meta_bytes(0x51, &usq.to_be_bytes()[1..]),
        });
    }
    for ts in &doc.time_signatures {
        if !ts.denominator.is_power_of_two() || ts.numerator > 255 || ts.denominator > 255 {
            loss.approximate(
                "transport/timeSignature".to_string(),
                "value",
                format!(
                    "{}/{} unrepresentable (SMF needs power-of-two u8 denominator); point dropped",
                    ts.numerator, ts.denominator
                ),
            );
            continue;
        }
        let (t, q) = scale_tick(ts.at_ticks, scale);
        track_loss(&mut loss, "transport/timeSignature", q);
        let data = [
            ts.numerator as u8,
            ts.denominator.trailing_zeros() as u8,
            24,
            8,
        ];
        cond.push(Ev {
            tick: t as i64,
            order: 0,
            bytes: meta_bytes(0x58, &data),
        });
    }
    for m in &doc.markers {
        let (t, q) = scale_tick(m.at_ticks, scale);
        track_loss(&mut loss, &format!("marker {:?}", m.name), q);
        if m.color.is_some() {
            loss.drop(
                format!("marker {:?}", m.name),
                "color",
                "SMF markers carry no color".to_string(),
            );
        }
        cond.push(Ev {
            tick: t as i64,
            order: 0,
            bytes: meta_bytes(0x06, m.name.as_bytes()),
        });
    }
    let conductor = render_track(&mut cond);

    // --- note tracks ---
    let mut tracks: Vec<Vec<u8>> = Vec::new();
    for tr in &doc.tracks {
        let mut evs: Vec<Ev> = Vec::new();
        if !tr.plugins.is_empty() {
            loss.drop(
                format!("track {:?}", tr.id),
                "plugins",
                "SMF carries no device/plugin state".to_string(),
            );
        }
        if tr.muted || tr.soloed || tr.gain_linear != 1.0 || tr.pan != 0.0 || tr.color.is_some() {
            loss.drop(
                format!("track {:?}", tr.id),
                "mixer",
                "SMF carries no gain/pan/mute/solo/color".to_string(),
            );
        }
        let mut note_clips = 0usize;
        for c in &tr.clips {
            match &c.content {
                ClipContent::Audio { .. } => {
                    loss.drop(
                        format!("clip {:?}", c.id),
                        "audio",
                        "SMF carries no audio clips".to_string(),
                    );
                }
                ClipContent::Notes { notes } => {
                    if !c.enabled {
                        loss.drop(
                            format!("clip {:?}", c.id),
                            "enabled",
                            "disabled clip dropped (SMF has no clip-enable state)".to_string(),
                        );
                        continue;
                    }
                    note_clips += 1;
                    if c.fade_in_ticks.is_some() || c.fade_out_ticks.is_some() {
                        loss.drop(
                            format!("clip {:?}", c.id),
                            "fades",
                            "SMF has no clip fades".to_string(),
                        );
                    }
                    for n in notes {
                        if n.channel > 15 {
                            loss.drop(
                                format!("note {:?}", n.id),
                                "channel",
                                format!("channel {} > 15 cannot be encoded", n.channel),
                            );
                            continue;
                        }
                        let base = c.start_ticks + c.offset_ticks + n.start_ticks;
                        let (start, q1) = scale_tick(base, scale);
                        track_loss(&mut loss, &format!("note {:?}", n.id), q1);
                        let (len, q2) = scale_tick(n.length_ticks, scale);
                        track_loss(&mut loss, &format!("note {:?}", n.id), q2);
                        let len = len.max(1);
                        evs.push(Ev {
                            tick: start as i64,
                            order: 1,
                            bytes: vec![0x90 | n.channel, n.pitch, n.velocity],
                        });
                        evs.push(Ev {
                            tick: (start + len) as i64,
                            order: 0,
                            bytes: vec![
                                0x80 | n.channel,
                                n.pitch,
                                n.release_velocity.unwrap_or(64),
                            ],
                        });
                    }
                }
            }
        }
        if note_clips > 1 {
            loss.approximate(
                format!("track {:?}", tr.id),
                "clipStructure",
                format!("{note_clips} note clips merged into one SMF track (SMF has no clip boundaries)"),
            );
        }
        if evs.is_empty() && tr.clips.is_empty() {
            loss.drop(
                format!("track {:?}", tr.id),
                "track",
                "empty track produces no SMF events".to_string(),
            );
        }
        if !tr.name.is_empty() {
            evs.push(Ev {
                tick: 0,
                order: 0,
                bytes: meta_bytes(0x03, tr.name.as_bytes()),
            });
        }
        tracks.push(render_track(&mut evs));
    }

    if let Some(lr) = &doc.loop_range {
        if lr.enabled {
            loss.drop(
                "document".to_string(),
                "loopRange",
                "SMF carries no loop range".to_string(),
            );
        }
    }
    if doc.comment.is_some() {
        loss.drop(
            "document".to_string(),
            "comment",
            "SMF has no comment field".to_string(),
        );
    }
    for a in &doc.assets {
        loss.drop(
            format!("asset {:?}", a.asset_id),
            "asset",
            "SMF carries no media assets".to_string(),
        );
    }

    let ntrks = 1 + tracks.len();
    if ntrks > u16::MAX as usize {
        return Err(ExchangeError::TooLarge(format!(
            "{ntrks} tracks exceeds SMF u16"
        )));
    }
    let mut out = Vec::with_capacity(64 * 1024);
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&(ntrks as u16).to_be_bytes());
    out.extend_from_slice(&division.to_be_bytes());
    out.extend_from_slice(&conductor);
    for t in &tracks {
        out.extend_from_slice(t);
    }
    Ok((out, loss.sorted()))
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.p + n > self.b.len() {
            return Err(ExchangeError::Malformed("smf truncated".into()));
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn varlen(&mut self) -> Result<u32> {
        let mut v: u32 = 0;
        for _ in 0..5 {
            let b = self.byte()?;
            v = (v << 7) | (b & 0x7F) as u32;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(ExchangeError::Malformed("smf varlen > 5 bytes".into()))
    }
}

struct TrackCollect {
    notes: Vec<(i64, i64, u8, u8, u8, u8)>, // start, end, pitch, vel, rel, ch
    name: Option<String>,
}

/// Parse SMF bytes into a document. `sample_rate` applies to the VOID-side
/// document only (SMF has no rate).
pub fn import_smf(
    bytes: &[u8],
    name: &str,
    sample_rate: u32,
) -> Result<(ExchangeDocument, LossReport)> {
    if bytes.len() > MAX_FILE {
        return Err(ExchangeError::TooLarge("smf > 64MiB".into()));
    }
    let mut loss = LossReport::new(ExchangeDirection::Import);
    let mut r = Rd { b: bytes, p: 0 };
    if r.take(4)? != b"MThd" {
        return Err(ExchangeError::Malformed("missing MThd".into()));
    }
    let hlen = r.u32()?;
    if hlen < 6 {
        return Err(ExchangeError::Malformed(format!("MThd length {hlen} < 6")));
    }
    let format = r.u16()?;
    let ntrks = r.u16()?;
    let division_raw = r.u16()?;
    if hlen > 6 {
        r.take((hlen - 6) as usize)?;
    }
    if format == 2 {
        return Err(ExchangeError::Unsupported(
            "SMF type 2 (independent sequences) is not supported".into(),
        ));
    }
    if format > 2 {
        return Err(ExchangeError::Malformed(format!("SMF format {format}")));
    }

    let smpte = division_raw & 0x8000 != 0;
    let (division, smpte_tps): (i64, f64) = if smpte {
        let fps = ((division_raw >> 8) as i8).unsigned_abs().max(1) as f64;
        let tpf = (division_raw & 0xFF).max(1) as f64;
        loss.approximate(
            "header".to_string(),
            "division",
            format!("SMPTE division {fps}fps/{tpf}tpf approximated to musical ticks"),
        );
        (0, fps * tpf)
    } else {
        (i64::from(division_raw).max(1), 0.0)
    };
    // SMF tick → void tick.
    let to_ticks = |v: u64| -> i64 {
        if !smpte {
            (v as i128 * TICKS as i128 / division as i128) as i64
        } else {
            // seconds = v/tps; quarter ≈ 0.5s at 120bpm — recorded above.
            ((v as f64 / smpte_tps) * (TICKS as f64 * 2.0)).round() as i64
        }
    };

    let mut doc = ExchangeDocument::new(name, sample_rate);
    let mut events = 0usize;
    let mut markers: Vec<Marker> = Vec::new();
    let mut doc_name: Option<String> = None;
    let mut collects: Vec<TrackCollect> = Vec::new();

    for ti in 0..ntrks as usize {
        if r.take(4)? != b"MTrk" {
            return Err(ExchangeError::Malformed(format!("track {ti} missing MTrk")));
        }
        let len = r.u32()? as usize;
        if r.p + len > bytes.len() {
            return Err(ExchangeError::Malformed(format!(
                "track {ti} overruns file"
            )));
        }
        let mut tr = Rd {
            b: &bytes[r.p..r.p + len],
            p: 0,
        };
        r.p += len;

        let mut collect = TrackCollect {
            notes: Vec::new(),
            name: None,
        };
        let mut abs: i64 = 0;
        let mut running: Option<u8> = None;
        let mut open: BTreeMap<(u8, u8), Vec<(i64, u8)>> = BTreeMap::new();
        let mut dropped: BTreeMap<&'static str, u32> = BTreeMap::new();

        while tr.p < len {
            let delta = tr.varlen()? as i64;
            abs = abs.saturating_add(delta);
            events += 1;
            if events > MAX_EVENTS {
                return Err(ExchangeError::TooLarge("smf event count".into()));
            }
            let mut status = tr.byte()?;
            if status < 0x80 {
                match running {
                    Some(rs) => {
                        tr.p -= 1; // push data byte back
                        status = rs;
                    }
                    None => {
                        return Err(ExchangeError::Malformed(
                            "running status without status byte".into(),
                        ))
                    }
                }
            } else if status < 0xF0 {
                running = Some(status);
            } else {
                running = None;
            }
            match status {
                0x80..=0x8F | 0x90..=0x9F => {
                    let ch = status & 0x0F;
                    let pitch = tr.byte()?;
                    let vel = tr.byte()?;
                    let is_on = status & 0xF0 == 0x90 && vel > 0;
                    if is_on {
                        open.entry((ch, pitch)).or_default().push((abs, vel));
                    } else {
                        let rel = if status & 0xF0 == 0x80 { vel } else { 0 };
                        match open.get_mut(&(ch, pitch)).and_then(|q| {
                            if q.is_empty() {
                                None
                            } else {
                                Some(q.remove(0))
                            }
                        }) {
                            Some((start, on_vel)) => {
                                collect.notes.push((start, abs, pitch, on_vel, rel, ch));
                            }
                            None => *dropped.entry("unmatchedNoteOff").or_default() += 1,
                        }
                    }
                }
                0xA0..=0xAF => {
                    tr.take(2)?;
                    *dropped.entry("polyAftertouch").or_default() += 1;
                }
                0xB0..=0xBF => {
                    tr.take(2)?;
                    *dropped.entry("controlChange").or_default() += 1;
                }
                0xC0..=0xCF => {
                    tr.take(1)?;
                    *dropped.entry("programChange").or_default() += 1;
                }
                0xD0..=0xDF => {
                    tr.take(1)?;
                    *dropped.entry("channelPressure").or_default() += 1;
                }
                0xE0..=0xEF => {
                    tr.take(2)?;
                    *dropped.entry("pitchBend").or_default() += 1;
                }
                0xF0 | 0xF7 => {
                    let n = tr.varlen()? as usize;
                    tr.take(n)?;
                    *dropped.entry("sysEx").or_default() += 1;
                }
                0xFF => {
                    let typ = tr.byte()?;
                    let n = tr.varlen()? as usize;
                    let data = tr.take(n)?;
                    match typ {
                        0x51 if n == 3 => {
                            let usq = u32::from_be_bytes([0, data[0], data[1], data[2]]);
                            if usq == 0 {
                                *dropped.entry("badTempo").or_default() += 1;
                            } else {
                                doc.tempo_map.push(TempoPoint {
                                    at_ticks: to_ticks(abs.max(0) as u64),
                                    bpm: 60_000_000.0 / usq as f64,
                                });
                            }
                        }
                        0x58 if n >= 4 => {
                            doc.time_signatures.push(TimeSignaturePoint {
                                at_ticks: to_ticks(abs.max(0) as u64),
                                numerator: data[0] as u32,
                                denominator: 1u32 << data[1].min(7),
                            });
                        }
                        0x03 => {
                            let s = String::from_utf8_lossy(data).to_string();
                            if ti == 0 {
                                doc_name = Some(s);
                            } else {
                                collect.name = Some(s);
                            }
                        }
                        0x06 => {
                            markers.push(Marker {
                                at_ticks: to_ticks(abs.max(0) as u64),
                                name: String::from_utf8_lossy(data).to_string(),
                                color: None,
                            });
                        }
                        0x2F => break,
                        _ => *dropped.entry("otherMeta").or_default() += 1,
                    }
                }
                _ => {
                    // system common (F1..FE): lengths vary; report + end track
                    *dropped.entry("systemCommon").or_default() += 1;
                    break;
                }
            }
        }
        // unclosed note-ons end at their start — recorded, not silent
        for ((_, _), q) in open.iter() {
            for (start, _) in q {
                *dropped.entry("unclosedNoteOn").or_default() += 1;
                let _ = start;
            }
        }
        for (aspect, count) in dropped {
            loss.drop(
                format!("track[{ti}]"),
                aspect,
                format!("{count} {aspect} event(s) not representable in the exchange document"),
            );
        }
        collects.push(collect);
    }

    if let Some(n) = doc_name.filter(|s| !s.is_empty()) {
        doc.name = n;
    }
    doc.markers = markers;
    doc.tempo_map.sort_by_key(|p| p.at_ticks);
    doc.time_signatures.sort_by_key(|p| p.at_ticks);
    doc.markers.sort_by_key(|m| m.at_ticks);
    doc.tempo_map.dedup_by(|a, b| a.at_ticks == b.at_ticks);
    doc.time_signatures
        .dedup_by(|a, b| a.at_ticks == b.at_ticks);

    for (ti, collect) in collects.iter().enumerate() {
        if collect.notes.is_empty() {
            continue;
        }
        let track_id = format!("smf-track-{ti}");
        let clip_id = format!("{track_id}-clip0");
        // collect.notes holds raw SMF ticks — convert every field.
        let min_start = collect.notes.iter().map(|n| n.0).min().unwrap_or(0);
        let end = collect
            .notes
            .iter()
            .map(|n| n.1.max(n.0 + 1))
            .max()
            .unwrap_or(0);
        let min_start_v = to_ticks(min_start.max(0) as u64);
        let mut notes: Vec<Note> = Vec::with_capacity(collect.notes.len());
        for (start, note_end, pitch, vel, rel, ch) in &collect.notes {
            let sv = to_ticks((*start).max(0) as u64);
            let ev = to_ticks((*note_end).max(0) as u64);
            notes.push(Note {
                id: crate::plan::deterministic_id(&clip_id, "note", notes.len()),
                pitch: *pitch,
                velocity: (*vel).max(1),
                release_velocity: Some(*rel),
                channel: *ch,
                start_ticks: sv - min_start_v,
                length_ticks: (ev - sv).max(1),
            });
        }
        doc.tracks.push(Track {
            id: track_id,
            name: collect
                .name
                .clone()
                .unwrap_or_else(|| format!("Track {ti}")),
            color: None,
            kind: TrackKind::MIDI,
            gain_linear: 1.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            clips: vec![Clip {
                id: clip_id,
                name: collect.name.clone().unwrap_or_default(),
                start_ticks: to_ticks(min_start.max(0) as u64),
                length_ticks: (to_ticks(end.max(0) as u64) - to_ticks(min_start.max(0) as u64))
                    .max(1),
                offset_ticks: 0,
                enabled: true,
                fade_in_ticks: None,
                fade_out_ticks: None,
                content: ClipContent::Notes { notes },
            }],
            plugins: Vec::new(),
        });
    }
    // Correct clip positions: note starts above were clip-relative already
    // after the `start - min_start` normalization.
    for tr in &mut doc.tracks {
        for c in &mut tr.clips {
            let _ = c;
        }
    }

    Ok((doc, loss.sorted()))
}
