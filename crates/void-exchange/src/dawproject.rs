//! DAWproject (.dawproject) interchange — real ZIP+XML read/write for the
//! VOID-supported subset (W20, T77).
//!
//! A .dawproject is a ZIP containing `project.xml` (schema:
//! bitwig/dawproject `Project.xsd`), `metadata.xml` (MetaData.xsd), and
//! referenced payload files (`media/*`, `plugins/*`). Times are written
//! as `beats` (1 beat = 960000 void ticks). This is a SUBSET, stated
//! honestly: warps, scenes, clip-slots, sends, video, nested clip
//! timelines and automation `Points` are parsed through to the loss
//! report, never silently skipped. `.logicx` and AAX are out of scope by
//! contract — never claimed.
//!
//! Determinism (T77): ZIP entries are written in sorted-name order with a
//! fixed timestamp; identical documents produce byte-identical archives.

use crate::document::*;
use crate::error::{ExchangeError, Result};
use crate::loss::{ExchangeDirection, LossReport};
use crate::registry::{DeviceRole, PluginDescriptor, PluginFormat};
use quick_xml::escape::escape;
use quick_xml::events::Event;
use quick_xml::Reader;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const TICKS: i64 = TICKS_PER_QUARTER;
const MAX_CONTAINER: u64 = 2 * 1024 * 1024 * 1024;
const MAX_XML: usize = 256 * 1024 * 1024;

/// Payload side-channel: container-relative path → bytes.
pub type FilesMap = BTreeMap<String, Vec<u8>>;

/// A safe container-relative path: non-empty, no leading separator, no
/// `..` segments, no backslashes/drive letters. Used by both the importer
/// (untrusted zip entries) and document validation.
pub fn is_safe_rel_path(p: &str) -> bool {
    if p.is_empty() || p.starts_with('/') || p.starts_with('\\') || p.contains('\\') {
        return false;
    }
    if p.contains(':') {
        return false;
    }
    !p.split('/').any(|seg| seg == ".." || seg.is_empty())
}

fn fixed_dt() -> zip::DateTime {
    zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Minimal XML DOM (events → tree). Depth/size bounded by MAX_XML.
// ---------------------------------------------------------------------------

struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn get(&self, k: &str) -> Option<&str> {
        self.attrs.get(k).map(|s| s.as_str())
    }
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    fn f(&self, k: &str) -> Result<f64> {
        self.get(k)
            .ok_or_else(|| ExchangeError::Malformed(format!("{} missing @{k}", self.name)))?
            .parse::<f64>()
            .map_err(|_| ExchangeError::Malformed(format!("{} @{} not f64", self.name, k)))
    }
    fn f_opt(&self, k: &str) -> Result<Option<f64>> {
        match self.get(k) {
            Some(v) => Ok(Some(v.parse::<f64>().map_err(|_| {
                ExchangeError::Malformed(format!("{} @{} not f64", self.name, k))
            })?)),
            None => Ok(None),
        }
    }
    fn b(&self, k: &str, default: bool) -> bool {
        match self.get(k) {
            Some(v) => v == "true" || v == "1",
            None => default,
        }
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
                    let a = a.map_err(|er| ExchangeError::Xml(er.to_string()))?;
                    let k = a.key.as_ref().to_string();
                    let v = a
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map(|c| c.into_owned())
                        .map_err(|er| ExchangeError::Xml(er.to_string()))?;
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
                    .ok_or_else(|| ExchangeError::Malformed("unbalanced close".into()))?;
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
            Err(e) => return Err(ExchangeError::Xml(e.to_string())),
        }
    }
    root.ok_or_else(|| ExchangeError::Malformed("no root element".into()))
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

fn beats(ticks: i64) -> String {
    let v = ticks as f64 / TICKS as f64;
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn attr(buf: &mut String, name: &str, val: &str) {
    buf.push(' ');
    buf.push_str(name);
    buf.push_str("=\"");
    buf.push_str(&escape(val));
    buf.push('"');
}

fn open(out: &mut String, ind: usize, name: &str, attrs: &str) {
    out.push_str(&"  ".repeat(ind));
    out.push('<');
    out.push_str(name);
    out.push_str(attrs);
    out.push_str(">\n");
}
fn empty(out: &mut String, ind: usize, name: &str, attrs: &str) {
    out.push_str(&"  ".repeat(ind));
    out.push('<');
    out.push_str(name);
    out.push_str(attrs);
    out.push_str("/>\n");
}
fn close(out: &mut String, ind: usize, name: &str) {
    out.push_str(&"  ".repeat(ind));
    out.push_str("</");
    out.push_str(name);
    out.push_str(">\n");
}

fn device_element(fmt: &PluginFormat) -> &'static str {
    match fmt {
        PluginFormat::Vst2 => "Vst2Plugin",
        PluginFormat::Vst3 => "Vst3Plugin",
        PluginFormat::Clap => "ClapPlugin",
        PluginFormat::Au => "AuPlugin",
        PluginFormat::Aax => "Device",
        PluginFormat::Builtin => "BuiltinDevice",
        PluginFormat::Other(_) => "Device",
    }
}

fn role_str(r: &Option<DeviceRole>) -> &'static str {
    match r {
        Some(DeviceRole::Instrument) => "instrument",
        Some(DeviceRole::NoteFx) => "noteFX",
        Some(DeviceRole::Analyzer) => "analyzer",
        _ => "audioFX",
    }
}

fn norm_vel(v: u8) -> String {
    format!("{:.17}", (v as f64 / 127.0).clamp(0.0, 1.0))
}

/// Serialize a document to (project.xml, metadata.xml).
/// `files` supplies payload bytes for every asset/state rel_path.
pub fn export_xml(
    doc: &ExchangeDocument,
    files: &FilesMap,
    loss: &mut LossReport,
) -> Result<(String, String)> {
    doc.validate()
        .map_err(|e| ExchangeError::Invalid(format!("document: {}", e.join("; "))))?;

    let mut x = String::with_capacity(64 * 1024);
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Project version=\"1.0\">\n");
    empty(&mut x, 1, "Application", " name=\"VOID\" version=\"0.1\"");

    open(&mut x, 1, "Transport", "");
    if let Some(tp) = doc.tempo_map.first() {
        empty(
            &mut x,
            2,
            "Tempo",
            &format!(" unit=\"bpm\" value=\"{}\"", tp.bpm),
        );
    }
    if let Some(ts) = doc.time_signatures.first() {
        empty(
            &mut x,
            2,
            "TimeSignature",
            &format!(
                " denominator=\"{}\" numerator=\"{}\"",
                ts.denominator, ts.numerator
            ),
        );
    }
    close(&mut x, 1, "Transport");

    open(&mut x, 1, "Structure", "");
    for tr in &doc.tracks {
        let cts = match tr.kind {
            TrackKind::AUDIO => "audio",
            TrackKind::MIDI => "notes",
            TrackKind::INSTRUMENT => "notes audio",
            TrackKind::BUS => "tracks",
        };
        let mut a = String::new();
        attr(&mut a, "contentType", cts);
        attr(&mut a, "loaded", "true");
        attr(&mut a, "name", &tr.name);
        if let Some(c) = &tr.color {
            attr(&mut a, "color", c);
        }
        attr(&mut a, "id", &tr.id);
        open(&mut x, 2, "Track", &a);

        let mut ca = String::new();
        attr(
            &mut ca,
            "role",
            if tr.kind == TrackKind::BUS {
                "submix"
            } else {
                "regular"
            },
        );
        attr(&mut ca, "audioChannels", "2");
        if tr.soloed {
            attr(&mut ca, "solo", "true");
        }
        open(&mut x, 3, "Channel", &ca);
        empty(&mut x, 4, "Mute", &format!(" value=\"{}\"", tr.muted));
        empty(
            &mut x,
            4,
            "Pan",
            &format!(" unit=\"normalized\" value=\"{}\"", tr.pan),
        );
        empty(
            &mut x,
            4,
            "Volume",
            &format!(" unit=\"linear\" value=\"{}\"", tr.gain_linear),
        );
        if !tr.plugins.is_empty() {
            open(&mut x, 4, "Devices", "");
            for (i, p) in tr.plugins.iter().enumerate() {
                let d = &p.descriptor;
                if matches!(d.format, PluginFormat::Aax) {
                    loss.drop(
                        format!("plugin {:?}", p.instance_id),
                        "deviceElement",
                        "AAX has no dawproject element; slot omitted".to_string(),
                    );
                    continue;
                }
                let mut da = String::new();
                attr(&mut da, "deviceID", &d.plugin_uid);
                attr(&mut da, "deviceName", &d.name);
                attr(&mut da, "deviceRole", role_str(&d.device_role));
                if let Some(v) = &d.vendor {
                    attr(&mut da, "deviceVendor", v);
                }
                if !matches!(d.format, PluginFormat::Builtin | PluginFormat::Other(_)) {
                    if let Some(ver) = &d.version {
                        attr(&mut da, "pluginVersion", ver);
                    }
                }
                attr(&mut da, "loaded", if p.present { "true" } else { "false" });
                attr(&mut da, "id", &format!("{}-dev{}", tr.id, i + 1));
                let el = device_element(&d.format);
                open(&mut x, 5, el, &da);
                empty(&mut x, 6, "Enabled", &format!(" value=\"{}\"", p.enabled));
                if !p.parameters.is_empty() {
                    open(&mut x, 6, "Parameters", "");
                    for prm in &p.parameters {
                        let mut pa = String::new();
                        if let Ok(pid) = prm.param_id.parse::<i64>() {
                            attr(&mut pa, "parameterID", &pid.to_string());
                        } else if !prm.param_id.is_empty() {
                            loss.drop(
                                format!("plugin {:?} param", p.instance_id),
                                "parameterID",
                                format!("non-numeric param id {:?} cannot map to dawproject parameterID", prm.param_id),
                            );
                        }
                        if let Some(n) = &prm.name {
                            attr(&mut pa, "name", n);
                        }
                        match &prm.value {
                            PluginParamValue::Real(v) => {
                                attr(&mut pa, "unit", "linear");
                                attr(&mut pa, "value", &v.to_string());
                                empty(&mut x, 7, "RealParameter", &pa);
                            }
                            PluginParamValue::Bool(v) => {
                                attr(&mut pa, "value", if *v { "true" } else { "false" });
                                empty(&mut x, 7, "BoolParameter", &pa);
                            }
                            PluginParamValue::Integer(v) => {
                                attr(&mut pa, "value", &v.to_string());
                                empty(&mut x, 7, "IntegerParameter", &pa);
                            }
                            PluginParamValue::Enum { index, labels } => {
                                attr(&mut pa, "value", &index.to_string());
                                attr(
                                    &mut pa,
                                    "count",
                                    &labels.as_ref().map(|l| l.len()).unwrap_or(0).to_string(),
                                );
                                if let Some(ls) = labels {
                                    attr(&mut pa, "labels", &ls.join(" "));
                                }
                                empty(&mut x, 7, "EnumParameter", &pa);
                            }
                            PluginParamValue::TimeSignature {
                                numerator,
                                denominator,
                            } => {
                                attr(&mut pa, "numerator", &numerator.to_string());
                                attr(&mut pa, "denominator", &denominator.to_string());
                                empty(&mut x, 7, "TimeSignatureParameter", &pa);
                            }
                        }
                    }
                    close(&mut x, 6, "Parameters");
                }
                if let Some(st) = &p.state {
                    match files.get(&st.rel_path) {
                        Some(blob) if sha256_hex(blob) == st.sha256 => {
                            let mut sa = String::new();
                            attr(&mut sa, "path", &st.rel_path);
                            empty(&mut x, 6, "State", &sa);
                        }
                        Some(_) => loss.drop(
                            format!("plugin {:?} state", p.instance_id),
                            "stateBlob",
                            format!("sha256 mismatch for {:?} — reference omitted", st.rel_path),
                        ),
                        None => loss.drop(
                            format!("plugin {:?} state", p.instance_id),
                            "stateBlob",
                            format!("payload {:?} absent — reference omitted", st.rel_path),
                        ),
                    }
                }
                close(&mut x, 5, el);
            }
            close(&mut x, 4, "Devices");
        }
        close(&mut x, 3, "Channel");
        close(&mut x, 2, "Track");
    }
    close(&mut x, 1, "Structure");

    open(&mut x, 1, "Arrangement", "");
    open(&mut x, 2, "Lanes", " timeUnit=\"beats\"");
    for tr in &doc.tracks {
        if tr.clips.is_empty() {
            continue;
        }
        let mut la = String::new();
        attr(&mut la, "track", &tr.id);
        attr(&mut la, "timeUnit", "beats");
        open(&mut x, 3, "Lanes", &la);
        open(&mut x, 4, "Clips", " timeUnit=\"beats\"");
        for c in &tr.clips {
            let mut ca = String::new();
            attr(&mut ca, "time", &beats(c.start_ticks));
            attr(&mut ca, "duration", &beats(c.length_ticks));
            attr(&mut ca, "contentTimeUnit", "beats");
            if c.offset_ticks != 0 {
                attr(&mut ca, "playStart", &beats(c.offset_ticks));
                attr(&mut ca, "playStop", &beats(c.offset_ticks + c.length_ticks));
            }
            if !c.name.is_empty() {
                attr(&mut ca, "name", &c.name);
            }
            if !c.enabled {
                attr(&mut ca, "enable", "false");
            }
            if c.fade_in_ticks.is_some() || c.fade_out_ticks.is_some() {
                attr(&mut ca, "fadeTimeUnit", "beats");
                if let Some(f) = c.fade_in_ticks {
                    attr(&mut ca, "fadeInTime", &beats(f));
                }
                if let Some(f) = c.fade_out_ticks {
                    attr(&mut ca, "fadeOutTime", &beats(f));
                }
            }
            match &c.content {
                ClipContent::Notes { notes } => {
                    open(&mut x, 5, "Clip", &ca);
                    open(&mut x, 6, "Notes", " timeUnit=\"beats\"");
                    let mut sorted: Vec<&Note> = notes.iter().collect();
                    sorted.sort_by(|a, b| (a.start_ticks, &a.id).cmp(&(b.start_ticks, &b.id)));
                    for n in sorted {
                        let mut na = String::new();
                        attr(&mut na, "time", &beats(n.start_ticks));
                        attr(&mut na, "duration", &beats(n.length_ticks));
                        attr(&mut na, "channel", &n.channel.to_string());
                        attr(&mut na, "key", &n.pitch.to_string());
                        attr(&mut na, "vel", &norm_vel(n.velocity));
                        if let Some(rv) = n.release_velocity {
                            attr(&mut na, "rel", &norm_vel(rv));
                        }
                        empty(&mut x, 7, "Note", &na);
                    }
                    close(&mut x, 6, "Notes");
                    close(&mut x, 5, "Clip");
                }
                ClipContent::Audio { asset_id } => {
                    let asset = doc.assets.iter().find(|a| a.asset_id == *asset_id);
                    match asset {
                        Some(a) if !a.missing && files.contains_key(&a.rel_path) => {
                            open(&mut x, 5, "Clip", &ca);
                            let mut aa = String::new();
                            attr(&mut aa, "timeUnit", "beats");
                            attr(&mut aa, "channels", &a.channels.unwrap_or(2).to_string());
                            attr(
                                &mut aa,
                                "sampleRate",
                                &a.sample_rate.unwrap_or(doc.sample_rate).to_string(),
                            );
                            attr(
                                &mut aa,
                                "duration",
                                &beats(a.duration_ticks.unwrap_or(c.length_ticks)),
                            );
                            open(&mut x, 6, "Audio", &aa);
                            let mut fa = String::new();
                            attr(&mut fa, "path", &a.rel_path);
                            empty(&mut x, 7, "File", &fa);
                            close(&mut x, 6, "Audio");
                            close(&mut x, 5, "Clip");
                        }
                        _ => loss.drop(
                            format!("clip {:?}", c.id),
                            "audio",
                            format!("asset {asset_id:?} has no payload — clip omitted"),
                        ),
                    }
                }
            }
        }
        close(&mut x, 4, "Clips");
        close(&mut x, 3, "Lanes");
    }
    close(&mut x, 2, "Lanes");

    if !doc.markers.is_empty() {
        open(&mut x, 2, "Markers", "");
        for m in &doc.markers {
            let mut ma = String::new();
            attr(&mut ma, "time", &beats(m.at_ticks));
            attr(&mut ma, "name", &m.name);
            if let Some(c) = &m.color {
                attr(&mut ma, "color", c);
            }
            empty(&mut x, 3, "Marker", &ma);
        }
        close(&mut x, 2, "Markers");
    }
    if doc.tempo_map.len() > 1 {
        open(&mut x, 2, "TempoAutomation", " unit=\"bpm\"");
        empty(&mut x, 3, "Target", "");
        for tp in &doc.tempo_map {
            empty(
                &mut x,
                3,
                "RealPoint",
                &format!(
                    " time=\"{}\" value=\"{}\" interpolation=\"hold\"",
                    beats(tp.at_ticks),
                    tp.bpm
                ),
            );
        }
        close(&mut x, 2, "TempoAutomation");
    }
    if doc.time_signatures.len() > 1 {
        open(&mut x, 2, "TimeSignatureAutomation", "");
        empty(&mut x, 3, "Target", "");
        for ts in &doc.time_signatures {
            empty(
                &mut x,
                3,
                "TimeSignaturePoint",
                &format!(
                    " time=\"{}\" numerator=\"{}\" denominator=\"{}\"",
                    beats(ts.at_ticks),
                    ts.numerator,
                    ts.denominator
                ),
            );
        }
        close(&mut x, 2, "TimeSignatureAutomation");
    }
    close(&mut x, 1, "Arrangement");
    x.push_str("</Project>\n");

    let mut m = String::new();
    m.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<MetaData>\n");
    m.push_str(&format!("  <Title>{}</Title>\n", escape(&doc.name)));
    if let Some(c) = &doc.comment {
        m.push_str(&format!("  <Comment>{}</Comment>\n", escape(c)));
    }
    m.push_str("</MetaData>\n");

    if doc.loop_range.is_some() {
        loss.drop(
            "document".to_string(),
            "loopRange",
            "dawproject schema has no transport loop element".to_string(),
        );
    }
    Ok((x, m))
}

/// Serialize + pack into .dawproject ZIP bytes.
pub fn export_dawproject(
    doc: &ExchangeDocument,
    files: &FilesMap,
) -> Result<(Vec<u8>, LossReport)> {
    let mut loss = LossReport::new(ExchangeDirection::Export);
    let (project_xml, metadata_xml) = export_xml(doc, files, &mut loss)?;
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(fixed_dt());
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    let mut names: Vec<String> = vec!["metadata.xml".into(), "project.xml".into()];
    names.extend(files.keys().cloned());
    names.sort();
    for name in names {
        let data: &[u8] = match name.as_str() {
            "metadata.xml" => metadata_xml.as_bytes(),
            "project.xml" => project_xml.as_bytes(),
            _ => files.get(&name).map(|v| v.as_slice()).unwrap_or(&[]),
        };
        zw.start_file(name.clone(), opts)
            .map_err(ExchangeError::Zip)?;
        zw.write_all(data).map_err(ExchangeError::Io)?;
    }
    let cur = zw.finish().map_err(ExchangeError::Zip)?;
    Ok((cur.into_inner(), loss.sorted()))
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

fn beats_to_ticks(beats: f64, unit: &str, bpm: f64, at: &str, loss: &mut LossReport) -> i64 {
    let b = if unit == "seconds" {
        loss.approximate(
            at.to_string(),
            "timeUnit".to_string(),
            format!("seconds value {beats} converted at {bpm}bpm"),
        );
        beats * bpm / 60.0
    } else {
        beats
    };
    let ticks = b * TICKS as f64;
    let rounded = ticks.round();
    if (ticks - rounded).abs() > 1e-6 {
        loss.approximate(
            at.to_string(),
            "ticks".to_string(),
            format!("{b} beats → {ticks} ticks, rounded to {rounded}"),
        );
    }
    rounded.max(0.0) as i64
}

fn fmt_of(elem: &str) -> PluginFormat {
    match elem {
        "Vst2Plugin" => PluginFormat::Vst2,
        "Vst3Plugin" => PluginFormat::Vst3,
        "ClapPlugin" => PluginFormat::Clap,
        "AuPlugin" => PluginFormat::Au,
        "BuiltinDevice" | "Equalizer" | "Compressor" | "NoiseGate" | "Limiter" => {
            PluginFormat::Builtin
        }
        other => PluginFormat::Other(other.to_string()),
    }
}

fn role_of(s: &str) -> DeviceRole {
    match s {
        "instrument" => DeviceRole::Instrument,
        "noteFX" => DeviceRole::NoteFx,
        "analyzer" => DeviceRole::Analyzer,
        _ => DeviceRole::AudioFx,
    }
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    crate::preserve::encode_hex(&h.finalize())
}

fn parse_device(
    d: &Node,
    idx: usize,
    track_id: &str,
    files: &FilesMap,
    loss: &mut LossReport,
) -> PluginSlot {
    let mut slot = PluginSlot {
        instance_id: d
            .get("id")
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{track_id}-dev{idx}")),
        slot: idx as i32,
        enabled: true,
        present: d.b("loaded", true),
        descriptor: PluginDescriptor {
            format: fmt_of(&d.name),
            plugin_uid: d.get("deviceID").unwrap_or("").to_string(),
            name: d.get("deviceName").unwrap_or("").to_string(),
            vendor: d.get("deviceVendor").map(|s| s.to_string()),
            version: d.get("pluginVersion").map(|s| s.to_string()),
            device_role: Some(role_of(d.get("deviceRole").unwrap_or("audioFX"))),
            arch: Vec::new(),
            state_format_version: None,
        },
        parameters: Vec::new(),
        state: None,
    };
    if let Some(en) = d.child("Enabled") {
        slot.enabled = en.b("value", true);
    }
    if let Some(params) = d.child("Parameters") {
        for p in &params.children {
            let param_id = p.get("parameterID").unwrap_or("").to_string();
            let name = p.get("name").map(|s| s.to_string());
            let value = match p.name.as_str() {
                "RealParameter" => PluginParamValue::Real(p.f("value").unwrap_or(0.0)),
                "BoolParameter" => PluginParamValue::Bool(p.b("value", false)),
                "IntegerParameter" => PluginParamValue::Integer(
                    p.get("value").and_then(|s| s.parse().ok()).unwrap_or(0),
                ),
                "EnumParameter" => PluginParamValue::Enum {
                    index: p.get("value").and_then(|s| s.parse().ok()).unwrap_or(0),
                    labels: p
                        .get("labels")
                        .map(|s| s.split(' ').map(|x| x.to_string()).collect()),
                },
                "TimeSignatureParameter" => PluginParamValue::TimeSignature {
                    numerator: p.get("numerator").and_then(|s| s.parse().ok()).unwrap_or(4),
                    denominator: p
                        .get("denominator")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(4),
                },
                _ => continue,
            };
            slot.parameters.push(PluginParam {
                param_id,
                name,
                value,
            });
        }
    }
    if let Some(st) = d.child("State") {
        let path = st.get("path").unwrap_or("").to_string();
        match files.get(&path) {
            Some(blob) => {
                slot.state = Some(PluginStateRef {
                    rel_path: path,
                    sha256: sha256_hex(blob),
                    bytes: blob.len().to_string(),
                    format_version: slot.descriptor.state_format_version.clone(),
                });
            }
            None => {
                loss.drop(
                    format!("plugin {:?}", slot.instance_id),
                    "state",
                    format!("State path {path:?} not present in container"),
                );
            }
        }
    }
    slot
}

fn parse_clip(
    ce: &Node,
    track_id: &str,
    ordinal: usize,
    bpm0: f64,
    files: &FilesMap,
    assets: &mut Vec<AssetRef>,
    loss: &mut LossReport,
) -> Result<Clip> {
    let clip_id = ce
        .get("id")
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::plan::deterministic_id(track_id, "clip", ordinal));
    let start = beats_to_ticks(
        ce.f("time")?,
        ce.get("timeUnit").unwrap_or("beats"),
        bpm0,
        &format!("clip {clip_id:?}"),
        loss,
    );
    let mut len = match ce.f_opt("duration")? {
        Some(d) => beats_to_ticks(
            d,
            "beats",
            bpm0,
            &format!("clip {clip_id:?} duration"),
            loss,
        ),
        None => 0,
    };
    if let (Some(ls), Some(le)) = (ce.f_opt("loopStart")?, ce.f_opt("loopEnd")?) {
        if le > ls {
            loss.drop(
                format!("clip {clip_id:?}"),
                "loop",
                "clip loopStart/loopEnd not representable".to_string(),
            );
        }
    }
    if ce.get("reference").is_some() {
        loss.approximate(
            format!("clip {clip_id:?}"),
            "reference",
            "referenced clip content inlined at import".to_string(),
        );
    }
    let fade_unit = ce.get("fadeTimeUnit").unwrap_or("beats");
    let fade_in = ce.f_opt("fadeInTime")?.map(|v| {
        beats_to_ticks(
            v,
            fade_unit,
            bpm0,
            &format!("clip {clip_id:?} fadeIn"),
            loss,
        )
    });
    let fade_out = ce.f_opt("fadeOutTime")?.map(|v| {
        beats_to_ticks(
            v,
            fade_unit,
            bpm0,
            &format!("clip {clip_id:?} fadeOut"),
            loss,
        )
    });
    let offset = match ce.f_opt("playStart")? {
        Some(ps) => beats_to_ticks(
            ps,
            "beats",
            bpm0,
            &format!("clip {clip_id:?} playStart"),
            loss,
        ),
        None => 0,
    };

    let mut content: Option<ClipContent> = None;
    for child in &ce.children {
        match child.name.as_str() {
            "Notes" => {
                let mut ns = Vec::new();
                for (i, ne) in child.children_named("Note").enumerate() {
                    let key: u8 = ne.get("key").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let chan: u8 = ne.get("channel").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let vel_f: f64 = ne
                        .get("vel")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(100.0 / 127.0);
                    let vel = (vel_f * 127.0).round().clamp(1.0, 127.0) as u8;
                    let rel = ne
                        .get("rel")
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|v| (v * 127.0).round().clamp(0.0, 127.0) as u8);
                    ns.push(Note {
                        id: crate::plan::deterministic_id(&clip_id, "note", i),
                        pitch: key,
                        velocity: vel,
                        release_velocity: rel,
                        channel: chan.min(15),
                        start_ticks: beats_to_ticks(
                            ne.f("time")?,
                            child.get("timeUnit").unwrap_or("beats"),
                            bpm0,
                            &format!("note {i} in {clip_id:?}"),
                            loss,
                        ),
                        length_ticks: beats_to_ticks(
                            ne.f("duration")?,
                            child.get("timeUnit").unwrap_or("beats"),
                            bpm0,
                            &format!("note {i} in {clip_id:?} duration"),
                            loss,
                        )
                        .max(1),
                    });
                }
                content = Some(ClipContent::Notes { notes: ns });
            }
            "Audio" => {
                let file = child.child("File");
                let path = file.and_then(|f| f.get("path")).map(|s| s.to_string());
                let external = file.map(|f| f.b("external", false)).unwrap_or(false);
                let asset_id = format!("{clip_id}-asset");
                let payload = path.as_ref().and_then(|p| files.get(p));
                let (rel_path, sha, bytes, missing) = match (path.clone(), payload, external) {
                    (Some(p), Some(b), false) => (p, sha256_hex(b), b.len().to_string(), false),
                    (Some(p), _, ext) => {
                        let why = if ext {
                            "external file reference"
                        } else {
                            "payload absent"
                        };
                        loss.drop(
                            format!("clip {clip_id:?}"),
                            "audioPayload",
                            format!("{why} for {p:?} — clip shell preserved"),
                        );
                        (p, String::new(), "0".to_string(), true)
                    }
                    (None, _, _) => {
                        loss.drop(
                            format!("clip {clip_id:?}"),
                            "audioPayload",
                            "Audio element without File reference".to_string(),
                        );
                        (
                            format!("media/missing-{ordinal}"),
                            String::new(),
                            "0".to_string(),
                            true,
                        )
                    }
                };
                let dur_ticks = child
                    .get("duration")
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|d| {
                        beats_to_ticks(
                            d,
                            child.get("timeUnit").unwrap_or("beats"),
                            bpm0,
                            &format!("audio in {clip_id:?}"),
                            loss,
                        )
                    });
                assets.push(AssetRef {
                    asset_id: asset_id.clone(),
                    rel_path,
                    media_type: "audio/*".to_string(),
                    sha256: sha,
                    bytes,
                    channels: child.get("channels").and_then(|s| s.parse().ok()),
                    sample_rate: child.get("sampleRate").and_then(|s| s.parse().ok()),
                    duration_ticks: dur_ticks,
                    missing,
                });
                content = Some(ClipContent::Audio { asset_id });
            }
            "Warps" | "Points" | "Lanes" | "Clips" | "ClipSlot" | "markers" | "Timeline"
            | "Video" => {}
            other => {
                loss.drop(
                    format!("clip {clip_id:?}"),
                    format!("child:{other}"),
                    "clip child timeline not representable".to_string(),
                );
            }
        }
    }
    // second pass for the not-yet-captured child types (warps etc.)
    for child in &ce.children {
        match child.name.as_str() {
            "Warps" => loss.drop(
                format!("clip {clip_id:?}"),
                "warps",
                "audio time-warp points not representable".to_string(),
            ),
            "Points" => loss.drop(
                format!("clip {clip_id:?}"),
                "automation",
                "clip automation points not representable".to_string(),
            ),
            "Lanes" | "Clips" | "ClipSlot" => loss.drop(
                format!("clip {clip_id:?}"),
                "nestedTimeline",
                "nested lanes/clips inside a clip not representable".to_string(),
            ),
            "Video" => loss.drop(
                format!("clip {clip_id:?}"),
                "video",
                "video content not representable".to_string(),
            ),
            "markers" => loss.drop(
                format!("clip {clip_id:?}"),
                "clipMarkers",
                "clip-level markers not representable".to_string(),
            ),
            _ => {}
        }
    }
    let content = content.unwrap_or_else(|| {
        loss.drop(
            format!("clip {clip_id:?}"),
            "content",
            "clip carried no Notes/Audio content".to_string(),
        );
        ClipContent::Notes { notes: Vec::new() }
    });
    if len == 0 {
        len = match &content {
            ClipContent::Notes { notes } => notes
                .iter()
                .map(|n| n.start_ticks + n.length_ticks)
                .max()
                .unwrap_or(1)
                .max(1),
            ClipContent::Audio { asset_id } => assets
                .iter()
                .find(|a| a.asset_id == *asset_id)
                .and_then(|a| a.duration_ticks)
                .unwrap_or(1)
                .max(1),
        };
    }
    Ok(Clip {
        id: clip_id,
        name: ce.get("name").unwrap_or("").to_string(),
        start_ticks: start,
        length_ticks: len,
        offset_ticks: offset,
        enabled: ce.b("enable", true),
        fade_in_ticks: fade_in,
        fade_out_ticks: fade_out,
        content,
    })
}

/// Parse a .dawproject ZIP container → (document, payload files, loss).
pub fn import_dawproject(
    bytes: &[u8],
    sample_rate: u32,
) -> Result<(ExchangeDocument, FilesMap, LossReport)> {
    let mut loss = LossReport::new(ExchangeDirection::Import);
    if bytes.len() as u64 > MAX_CONTAINER {
        return Err(ExchangeError::TooLarge("container > 2GiB".into()));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(ExchangeError::Zip)?;

    let mut files: FilesMap = BTreeMap::new();
    let mut project_xml = None;
    let mut metadata_xml = None;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(ExchangeError::Zip)?;
        let name = f.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        if name.starts_with('/') || name.split('/').any(|s| s == "..") || name.contains('\\') {
            return Err(ExchangeError::UnsafePath(name));
        }
        let mut buf = Vec::with_capacity(f.size().min(16 << 20) as usize);
        f.read_to_end(&mut buf).map_err(ExchangeError::Io)?;
        match name.as_str() {
            "project.xml" => {
                if buf.len() > MAX_XML {
                    return Err(ExchangeError::TooLarge("project.xml > 256MiB".into()));
                }
                project_xml = Some(
                    String::from_utf8(buf)
                        .map_err(|_| ExchangeError::Malformed("project.xml not utf-8".into()))?,
                );
            }
            "metadata.xml" => {
                metadata_xml = Some(
                    String::from_utf8(buf)
                        .map_err(|_| ExchangeError::Malformed("metadata.xml not utf-8".into()))?,
                );
            }
            _ => {
                files.insert(name, buf);
            }
        }
    }
    let project_xml = project_xml.ok_or(ExchangeError::NotFound("project.xml".into()))?;
    let root = parse_dom(&project_xml)?;
    if root.name != "Project" {
        return Err(ExchangeError::Malformed(format!(
            "root element {:?} (expected Project)",
            root.name
        )));
    }

    let mut doc = ExchangeDocument::new("imported", sample_rate);

    // --- Transport ---
    let mut tempo_map: Vec<TempoPoint> = Vec::new();
    let mut time_sigs: Vec<TimeSignaturePoint> = Vec::new();
    if let Some(t) = root.child("Transport") {
        if let Some(tp) = t.child("Tempo") {
            if let Ok(bpm) = tp.f("value") {
                tempo_map.push(TempoPoint { at_ticks: 0, bpm });
            }
        }
        if let Some(ts) = t.child("TimeSignature") {
            let n = ts.get("numerator").and_then(|s| s.parse().ok());
            let d = ts.get("denominator").and_then(|s| s.parse().ok());
            if let (Some(n), Some(d)) = (n, d) {
                time_sigs.push(TimeSignaturePoint {
                    at_ticks: 0,
                    numerator: n,
                    denominator: d,
                });
            }
        }
    }
    let bpm0 = tempo_map.first().map(|t| t.bpm).unwrap_or(120.0);

    // --- Arrangement: lanes, markers, tempo/sig automation ---
    // Collect clip lanes: Clips elems anywhere under Arrangement carry a
    // `track` IDREF on themselves or an ancestor Lanes.
    let mut lanes: Vec<(String, &Node)> = Vec::new(); // (track_id, Clips node)
    if let Some(arr) = root.child("Arrangement") {
        fn walk_lanes<'a>(
            node: &'a Node,
            track_ctx: &str,
            lanes: &mut Vec<(String, &'a Node)>,
            loss: &mut LossReport,
        ) {
            for c in &node.children {
                match c.name.as_str() {
                    "Clips" => {
                        let tid = c.get("track").unwrap_or(track_ctx).to_string();
                        lanes.push((tid, c));
                    }
                    "Lanes" | "Tracks" | "Timeline" => {
                        let tid = c.get("track").unwrap_or(track_ctx).to_string();
                        walk_lanes(c, &tid, lanes, loss);
                    }
                    "ClipSlot" => loss.drop(
                        "arrangement".to_string(),
                        "clipSlots".to_string(),
                        "ClipSlot (session-view) not representable".to_string(),
                    ),
                    "Warps" | "Points" | "Notes" | "Audio" | "Video" | "markers" => loss.drop(
                        "arrangement".to_string(),
                        format!("orphan:{}", c.name),
                        "arrangement-level timeline without clip wrapper".to_string(),
                    ),
                    _ => {}
                }
            }
        }
        if let Some(lanes_root) = arr.child("Lanes") {
            walk_lanes(lanes_root, "", &mut lanes, &mut loss);
        }
        if let Some(mks) = arr.child("Markers") {
            for m in mks.children_named("Marker") {
                doc.markers.push(Marker {
                    at_ticks: beats_to_ticks(m.f("time")?, "beats", bpm0, "marker", &mut loss),
                    name: m.get("name").unwrap_or("").to_string(),
                    color: m.get("color").map(|s| s.to_string()),
                });
            }
        }
        if let Some(ta) = arr.child("TempoAutomation") {
            for p in ta.children_named("RealPoint") {
                if let (Ok(t), Ok(v)) = (p.f("time"), p.f("value")) {
                    tempo_map.push(TempoPoint {
                        at_ticks: beats_to_ticks(t, "beats", bpm0, "tempoAutomation", &mut loss),
                        bpm: v,
                    });
                }
            }
        }
        if let Some(sa) = arr.child("TimeSignatureAutomation") {
            for p in sa.children_named("TimeSignaturePoint") {
                let t = p.f("time").unwrap_or(0.0);
                let n = p.get("numerator").and_then(|s| s.parse().ok());
                let d = p.get("denominator").and_then(|s| s.parse().ok());
                if let (Some(n), Some(d)) = (n, d) {
                    time_sigs.push(TimeSignaturePoint {
                        at_ticks: beats_to_ticks(
                            t,
                            "beats",
                            bpm0,
                            "timeSignatureAutomation",
                            &mut loss,
                        ),
                        numerator: n,
                        denominator: d,
                    });
                }
            }
        }
    }
    tempo_map.sort_by_key(|p| p.at_ticks);
    tempo_map.dedup_by(|a, b| a.at_ticks == b.at_ticks);
    time_sigs.sort_by_key(|p| p.at_ticks);
    time_sigs.dedup_by(|a, b| a.at_ticks == b.at_ticks);
    doc.tempo_map = tempo_map;
    doc.time_signatures = time_sigs;

    if root.child("Scenes").is_some() {
        loss.drop(
            "structure".to_string(),
            "scenes",
            "Scenes section not representable".to_string(),
        );
    }

    // --- Structure: tracks + channels + devices ---
    let mut track_list: Vec<Track> = Vec::new();
    let mut assets: Vec<AssetRef> = Vec::new();
    if let Some(structure) = root.child("Structure") {
        for te in structure.children_named("Track") {
            let id = te
                .get("id")
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("track-{}", track_list.len()));
            if !te.b("loaded", true) {
                loss.approximate(
                    format!("track {id:?}"),
                    "loaded",
                    "track loaded=\"false\" — imported anyway".to_string(),
                );
            }
            let content = te.get("contentType").unwrap_or("");
            let kind = if content.contains("notes") && content.contains("audio") {
                TrackKind::INSTRUMENT
            } else if content.contains("notes") {
                TrackKind::MIDI
            } else if content.contains("audio") {
                TrackKind::AUDIO
            } else if content.contains("tracks") {
                TrackKind::BUS
            } else {
                TrackKind::MIDI
            };
            let mut muted = false;
            let mut pan = 0.0;
            let mut gain = 1.0;
            let mut solo = false;
            let mut plugins = Vec::new();
            if let Some(ch) = te.child("Channel") {
                solo = ch.b("solo", false);
                let role = ch.get("role").unwrap_or("regular");
                if role != "regular" && role != "submix" {
                    loss.drop(
                        format!("track {id:?}"),
                        "role",
                        format!("channel role {role:?} not representable"),
                    );
                }
                if let Some(m) = ch.child("Mute") {
                    muted = m.b("value", false);
                }
                if let Some(p) = ch.child("Pan") {
                    pan = p.f("value").unwrap_or(0.0).clamp(-1.0, 1.0);
                }
                if let Some(v) = ch.child("Volume") {
                    let val = v.f("value").unwrap_or(1.0);
                    let unit = v.get("unit").unwrap_or("linear");
                    if unit == "decibel" {
                        gain = 10f64.powf(val / 20.0);
                        loss.approximate(
                            format!("track {id:?}"),
                            "volumeUnit",
                            "dB volume converted to linear gain".to_string(),
                        );
                    } else {
                        gain = val;
                    }
                }
                if ch.child("Sends").is_some() {
                    loss.drop(
                        format!("track {id:?}"),
                        "sends",
                        "channel sends not representable".to_string(),
                    );
                }
                if let Some(devs) = ch.child("Devices") {
                    for (di, d) in devs.children.iter().enumerate() {
                        plugins.push(parse_device(d, di, &id, &files, &mut loss));
                    }
                }
            }
            // nested child tracks flatten — hierarchy lost, recorded once
            let children: Vec<&Node> = te.children_named("Track").collect();
            if !children.is_empty() {
                loss.approximate(
                    format!("track {id:?}"),
                    "hierarchy",
                    format!(
                        "{} nested child track(s) flattened to top level",
                        children.len()
                    ),
                );
            }
            track_list.push(Track {
                id,
                name: te.get("name").unwrap_or("").to_string(),
                color: te.get("color").map(|s| s.to_string()),
                kind,
                gain_linear: gain,
                pan,
                muted,
                soloed: solo,
                clips: Vec::new(),
                plugins,
            });
        }
    }

    // --- Attach clips to tracks ---
    for (tid, clips_node) in &lanes {
        for (ci, ce) in clips_node.children_named("Clip").enumerate() {
            let clip = parse_clip(ce, tid, ci, bpm0, &files, &mut assets, &mut loss)?;
            if let Some(tr) = track_list.iter_mut().find(|t| &t.id == tid) {
                tr.clips.push(clip);
            } else {
                let new_id = format!("orphan-{tid}");
                loss.approximate(
                    format!("clip {:?}", clip.id),
                    "track",
                    format!(
                        "clip references unknown track {tid:?}; attached to placeholder {new_id:?}"
                    ),
                );
                track_list.push(Track {
                    id: new_id,
                    name: format!("(orphan clips of {tid})"),
                    color: None,
                    kind: TrackKind::MIDI,
                    gain_linear: 1.0,
                    pan: 0.0,
                    muted: false,
                    soloed: false,
                    clips: vec![clip],
                    plugins: Vec::new(),
                });
            }
        }
    }
    doc.tracks = track_list;
    doc.assets = assets;

    // --- metadata.xml ---
    if let Some(mx) = &metadata_xml {
        if let Ok(mroot) = parse_dom(mx) {
            if let Some(t) = mroot.child("Title") {
                let v = t.text.trim();
                if !v.is_empty() {
                    doc.name = v.to_string();
                }
            }
            if let Some(c) = mroot.child("Comment") {
                let v = c.text.trim();
                if !v.is_empty() {
                    doc.comment = Some(v.to_string());
                }
            }
        }
    }

    Ok((doc, files, loss.sorted()))
}
