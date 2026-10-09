//! WGSL sources for the compositor + built-in generator presets.
//! Everything is deterministic: presets are pure functions of
//! (tick, seed, params, beat_phase, audio features) — no wall clock.

/// Full-screen textured-quad layer shader. One bind group per layer:
///   b0: uniform { transform mat3x2 packed as 4 vec2 rows + opacity }
///   b1: sampled texture, b2: sampler
pub const LAYER_WGSL: &str = r#"
struct LayerU {
    affine: vec4<f32>,   // (a,b,c,d) 2x2 affine row-major
    offset: vec4<f32>,   // (tx,ty,opacity,unused)
};
@group(0) @binding(0) var<uniform> u: LayerU;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var s_src: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    // Full-viewport quad: 2 triangles, uv 0..1 over the layer's rect.
    var quad = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));
    let p = quad[vi];
    let xf = vec2<f32>(
        u.affine.x * p.x + u.affine.y * p.y + u.offset.x,
        u.affine.z * p.x + u.affine.w * p.y + u.offset.y);
    var o: VsOut;
    // NDC: x,y already in output pixels -> map [0..w]x[0..h] to clip.
    o.pos = vec4<f32>(xf.x * 2.0 - 1.0, 1.0 - xf.y * 2.0, 0.0, 1.0);
    o.uv = p;
    return o;
}

@fragment
fn fs(v: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(t_src, s_src, v.uv);
    return vec4<f32>(c.rgb * c.a, c.a) * u.offset.z;
}
"#;

/// Generator-layer shader template. `%BODY%` must define
/// `fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32>` returning premultiplied
/// color in 0..1.
pub const GENERATOR_TEMPLATE: &str = r#"
struct GenU {
    tick: f32,         // musical tick (ticks/960000 = quarters)
    beat: f32,         // beats elapsed (float quarter index)
    beat_phase: f32,   // 0..1 phase inside current quarter
    rms: f32,          // latest decimated audio rms
    peak: f32,         // latest decimated audio peak
    seed: f32,         // preset seed
    p0: vec4<f32>,     // params
    p1: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: GenU;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    var quad = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    var o: VsOut;
    o.pos = vec4<f32>(quad[vi], 0.0, 1.0);
    o.uv = quad[vi] * 0.5 + vec2<f32>(0.5);
    return o;
}

%BODY%

@fragment
fn fs(v: VsOut) -> @location(0) vec4<f32> {
    return gen(v.uv, u);
}
"#;

/// Transition merge pass: blend two stack composites with progress +
/// wipe mask.
pub const MERGE_WGSL: &str = r#"
struct MergeU {
    progress: f32,
    kind: u32,         // 0 cut, 1 fade, 2 wipe
    angle: f32,
    aspect: f32,
};
@group(0) @binding(0) var<uniform> u: MergeU;
@group(0) @binding(1) var t_a: texture_2d<f32>;  // from stack
@group(0) @binding(2) var t_b: texture_2d<f32>;  // incoming stack
@group(0) @binding(3) var s_lin: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    var quad = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    var o: VsOut;
    o.pos = vec4<f32>(quad[vi], 0.0, 1.0);
    o.uv = quad[vi] * 0.5 + vec2<f32>(0.5);
    return o;
}

@fragment
fn fs(v: VsOut) -> @location(0) vec4<f32> {
    let a = textureSample(t_a, s_lin, v.uv);
    let b = textureSample(t_b, s_lin, v.uv);
    var w = clamp(u.progress, 0.0, 1.0);
    if (u.kind == 2u) {
        // Wipe: spatial mask rotated by angle; progress sweeps -0.5..1.5
        // of the projected extent.
        let dir = vec2<f32>(cos(u.angle), sin(u.angle) / max(u.aspect, 0.001));
        let proj = dot(v.uv - vec2<f32>(0.5), dir);
        let edge = (u.progress * 2.0 - 0.5) * 0.6;
        w = clamp((proj - (edge - 0.6)) / 0.12 + 0.5, 0.0, 1.0);
        w = 1.0 - w;
    }
    if (u.kind == 0u) {
        w = f32(u.progress >= 1.0);
    }
    // premultiplied composite of A under B
    return a * (1.0 - w) + b * w;
}
"#;

/// Generator preset bodies (deterministic, param-driven).
pub fn generator_body(preset: &str) -> Option<String> {
    let body = match preset {
        "black" => {
            r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
"#
        }
        "color-bars" => {
            r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    // SMPTE-style bars, deterministic; seed picks hue rotation.
    let n = 8.0;
    let i = u32(floor(uv.x * n));
    var c = vec3<f32>(0.0);
    let r = f32((i + u32(u.seed)) % 8u);
    c = vec3<f32>(
        select(0.0, 1.0, (u32(r) & 4u) != 0u),
        select(0.0, 1.0, (u32(r) & 2u) != 0u),
        select(0.0, 1.0, (u32(r) & 1u) != 0u));
    let a = 1.0;
    return vec4<f32>(c * a, a);
}
"#
        }
        "checker" => {
            r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    let cells = max(u.p0.x, 2.0);
    let p = vec2<u32>(vec2<f32>(uv * cells));
    let on = f32((p.x + p.y) & 1u);
    let c = vec3<f32>(on * u.p0.y + u.p0.z * (1.0 - on));
    return vec4<f32>(c, 1.0);
}
"#
        }
        "gradient" => {
            r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    // Vertical two-stop gradient; p0.rgb = top, p1.rgb = bottom.
    let c = mix(u.p0.rgb, u.p1.rgb, uv.y);
    return vec4<f32>(c, 1.0);
}
"#
        }
        "plasma" => {
            r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    // Deterministic plasma driven by beat + seed (no wall time).
    let t = u.beat * 0.25 + u.seed;
    let p = uv * 8.0;
    let v = sin(p.x + t) + sin(p.y + t * 1.3) + sin((p.x + p.y) * 0.7 + t);
    let c = vec3<f32>(
        0.5 + 0.5 * sin(v + 0.0),
        0.5 + 0.5 * sin(v + 2.094),
        0.5 + 0.5 * sin(v + 4.188));
    return vec4<f32>(c, 1.0);
}
"#
        }
        "pulse" => {
            r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    // Beat-synced radial pulse: radius breathes on beat_phase + rms.
    let d = distance(uv, vec2<f32>(0.5));
    let r = 0.18 + 0.22 * (1.0 - u.beat_phase) + u.rms * 0.15;
    let w = smoothstep(0.0, 0.02, r - d) * smoothstep(0.0, 0.02, d - r + 0.08);
    let c = vec3<f32>(u.p0.x, u.p0.y, u.p0.z) * (0.4 + u.peak * 0.6 + 0.6 * w);
    return vec4<f32>(c * w, w);
}
"#
        }
        _ => return None,
    };
    Some(body.to_string())
}
