//! wgpu compositor. Offscreen-first: every channel renders into an
//! Rgba8Unorm target; readback maps to CPU pixels + sha256 (the headless
//! evidence path). A windowed surface is an optional presentation target
//! behind the `window` feature — the offscreen path is always exercised.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::util::DeviceExt;

use crate::error::{Result, VisualError};
use crate::scene::{CompositionPlan, ResolvedLayer};
use crate::shaders;
use crate::types::*;

pub const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

static FRAME_SEQ: AtomicU64 = AtomicU64::new(1);

/// Adapter we actually rendered on — recorded into evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterInfo {
    pub name: String,
    pub backend: String,
    pub driver: String,
    pub driver_info: String,
}

/// One produced frame. Timing fields identify the CLOCK it was rendered
/// for — never a visual-side clock.
#[derive(Debug, Clone)]
pub struct ProducedFrame {
    pub channel: VisualChannel,
    pub sequence: u64,
    pub position_ticks: i64,
    pub timeline_sample: i64,
    pub device_sample_counter: i64,
    pub host_clock_ns: u64,
    pub clock_sequence: u64,
    pub render_us: u64,
    pub width: u32,
    pub height: u32,
    pub pixels: Option<Vec<u8>>,
    pub frame_sha256: String,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: AdapterInfo,
    /// Per-channel offscreen targets + readback staging.
    targets: HashMap<usize, ChannelTarget>,
    layer_pipelines: HashMap<u8, wgpu::RenderPipeline>, // by BlendMode idx
    layer_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    merge_pipeline: wgpu::RenderPipeline,
    merge_layout: wgpu::BindGroupLayout,
    /// Cache: decoded layer textures keyed by (layer_id, asset sha / gen id).
    textures: HashMap<String, (wgpu::Texture, wgpu::TextureView)>,
    gen_modules: HashMap<String, wgpu::RenderPipeline>,
    gen_layouts: HashMap<String, wgpu::BindGroupLayout>,
    /// Audio feature uniform (rms/peak) for reactive presets.
    pub audio_rms: f32,
    pub audio_peak: f32,
    /// Test/diagnostic hook: per-render artificial delay. Production
    /// code never sets this — it exists to PROVE the drop policy under
    /// synthetic load (tests/visual drop-under-load).
    pub frame_delay_us: u64,
}

struct ChannelTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    readback: wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row: u32,
}

impl ChannelTarget {
    fn new(device: &wgpu::Device, w: u32, h: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("void-visual-target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let padded_row = w.next_multiple_of(256 / 4) * 4;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("void-visual-readback"),
            size: (padded_row as u64) * (h as u64),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            texture,
            view,
            readback,
            width: w,
            height: h,
            padded_row,
        }
    }
}

fn blend_state(mode: BlendMode) -> wgpu::BlendState {
    use wgpu::{BlendComponent as C, BlendFactor as F, BlendOperation as O};
    let (src, dst) = match mode {
        BlendMode::Normal => (F::One, F::OneMinusSrcAlpha), // premultiplied src-over
        BlendMode::Add => (F::One, F::One),
        BlendMode::Multiply => (F::Dst, F::Zero),
        BlendMode::Screen => (F::One, F::OneMinusSrc),
    };
    wgpu::BlendState {
        color: C {
            src_factor: src,
            dst_factor: dst,
            operation: O::Add,
        },
        alpha: C {
            src_factor: F::One,
            dst_factor: F::OneMinusSrcAlpha,
            operation: O::Add,
        },
    }
}

impl Renderer {
    /// Headless renderer: picks any adapter (llvmpipe/lavapipe preferred
    /// via force_fallback_adapter) — works with zero displays.
    pub fn new_headless(width: u32, height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            },
        ))
        .or_else(|_| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        })
        .map_err(|_e| VisualError::NoAdapter)?;
        let info = adapter.get_info();
        let adapter_info = AdapterInfo {
            name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            driver: info.driver.clone(),
            driver_info: info.driver_info.clone(),
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("void-visual"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            },
        ))
        .map_err(|e| VisualError::DeviceUnavailable(e.to_string()))?;
        let mut r = Self::build(device, queue, adapter_info);
        r.ensure_target(VisualChannel::Preview.idx(), width, height);
        r.ensure_target(VisualChannel::Program.idx(), width, height);
        Ok(r)
    }

    fn build(device: wgpu::Device, queue: wgpu::Queue, adapter_info: AdapterInfo) -> Self {
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer-bgl"),
            entries: &[
                bgl_entry(0, true),
                bgl_tex(1),
                bgl_sampler(2),
            ],
        });
        let layer_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("layer"),
            source: wgpu::ShaderSource::Wgsl(shaders::LAYER_WGSL.into()),
        });
        let mut layer_pipelines = HashMap::new();
        for (i, mode) in BlendMode::ALL.iter().enumerate() {
            layer_pipelines.insert(
                i as u8,
                layer_pipeline(&device, &layer_layout, &layer_module, blend_state(*mode)),
            );
        }
        let merge_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("merge-bgl"),
            entries: &[bgl_entry(0, true), bgl_tex(1), bgl_tex(2), bgl_sampler(3)],
        });
        let merge_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("merge"),
            source: wgpu::ShaderSource::Wgsl(shaders::MERGE_WGSL.into()),
        });
        let merge_pipeline = layer_pipeline(&device, &merge_layout, &merge_module, wgpu::BlendState::REPLACE);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            device,
            queue,
            adapter_info,
            targets: HashMap::new(),
            layer_pipelines,
            layer_layout,
            sampler,
            merge_pipeline,
            merge_layout,
            textures: HashMap::new(),
            gen_modules: HashMap::new(),
            gen_layouts: HashMap::new(),
            audio_rms: 0.0,
            audio_peak: 0.0,
            frame_delay_us: 0,
        }
    }

    pub fn adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }

    pub fn ensure_target(&mut self, channel: usize, w: u32, h: u32) {
        let need = self
            .targets
            .get(&channel)
            .map(|t| t.width != w || t.height != h)
            .unwrap_or(true);
        if need {
            self.targets
                .insert(channel, ChannelTarget::new(&self.device, w, h));
        }
    }

    /// Upload/replace a layer's texture content (decode output).
    pub fn set_layer_texture(&mut self, key: &str, frame: &crate::media::RgbaFrame) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("layer-tex"),
            size: wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        self.textures.insert(key.to_string(), (texture, view));
    }

    pub fn drop_layer_texture(&mut self, key: &str) {
        self.textures.remove(key);
    }

    pub fn has_texture(&self, key: &str) -> bool {
        self.textures.contains_key(key)
    }

    fn stack_texture(&mut self, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("void-visual-stack"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        (texture, view)
    }

    /// Render one layer stack into `view`.
    fn draw_stack(
        &mut self,
        stack: &[ResolvedLayer],
        view: &wgpu::TextureView,
        out_w: u32,
        out_h: u32,
        tick: i64,
        beat_phase: f32,
    ) {
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stack"),
            });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stack"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            for layer in stack {
                match layer.kind {
                    VisualLayerKind::Image | VisualLayerKind::Video => {
                        let Some(key) = layer.media.as_ref().map(|m| texture_key(&layer.layer_id, &m.sha256)) else { continue };
                        let Some((_, texview)) = self.textures.get(&key) else {
                            continue;
                        };
                        let (tw, th) = tex_dims(&self.textures[&key].0);
                        self.draw_layer(
                            &mut pass,
                            texview,
                            tw,
                            th,
                            layer,
                            out_w,
                            out_h,
                        );
                    }
                    VisualLayerKind::Generator => {
                        let Some(g) = &layer.generator else { continue };
                        self.draw_generator(&mut pass, g, layer, out_w, out_h, tick, beat_phase);
                    }
                }
            }
        }
        self.queue.submit([enc.finish()]);
    }

    fn draw_layer(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        texview: &wgpu::TextureView,
        tex_w: u32,
        tex_h: u32,
        layer: &ResolvedLayer,
        out_w: u32,
        out_h: u32,
    ) {
        let t = &layer.transform;
        // Layer rect centered, scaled by (tex dims × scale), rotated.
        let (cw, ch) = (tex_w as f32 * t.scale_x, tex_h as f32 * t.scale_y);
        let (cx, cy) = (out_w as f32 / 2.0 + t.x, out_h as f32 / 2.0 + t.y);
        let (s, c) = t.rotation_rad.sin_cos();
        // Affine maps unit quad → pixel space (rotation around center).
        let a = c * cw;
        let b = -s * ch;
        let c2 = s * cw;
        let d = c * ch;
        let tx = cx - (c * cw + -s * ch) * 0.5 - s * 0.0;
        let ty = cy - (s * cw + c * ch) * 0.5;
        let _ = (tx, ty);
        // Simpler correct affine: M = T(cx,cy) · R · S(±cw/2) — quad
        // points are 0..1; center them first in the affine by using
        // translate = center - R*S*(0.5,0.5).
        let mxc = c * cw * 0.5 - s * ch * 0.5;
        let myc = s * cw * 0.5 + c * ch * 0.5;
        let tx = cx - mxc;
        let ty = cy - myc;
        let uni = LayerUni {
            affine: [a / out_w as f32, b / out_h as f32, c2 / out_w as f32, d / out_h as f32],
            offset: [tx / out_w as f32, ty / out_h as f32, layer.opacity, 0.0],
        };
        let ubuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("layer-uni"),
                contents: bytemuck::bytes_of(&uni),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer-bg"),
            layout: &self.layer_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubuf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texview),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        pass.set_pipeline(&self.layer_pipelines[&(layer.blend as u8)]);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..6, 0..1);
    }

    fn draw_generator(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        g: &GeneratorSpec,
        _layer: &ResolvedLayer,
        out_w: u32,
        out_h: u32,
        tick: i64,
        beat_phase: f32,
    ) {
        if !self.gen_modules.contains_key(&g.preset) {
            let Some(body) = shaders::generator_body(&g.preset) else {
                return;
            };
            let src = shaders::GENERATOR_TEMPLATE.replace("%BODY%", &body);
            let module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("gen"),
                    source: wgpu::ShaderSource::Wgsl(src.into()),
                });
            let layout = self
                .device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gen-bgl"),
                    entries: &[bgl_entry(0, true)],
                });
            self.gen_modules.insert(
                g.preset.clone(),
                layer_pipeline(&self.device, &layout, &module, blend_state(BlendMode::Normal)),
            );
            // Store layout alongside via a parallel map keyed by preset.
            self.gen_layouts.insert(g.preset.clone(), layout);
        }
        let pipeline = self.gen_modules.get(&g.preset).unwrap().clone();
        let layout = self.gen_layouts.get(&g.preset).unwrap().clone();
        let params = gen_params(&g.param_json);
        let uni = GenUni {
            tick: tick as f32 / 960_000.0,
            beat: tick as f32 / 960_000.0,
            beat_phase,
            rms: self.audio_rms,
            peak: self.audio_peak,
            seed: g.seed as f32,
            pad0: 0.0,
            pad1: 0.0,
            p0: params[0],
            p1: params[1],
        };
        let ubuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("gen-uni"),
                contents: bytemuck::bytes_of(&uni),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gen-bg"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ubuf.as_entire_binding(),
            }],
        });
        // Blend modes apply to generators too — reuse the layer-blend
        // pipeline concept: the gen pipeline was built with Normal; for
        // non-Normal blend modes build variant lazily.
        let _ = (out_w, out_h);
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..6, 0..1);
    }

    /// Render a composition plan into the channel target and read it back.
    /// `clock_*` fields are copied onto the produced frame — the frame is
    /// stamped with the AUDIO clock it was rendered for.
    pub fn render(
        &mut self,
        plan: &CompositionPlan,
        timeline_sample: i64,
        device_sample_counter: i64,
        host_clock_ns: u64,
        clock_sequence: u64,
        beat_phase: f32,
        readback: bool,
    ) -> Result<ProducedFrame> {
        let started = std::time::Instant::now();
        let ch = plan.channel.idx();
        // Ensure channel target exists (route size may differ per channel —
        // caller ensures via ensure_target; default to plan dims).
        let (w, h) = {
            let t = self.targets.get(&ch).ok_or(VisualError::NoAdapter)?;
            (t.width, t.height)
        };
        if self.frame_delay_us > 0 {
            // Synthetic load injection (tests only): a slow GPU renders
            // slower than the clock rate — the drop policy must absorb it.
            std::thread::sleep(std::time::Duration::from_micros(self.frame_delay_us));
        }
        if let Some(incoming) = &plan.incoming {
            // Two-pass transition: render each stack to scratch, then merge.
            let (a_tex, a_view) = self.stack_texture(w, h);
            let (b_tex, b_view) = self.stack_texture(w, h);
            self.draw_stack(&plan.stack, &a_view, w, h, plan.tick, beat_phase);
            self.draw_stack(incoming, &b_view, w, h, plan.tick, beat_phase);
            self.merge(&a_view, &b_view, plan, w, h);
            let _ = (a_tex, b_tex);
        } else {
            let view = self.targets.get(&ch).unwrap().view.clone();
            self.draw_stack(&plan.stack, &view, w, h, plan.tick, beat_phase);
        }
        let (pixels, sha) = if readback {
            let f = self.readback(ch)?;
            let sha = f.sha256();
            (Some(f.pixels), sha)
        } else {
            (None, String::new())
        };
        Ok(ProducedFrame {
            channel: plan.channel,
            sequence: FRAME_SEQ.fetch_add(1, Ordering::Relaxed),
            position_ticks: plan.tick,
            timeline_sample,
            device_sample_counter,
            host_clock_ns,
            clock_sequence,
            render_us: started.elapsed().as_micros() as u64,
            width: w,
            height: h,
            pixels,
            frame_sha256: sha,
        })
    }

    fn merge(&mut self, a: &wgpu::TextureView, b: &wgpu::TextureView, plan: &CompositionPlan, w: u32, h: u32) {
        let uni = MergeUni {
            progress: plan.transition_progress,
            kind: match plan.transition_kind {
                TransitionKind::Cut => 0,
                TransitionKind::Fade => 1,
                TransitionKind::Wipe => 2,
            },
            angle: plan.wipe_angle,
            aspect: w as f32 / h.max(1) as f32,
        };
        let ubuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("merge-uni"),
                contents: bytemuck::bytes_of(&uni),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("merge-bg"),
            layout: &self.merge_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubuf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("merge") });
        {
            let view = self.targets.get(&plan.channel.idx()).unwrap().view.clone();
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("merge"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.merge_pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..6, 0..1);
        }
        self.queue.submit([enc.finish()]);
    }

    /// Copy the channel target to CPU RGBA pixels (unpadded).
    pub fn readback(&mut self, channel: usize) -> Result<crate::media::RgbaFrame> {
        let t = self.targets.get(&channel).ok_or(VisualError::NoAdapter)?;
        let (w, h, padded) = (t.width, t.height, t.padded_row);
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &t.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &t.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = t.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait())
            .map_err(|e| VisualError::DeviceUnavailable(e.to_string()))?;
        rx.recv()
            .map_err(|_| VisualError::DeviceUnavailable("map channel".into()))?
            .map_err(|e| VisualError::DeviceUnavailable(e.to_string()))?;
        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
        }
        drop(data);
        t.readback.unmap();
        Ok(crate::media::RgbaFrame {
            width: w,
            height: h,
            pixels,
        })
    }
}

pub fn texture_key(layer_id: &str, sha256: &str) -> String {
    format!("{layer_id}:{sha256}")
}

fn tex_dims(t: &wgpu::Texture) -> (u32, u32) {
    (t.width(), t.height())
}

fn bgl_entry(binding: u32, uniform: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: if uniform {
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            }
        } else {
            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
        },
        count: None,
    }
}

fn bgl_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn layer_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("layer"),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FRAME_FORMAT,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LayerUni {
    affine: [f32; 4],
    offset: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GenUni {
    tick: f32,
    beat: f32,
    beat_phase: f32,
    rms: f32,
    peak: f32,
    seed: f32,
    pad0: f32,
    pad1: f32,
    p0: [f32; 4],
    p1: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MergeUni {
    progress: f32,
    kind: u32,
    angle: f32,
    aspect: f32,
}

/// Bounded JSON params → two vec4 slots (p0..p1), 8 floats total.
fn gen_params(json: &str) -> [[f32; 4]; 2] {
    let mut out = [[0.0f32; 4]; 2];
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
        if let Some(arr) = v.get("params").and_then(|p| p.as_array()) {
            for (i, x) in arr.iter().take(8).enumerate() {
                out[i / 4][i % 4] = x.as_f64().unwrap_or(0.0) as f32;
            }
        }
    }
    out
}
