//! Declarative scene-generation spec (job `parameters`) + the worker's
//! emitted scene document (`scene.json`). Generated visuals are DATA —
//! bounded params, declared layers, audio bindings — never code, paths
//! or ambient capabilities (CONTRACTS.md §6/§8, T84's "forbidden scene
//! actions").

use crate::error::{Result, VisFxError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Schema tag carried on both envelopes.
pub const SPEC_TAG: &str = "void-scene-gen-spec";
pub const DOC_TAG: &str = "void-scene-gen";
/// Artifact name the worker must publish inside its staging dir.
pub const DOC_FILE: &str = "scene.json";

/// Field names a spec/doc may never carry — ambient, filesystem,
/// network or code-execution reach. Checked shallowly at the envelope
/// level AND recursively inside params/actions.
pub const FORBIDDEN_FIELDS: &[&str] = &[
    "url",
    "uri",
    "path",
    "file",
    "filepath",
    "command",
    "cmd",
    "exec",
    "shell",
    "argv",
    "network",
    "http",
    "https",
    "socket",
    "env",
    "process",
    "module",
    "eval",
    "import",
    "require",
    "wgsl_module_path",
    "fetch",
    "download",
    "upload",
];

/// Allowed declarative action verbs on a scene doc. Anything else is a
/// forbidden scene action (T84): the doc can arrange its own declared
/// layers and bindings and nothing else.
pub const ALLOWED_ACTIONS: &[&str] = &[
    "add_layer",
    "remove_layer",
    "set_transform",
    "set_blend",
    "set_opacity",
    "set_transition",
    "set_trim",
    "bind_audio_feature",
    "set_anchor",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneGenKind {
    /// Procedural/generator-layer material (preset or generated shader).
    GeneratorLoop,
    /// A single generated still/texture asset for a media layer.
    StillImage,
    /// A generated shader preset candidate (validated on collect).
    ShaderPreset,
    /// Multi-layer declarative scene patch.
    ScenePatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureSource {
    Rms,
    Peak,
    Onset,
    Band,
}

/// Normalized (0..1) audio-feature → scene-parameter binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioBinding {
    pub source: FeatureSource,
    /// Only meaningful when source == Band.
    #[serde(default)]
    pub band: u8,
    /// Scene param name the feature drives.
    pub target: String,
    /// Mapper kind: linear | sqrt | db_floor.
    pub mapper: String,
    #[serde(default = "one")]
    pub gain: f32,
    /// Per-frame attack/release smoothing coefficients (0..1; 0 = off).
    #[serde(default)]
    pub attack: f32,
    #[serde(default)]
    pub release: f32,
}

fn one() -> f32 {
    1.0
}

/// The declarative spec sent to the worker inside `JobSpec.parameters`.
/// Everything is bounded and finite; ids are minted by the service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneGenSpec {
    pub tag: String,
    pub kind: SceneGenKind,
    /// Decimal i64 string — musical duration of the requested material.
    pub duration_ticks: String,
    /// Decimal u64 string — generation seed (determinism evidence).
    pub seed: String,
    pub width: u32,
    pub height: u32,
    /// Rational fps (CONTRACTS §7).
    pub fps_num: u32,
    pub fps_den: u32,
    /// Bounded scalar params (e.g. hue, density). Names are ident-safe.
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub audio_bindings: Vec<AudioBinding>,
    /// Optional shader-body candidate (generator body only — validated
    /// through the preset registry on collect, never trusted here).
    #[serde(default)]
    pub shader_body: Option<String>,
    /// Free-text intent (displayed; never interpreted).
    #[serde(default)]
    pub description: Option<String>,
}

impl SceneGenSpec {
    pub fn new(kind: SceneGenKind, duration_ticks: i64, seed: u64) -> Self {
        Self {
            tag: SPEC_TAG.into(),
            kind,
            duration_ticks: duration_ticks.to_string(),
            seed: seed.to_string(),
            width: 1920,
            height: 1080,
            fps_num: 60,
            fps_den: 1,
            params: serde_json::Map::new(),
            audio_bindings: Vec::new(),
            shader_body: None,
            description: None,
        }
    }

    /// Structural + safety validation. Returns Err on forbidden fields,
    /// non-finite numbers, oversize content or bad shapes.
    pub fn validate(&self) -> Result<()> {
        if self.tag != SPEC_TAG {
            return Err(VisFxError::InvalidSpec(format!("tag {:?}", self.tag)));
        }
        self.duration_ticks
            .parse::<i64>()
            .ok()
            .filter(|&d| d > 0)
            .ok_or_else(|| VisFxError::InvalidSpec("durationTicks must be >0 i64".into()))?;
        self.seed
            .parse::<u64>()
            .map_err(|_| VisFxError::InvalidSpec("seed must be decimal u64".into()))?;
        if self.width == 0 || self.height == 0 || self.width > 8192 || self.height > 8192 {
            return Err(VisFxError::InvalidSpec(
                "width/height out of 1..8192".into(),
            ));
        }
        if self.fps_num == 0 || self.fps_den == 0 {
            return Err(VisFxError::InvalidSpec(
                "fps rational must be non-zero".into(),
            ));
        }
        if self.params.len() > 64 {
            return Err(VisFxError::InvalidSpec("params >64 entries".into()));
        }
        check_forbidden_value(&serde_json::Value::Object(self.params.clone()), "params")?;
        for (k, v) in &self.params {
            if !is_ident(k) {
                return Err(VisFxError::InvalidSpec(format!("param name {k:?}")));
            }
            check_param_value(k, v)?;
        }
        if self.audio_bindings.len() > 32 {
            return Err(VisFxError::InvalidSpec("audioBindings >32".into()));
        }
        for b in &self.audio_bindings {
            if !is_ident(&b.target) {
                return Err(VisFxError::InvalidSpec(format!(
                    "binding target {:?}",
                    b.target
                )));
            }
            if !matches!(b.mapper.as_str(), "linear" | "sqrt" | "db_floor") {
                return Err(VisFxError::InvalidSpec(format!("mapper {:?}", b.mapper)));
            }
            for (name, f) in [
                ("gain", b.gain),
                ("attack", b.attack),
                ("release", b.release),
            ] {
                if !f.is_finite() {
                    return Err(VisFxError::InvalidSpec(format!(
                        "binding {name} not finite"
                    )));
                }
            }
            if !(0.0..=1.0).contains(&b.attack) || !(0.0..=1.0).contains(&b.release) {
                return Err(VisFxError::InvalidSpec(
                    "attack/release must be 0..1".into(),
                ));
            }
        }
        if let Some(body) = &self.shader_body {
            if body.len() > 64 * 1024 {
                return Err(VisFxError::InvalidSpec("shaderBody >64KiB".into()));
            }
        }
        if let Some(d) = &self.description {
            if d.len() > 1024 {
                return Err(VisFxError::InvalidSpec("description >1024".into()));
            }
            check_forbidden_text(d)?;
        }
        Ok(())
    }

    /// Canonical sha256 (context-hash analogue — binds the job to these
    /// exact params; sorted JSON keys via serde Value).
    pub fn sha256(&self) -> String {
        let canon = serde_json::to_vec(self).unwrap_or_default();
        hex(&Sha256::digest(&canon))
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn check_param_value(k: &str, v: &serde_json::Value) -> Result<()> {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if !f.is_finite() {
                    return Err(VisFxError::InvalidSpec(format!("param {k:?} NaN/Inf")));
                }
            }
            Ok(())
        }
        serde_json::Value::Bool(_) => Ok(()),
        serde_json::Value::String(s) if s.len() <= 256 => Ok(()),
        _ => Err(VisFxError::InvalidSpec(format!(
            "param {k:?} must be finite number/bool/string<=256"
        ))),
    }
}

/// Recursively reject forbidden field names at any depth of a JSON
/// value — the spec/doc cannot smuggle ambient capability inside a
/// nested param object.
pub fn check_forbidden_value(v: &serde_json::Value, where_: &str) -> Result<()> {
    match v {
        serde_json::Value::Object(m) => {
            for (k, val) in m {
                if FORBIDDEN_FIELDS.contains(&k.to_ascii_lowercase().as_str()) {
                    return Err(VisFxError::Forbidden(format!("{where_}.{k}")));
                }
                check_forbidden_value(val, &format!("{where_}.{k}"))?;
            }
            Ok(())
        }
        serde_json::Value::Array(a) => {
            for (i, val) in a.iter().enumerate() {
                check_forbidden_value(val, &format!("{where_}[{i}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Descriptions are data, not instructions — reject obvious capability
/// smuggling ("open http://…", "exec …") while allowing prose.
fn check_forbidden_text(d: &str) -> Result<()> {
    let l = d.to_ascii_lowercase();
    for pat in ["http://", "https://", "file://", "exec(", "eval("] {
        if l.contains(pat) {
            return Err(VisFxError::Forbidden(format!(
                "description contains {pat:?}"
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SceneDoc — the worker's output, validated on collect
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocTransform {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "one")]
    pub scale_x: f32,
    #[serde(default = "one")]
    pub scale_y: f32,
    #[serde(default)]
    pub rotation_rad: f32,
    #[serde(default = "one")]
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocGenerator {
    /// Builtin preset id (void-visual `generator_body` registry) OR
    /// absent when `shaderBody` carries a generated body.
    #[serde(default)]
    pub preset: Option<String>,
    /// Generated WGSL generator body — validated through the preset
    /// registry (naga parse + validate + static cost budget).
    #[serde(default)]
    pub shader_body: Option<String>,
    #[serde(default)]
    pub seed: String,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocMedia {
    /// Artifact name the worker declared (staging-relative file name).
    /// Verified against the job's actual artifacts on collect.
    pub artifact: String,
    pub media_kind: String,
    /// Decimal i64 string; 0 = still/looping.
    pub duration_ticks: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocLayer {
    pub name: String,
    /// preview | program
    pub channel: String,
    /// generator | media
    pub kind: String,
    #[serde(default)]
    pub blend: Option<String>,
    #[serde(default)]
    pub transform: Option<DocTransform>,
    #[serde(default)]
    pub generator: Option<DocGenerator>,
    #[serde(default)]
    pub media: Option<DocMedia>,
    /// Trim window in ticks (decimal strings; out -1 = unbounded).
    #[serde(default)]
    pub in_ticks: Option<String>,
    #[serde(default)]
    pub out_ticks: Option<String>,
}

/// Declarative scene action — closed verb set; anything else is a
/// forbidden scene action (T84).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocAction {
    pub action: String,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub value: serde_json::Value,
}

/// `scene.json` — what a visual-generation worker emits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneDoc {
    pub tag: String,
    pub generator_id: String,
    pub generator_version: String,
    #[serde(default)]
    pub model_id: Option<String>,
    /// Decimal u64 seed the worker actually used.
    pub seed: String,
    #[serde(default)]
    pub layers: Vec<DocLayer>,
    #[serde(default)]
    pub actions: Vec<DocAction>,
    /// Measured facts only — echoed into provenance verbatim.
    #[serde(default)]
    pub analysis: serde_json::Value,
}

pub fn parse_scene_doc(bytes: &[u8]) -> Result<SceneDoc> {
    if bytes.len() > 1024 * 1024 {
        return Err(VisFxError::InvalidDocument("scene.json >1MiB".into()));
    }
    let doc: SceneDoc = serde_json::from_slice(bytes)?;
    validate_doc_shape(&doc)?;
    Ok(doc)
}

/// Shape/verb validation — compile checks on shader bodies are the
/// registry's job (service calls it after this).
fn validate_doc_shape(doc: &SceneDoc) -> Result<()> {
    if doc.tag != DOC_TAG {
        return Err(VisFxError::InvalidDocument(format!("tag {:?}", doc.tag)));
    }
    if doc.generator_id.is_empty() || doc.generator_id.len() > 200 {
        return Err(VisFxError::InvalidDocument("generatorId".into()));
    }
    doc.seed
        .parse::<u64>()
        .map_err(|_| VisFxError::InvalidDocument("seed must be decimal u64".into()))?;
    if doc.layers.len() > 64 || doc.actions.len() > 256 {
        return Err(VisFxError::InvalidDocument("layers>64/actions>256".into()));
    }
    for a in &doc.actions {
        if !ALLOWED_ACTIONS.contains(&a.action.as_str()) {
            return Err(VisFxError::Forbidden(format!("action {:?}", a.action)));
        }
        check_forbidden_value(&a.value, "action.value")?;
    }
    for l in &doc.layers {
        if l.name.is_empty() || l.name.len() > 200 {
            return Err(VisFxError::InvalidDocument("layer name".into()));
        }
        if !matches!(l.channel.as_str(), "preview" | "program") {
            return Err(VisFxError::InvalidDocument(format!(
                "channel {:?}",
                l.channel
            )));
        }
        match l.kind.as_str() {
            "generator" => {
                let g = l
                    .generator
                    .as_ref()
                    .ok_or_else(|| VisFxError::InvalidDocument("generator missing".into()))?;
                if g.preset.is_none() && g.shader_body.is_none() {
                    return Err(VisFxError::InvalidDocument(
                        "generator needs preset|shaderBody".into(),
                    ));
                }
                if let Some(b) = &g.shader_body {
                    if b.len() > 64 * 1024 {
                        return Err(VisFxError::InvalidDocument("shaderBody >64KiB".into()));
                    }
                }
            }
            "media" => {
                let m = l
                    .media
                    .as_ref()
                    .ok_or_else(|| VisFxError::InvalidDocument("media missing".into()))?;
                if m.artifact.is_empty()
                    || m.artifact.contains('/')
                    || m.artifact.contains('\\')
                    || m.artifact.contains("..")
                {
                    return Err(VisFxError::InvalidDocument(format!(
                        "artifact name {:?}",
                        m.artifact
                    )));
                }
                m.duration_ticks
                    .parse::<i64>()
                    .ok()
                    .filter(|&d| d >= 0)
                    .ok_or_else(|| VisFxError::InvalidDocument("durationTicks".into()))?;
            }
            other => {
                return Err(VisFxError::InvalidDocument(format!("layer kind {other:?}")));
            }
        }
        for (name, s) in [("inTicks", &l.in_ticks), ("outTicks", &l.out_ticks)] {
            if let Some(s) = s {
                s.parse::<i64>()
                    .map_err(|_| VisFxError::InvalidDocument(format!("{name} not i64")))?;
            }
        }
        if let Some(t) = &l.transform {
            for (n, f) in [
                ("x", t.x),
                ("y", t.y),
                ("scaleX", t.scale_x),
                ("scaleY", t.scale_y),
                ("rotationRad", t.rotation_rad),
                ("opacity", t.opacity),
            ] {
                if !f.is_finite() {
                    return Err(VisFxError::InvalidDocument(format!(
                        "transform.{n} not finite"
                    )));
                }
            }
            if !(0.0..=1.0).contains(&t.opacity) {
                return Err(VisFxError::InvalidDocument("opacity outside 0..1".into()));
            }
        }
    }
    check_forbidden_value(&serde_json::to_value(doc).unwrap_or_default(), "doc")?;
    Ok(())
}

pub fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}
