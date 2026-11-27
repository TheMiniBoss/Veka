//! Le cœur du renderer : transforme une `DrawList` en lots instanciés et les dessine.

use bytemuck::{Pod, Zeroable};
use veka_foundation::{Rect, Transform2D};
use veka_graphics::{BackdropEffect, DrawCommand, DrawList, Paint, Shape};
use veka_render::FrameInfo;

use crate::atlas::{Atlas, Full, GlyphKey};
use crate::backdrop::{self, Backdrop, Capture, EffectInstance};
use crate::ramps::{self, Ramps, RampsFull};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ShapeInstance {
    rect: [f32; 4],
    radii: [f32; 4],
    /// Couleur unie ; pour un dégradé, l'opacité du calque sur les quatre composantes.
    color: [f32; 4],
    xform: [f32; 4],
    extra: [f32; 4],
    /// Sorte de remplissage (0 uni, 1 linéaire, 2 radial, 3 elliptique, 4 angulaire, 5 maillé), puis
    /// position de la rampe dans l'atlas (texels).
    fill: [f32; 4],
    /// Géométrie du dégradé, en points du repère local.
    geo: [f32; 4],
    geo2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GlyphInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shapes,
    Glyphs,
    Effects,
}

/// Une suite d'instances du même pipeline, avec le même découpage.
#[derive(Clone, Debug)]
struct Batch {
    kind: Kind,
    range: std::ops::Range<u32>,
    scissor: [u32; 4],
    /// Capture de l'arrière-plan à faire avant ce lot.
    capture: Option<usize>,
}

/// Ce que la dernière image a demandé au GPU (tests, mesures).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Captures de l'arrière-plan (copie + pyramide de flou).
    pub captures: u32,
    /// Effets d'arrière-plan composés.
    pub effects: u32,
    /// Passes de rendu (dessin, pyramide, copie vers la cible).
    pub passes: u32,
}

/// Pipelines et ressources GPU, pour un format de cible donné.
pub struct Painter {
    format: wgpu::TextureFormat,
    globals: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    shape_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_layout: wgpu::BindGroupLayout,
    glyph_bg: Option<(u32, wgpu::BindGroup)>,
    ramps: Ramps,
    ramps_bg: wgpu::BindGroup,
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler_linear: wgpu::Sampler,
    sampler_nearest: wgpu::Sampler,
    atlas: Atlas,
    intermediate: Option<(wgpu::Texture, wgpu::TextureView, wgpu::BindGroup)>,
    shape_buf: Option<wgpu::Buffer>,
    glyph_buf: Option<wgpu::Buffer>,
    shapes: Vec<ShapeInstance>,
    glyphs: Vec<GlyphInstance>,
    batches: Vec<Batch>,
    backdrop: Backdrop,
    effects: Vec<EffectInstance>,
    effect_capture: Vec<usize>,
    captures: Vec<Capture>,
    stats: FrameStats,
}

impl std::fmt::Debug for Painter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painter").field("format", &self.format).finish()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    vs: &str,
    fs: &str,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(vs),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vs),
            compilation_options: Default::default(),
            buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

const SHAPE_ATTRS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
    4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
];
const GLYPH_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];

impl Painter {
    /// `format` : format de la cible (de préférence sRGB, pour mélanger les couleurs en linéaire).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("veka shaders"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("veka blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("veka globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("veka globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("veka globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let texture_layout = |label| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            })
        };
        let glyph_layout = texture_layout("veka atlas");
        let blit_layout = texture_layout("veka blit");
        let ramps_layout = texture_layout("veka dégradés");
        let shape_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka formes"),
            bind_group_layouts: &[Some(&globals_layout), Some(&ramps_layout)],
            immediate_size: 0,
        });
        let glyph_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka glyphes"),
            bind_group_layouts: &[Some(&globals_layout), Some(&glyph_layout)],
            immediate_size: 0,
        });
        let blit_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka blit"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let premul = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let shape_pipeline = pipeline(
            device,
            &module,
            &shape_pl,
            "shape_vs",
            "shape_fs",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ShapeInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &SHAPE_ATTRS,
            })],
            format,
            premul,
        );
        let glyph_pipeline = pipeline(
            device,
            &module,
            &glyph_pl,
            "glyph_vs",
            "glyph_fs",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &GLYPH_ATTRS,
            })],
            format,
            premul,
        );
        let blit_pipeline = pipeline(
            device,
            &blit_module,
            &blit_pl,
            "blit_vs",
            "blit_fs",
            &[],
            format,
            None,
        );
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let backdrop = Backdrop::new(device, format, &globals_layout);
        let ramps = Ramps::new(device);
        let sampler_linear = sampler(wgpu::FilterMode::Linear);
        let ramps_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("veka dégradés"),
            layout: &ramps_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ramps.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler_linear),
                },
            ],
        });
        Self {
            format,
            globals,
            globals_bg,
            shape_pipeline,
            glyph_pipeline,
            glyph_layout,
            glyph_bg: None,
            ramps,
            ramps_bg,
            blit_pipeline,
            blit_layout,
            sampler_linear,
            sampler_nearest: sampler(wgpu::FilterMode::Nearest),
            atlas: Atlas::new(device, 1024),
            intermediate: None,
            shape_buf: None,
            glyph_buf: None,
            shapes: Vec::new(),
            glyphs: Vec::new(),
            batches: Vec::new(),
            backdrop,
            effects: Vec::new(),
            effect_capture: Vec::new(),
            captures: Vec::new(),
            stats: FrameStats::default(),
        }
    }

    /// Ce que la dernière image a demandé au GPU.
    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Dessine `list` dans `target` (une vue de texture au format de ce painter).
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        list: &DrawList,
        frame: FrameInfo,
        target: &wgpu::TextureView,
    ) {
        let (w, h) = (frame.width.max(1), frame.height.max(1));
        // Préparation des instances ; si un atlas déborde, il est agrandi (glyphes) ou vidé (dégradés) et
        // on recommence une fois.
        match self.prepare(device, queue, list, frame) {
            Ok(()) => {}
            Err(Overflow::Glyphs) => {
                let _ = self.prepare(device, queue, list, frame);
            }
            Err(Overflow::Ramps) => {
                self.ramps.clear();
                let _ = self.prepare(device, queue, list, frame);
            }
        }
        queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                viewport: [w as f32, h as f32],
                _pad: [0.0; 2],
            }),
        );
        let shape_buf = upload(
            device,
            queue,
            &mut self.shape_buf,
            bytemuck::cast_slice(&self.shapes),
            "veka formes",
        );
        let glyph_buf = upload(
            device,
            queue,
            &mut self.glyph_buf,
            bytemuck::cast_slice(&self.glyphs),
            "veka glyphes",
        );
        if self.glyph_bg.as_ref().is_none_or(|(s, _)| *s != self.atlas.size) {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("veka atlas"),
                layout: &self.glyph_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.atlas.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                    },
                ],
            });
            self.glyph_bg = Some((self.atlas.size, bg));
        }
        self.ensure_intermediate(device, w, h);
        self.backdrop.prepare(device, queue, &self.captures, w, h);
        let effect_buf = upload(
            device,
            queue,
            &mut self.backdrop.instance_buf,
            bytemuck::cast_slice(&self.effects),
            "veka effets",
        );
        let (inter_tex, inter_view, inter_bg) = self.intermediate.as_ref().expect("texture intermédiaire");
        let mut stats = FrameStats {
            captures: self.captures.len() as u32,
            effects: self.effects.len() as u32,
            passes: 0,
        };

        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("veka") });
        // Le dessin est coupé avant chaque lot qui commence par une capture de l'arrière-plan.
        let mut start = 0;
        let mut first = true;
        while first || start < self.batches.len() {
            let end = (start + 1..self.batches.len())
                .find(|&i| self.batches[i].capture.is_some())
                .unwrap_or(self.batches.len());
            if let Some(c) = self.batches.get(start).and_then(|b| b.capture) {
                if first {
                    // Rien n'est encore dessiné : la capture lit une image vide.
                    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("veka effacement"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: inter_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
                    stats.passes += 1;
                    first = false;
                }
                self.backdrop.capture(&mut enc, inter_tex, &self.captures, c);
                stats.passes += 1 + 3 * self.captures[c].levels;
            }
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("veka dessin"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: inter_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if first {
                            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            stats.passes += 1;
            first = false;
            pass.set_bind_group(0, &self.globals_bg, &[]);
            let glyph_bg = &self.glyph_bg.as_ref().expect("atlas").1;
            for b in &self.batches[start.min(self.batches.len())..end] {
                if b.range.is_empty() || b.scissor[2] == 0 || b.scissor[3] == 0 {
                    continue;
                }
                pass.set_scissor_rect(b.scissor[0], b.scissor[1], b.scissor[2], b.scissor[3]);
                match b.kind {
                    Kind::Shapes => {
                        pass.set_pipeline(&self.shape_pipeline);
                        pass.set_bind_group(1, &self.ramps_bg, &[]);
                        pass.set_vertex_buffer(0, shape_buf.as_ref().expect("formes").slice(..));
                    }
                    Kind::Glyphs => {
                        pass.set_pipeline(&self.glyph_pipeline);
                        pass.set_bind_group(1, glyph_bg, &[]);
                        pass.set_vertex_buffer(0, glyph_buf.as_ref().expect("glyphes").slice(..));
                    }
                    Kind::Effects => {
                        let Some(bg) = self.backdrop.effect_bind_group() else {
                            continue;
                        };
                        pass.set_pipeline(&self.backdrop.effect_pipeline);
                        pass.set_bind_group(1, bg, &[]);
                        pass.set_vertex_buffer(0, effect_buf.as_ref().expect("effets").slice(..));
                    }
                }
                pass.draw(0..6, b.range.clone());
            }
            drop(pass);
            start = end;
        }
        {
            // Copie vers la cible.
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("veka copie"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, inter_bg, &[]);
            pass.draw(0..3, 0..1);
            stats.passes += 1;
        }
        self.stats = stats;
        queue.submit([enc.finish()]);
    }

    fn ensure_intermediate(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        if self
            .intermediate
            .as_ref()
            .is_some_and(|(t, ..)| t.width() == w && t.height() == h)
        {
            return;
        }
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("veka intermédiaire"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("veka blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_nearest),
                },
            ],
        });
        self.intermediate = Some((tex, view, bg));
    }

    /// Parcourt la liste : formes et glyphes en instances, regroupées en lots consécutifs.
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        list: &DrawList,
        frame: FrameInfo,
    ) -> Result<(), Overflow> {
        self.shapes.clear();
        self.glyphs.clear();
        self.batches.clear();
        self.effects.clear();
        self.effect_capture.clear();
        self.captures.clear();
        let (fw, fh) = (frame.width.max(1) as f32, frame.height.max(1) as f32);
        // Ce qui a été dessiné depuis la dernière capture (pixels), pour savoir si un effet peut la
        // partager.
        let mut dirty: Vec<[f32; 4]> = Vec::new();
        let mut current: Option<usize> = None;
        let s = frame.scale;
        let full = [0, 0, frame.width.max(1), frame.height.max(1)];
        let mut transforms = vec![Transform2D::IDENTITY];
        let mut clips: Vec<[u32; 4]> = vec![full];
        let mut opacity = vec![1.0f32];
        let atlas_size = self.atlas.size as f32;

        for cmd in &list.commands {
            let t = *transforms.last().expect("pile");
            let scissor = *clips.last().expect("pile");
            let alpha = *opacity.last().expect("pile");
            match cmd {
                DrawCommand::FillShape { shape, rect, paint } => {
                    let inst = shape_instance(&mut self.ramps, queue, shape, rect, paint, &t, s, alpha, 0.0)?;
                    push(
                        &mut self.batches,
                        Kind::Shapes,
                        self.shapes.len() as u32,
                        scissor,
                        None,
                    );
                    self.shapes.push(inst);
                    if current.is_some() {
                        mark(&mut dirty, transform_bounds(&t, rect, 1.0, s));
                    }
                }
                DrawCommand::StrokeShape {
                    shape,
                    rect,
                    paint,
                    width,
                } => {
                    let inst =
                        shape_instance(&mut self.ramps, queue, shape, rect, paint, &t, s, alpha, *width)?;
                    push(
                        &mut self.batches,
                        Kind::Shapes,
                        self.shapes.len() as u32,
                        scissor,
                        None,
                    );
                    self.shapes.push(inst);
                    if current.is_some() {
                        mark(&mut dirty, transform_bounds(&t, rect, 1.0 + width * 0.5, s));
                    }
                }
                DrawCommand::Glyphs(run) => {
                    let Some(font) = list.fonts.get(run.font.0 as usize) else {
                        continue;
                    };
                    let data = (*font.data).as_ref();
                    // Échelle de la transformation (pas de rotation des glyphes à l'étape 1).
                    let ts = (t.a * t.d - t.b * t.c).abs().sqrt().max(1e-4);
                    let size_px = run.size * s * ts;
                    // Texte : couleur unie (moyenne d'un dégradé) pour l'instant.
                    let color = premul(paint_color(&run.paint), alpha);
                    for g in &list.glyphs[run.glyphs.start as usize..run.glyphs.end as usize] {
                        let p = t.apply(veka_foundation::Point::new(g.x, g.y));
                        let (gx, gy) = (p.x * s, p.y * s);
                        let ix = gx.floor();
                        let subpx = (((gx - ix) * 4.0).round() as u8).min(3);
                        let key = GlyphKey {
                            font: font.key,
                            index: font.index,
                            glyph: g.id,
                            size_q: (size_px * 4.0).round() as u32,
                            subpx,
                        };
                        let Some(e) = self.atlas.get(device, queue, key, data)? else {
                            continue;
                        };
                        let x = ix + e.left as f32;
                        let y = gy.round() - e.top as f32;
                        push(
                            &mut self.batches,
                            Kind::Glyphs,
                            self.glyphs.len() as u32,
                            scissor,
                            None,
                        );
                        if current.is_some() {
                            mark(&mut dirty, [x, y, x + e.width as f32, y + e.height as f32]);
                        }
                        self.glyphs.push(GlyphInstance {
                            rect: [x, y, e.width as f32, e.height as f32],
                            uv: [
                                e.x as f32 / atlas_size,
                                e.y as f32 / atlas_size,
                                (e.x + e.width) as f32 / atlas_size,
                                (e.y + e.height) as f32 / atlas_size,
                            ],
                            color,
                        });
                    }
                }
                DrawCommand::PushClip { rect, .. } => {
                    // Étape 1 : découpage rectangulaire (la forme exacte viendra avec les masques).
                    let r = transform_rect(&t, rect, s);
                    clips.push(intersect(scissor, r, full));
                }
                DrawCommand::PopClip => {
                    if clips.len() > 1 {
                        clips.pop();
                    }
                }
                DrawCommand::PushTransform(m) => transforms.push(m.then(&t)),
                DrawCommand::PopTransform => {
                    if transforms.len() > 1 {
                        transforms.pop();
                    }
                }
                // Étape 1 : l'opacité multiplie chaque élément (pas encore de calque composé à part).
                DrawCommand::PushLayer { opacity: o } => opacity.push(alpha * o),
                DrawCommand::PopLayer => {
                    if opacity.len() > 1 {
                        opacity.pop();
                    }
                }
                DrawCommand::Backdrop { shape, rect, effect } => {
                    let BackdropEffect::Blur(blur) = effect else {
                        continue;
                    };
                    let b = transform_bounds(&t, rect, 1.0, s);
                    let clip = [
                        scissor[0] as f32,
                        scissor[1] as f32,
                        (scissor[0] + scissor[2]) as f32,
                        (scissor[1] + scissor[3]) as f32,
                    ];
                    let visible = [
                        b[0].max(clip[0]),
                        b[1].max(clip[1]),
                        b[2].min(clip[2]),
                        b[3].min(clip[3]),
                    ];
                    if visible[2] <= visible[0] || visible[3] <= visible[1] || alpha <= 0.0 {
                        continue;
                    }
                    let ts = (t.a * t.d - t.b * t.c).abs().sqrt().max(1e-4);
                    let reach = effect.reach() * s * ts + 2.0;
                    let region = [
                        (visible[0] - reach).max(0.0).floor(),
                        (visible[1] - reach).max(0.0).floor(),
                        (visible[2] + reach).min(fw).ceil(),
                        (visible[3] + reach).min(fh).ceil(),
                    ];
                    let mut lf = backdrop::blur_level(blur.radius.max(0.0) * s * ts);
                    if blur.low_quality {
                        lf = lf.round();
                    }
                    let levels = (lf.ceil() as u32).min(backdrop::LEVELS as u32 - 1);
                    // Partage la capture en cours si rien n'a été dessiné sur cette zone depuis.
                    let reuse = current.filter(|_| !dirty.iter().any(|d| overlaps(d, &region)));
                    let (c, capture) = match reuse {
                        Some(c) => {
                            let cap = &mut self.captures[c];
                            cap.region = union(&cap.region, &region);
                            cap.levels = cap.levels.max(levels);
                            (c, None)
                        }
                        None => {
                            self.captures.push(Capture { region, levels });
                            dirty.clear();
                            let c = self.captures.len() - 1;
                            current = Some(c);
                            (c, Some(c))
                        }
                    };
                    // La composition change ces pixels : un effet suivant dessus devra recapturer.
                    mark(&mut dirty, visible);
                    let (grect, radii, kind) = geometry(shape, rect);
                    let tint = veka_foundation::Color { a: 1.0, ..blur.tint }.to_linear_premultiplied();
                    let index = self.effects.len() as u32;
                    if capture.is_some() {
                        self.batches.push(Batch {
                            kind: Kind::Effects,
                            range: index..index + 1,
                            scissor,
                            capture,
                        });
                    } else {
                        push(&mut self.batches, Kind::Effects, index, scissor, None);
                    }
                    self.effects.push(EffectInstance {
                        rect: [grect.x(), grect.y(), grect.width(), grect.height()],
                        radii,
                        xform: [t.a * s, t.b * s, t.c * s, t.d * s],
                        extra: [t.tx * s, t.ty * s, kind, alpha],
                        params: [
                            lf,
                            blur.saturation,
                            if blur.low_quality { 0.0 } else { blur.noise },
                            if blur.low_quality { 1.0 } else { 0.0 },
                        ],
                        tint: [
                            tint[0],
                            tint[1],
                            tint[2],
                            (blur.tint_amount * blur.tint.a).clamp(0.0, 1.0),
                        ],
                        region,
                    });
                    self.effect_capture.push(c);
                }
            }
        }
        // Chaque effet lit toute la zone de sa capture (agrandie par les effets qui la partagent).
        for (e, &c) in self.effects.iter_mut().zip(&self.effect_capture) {
            e.region = self.captures[c].region;
        }
        Ok(())
    }
}

/// Un atlas est plein pendant la préparation.
enum Overflow {
    Glyphs,
    Ramps,
}

impl From<Full> for Overflow {
    fn from(_: Full) -> Self {
        Overflow::Glyphs
    }
}

impl From<RampsFull> for Overflow {
    fn from(_: RampsFull) -> Self {
        Overflow::Ramps
    }
}

fn push(batches: &mut Vec<Batch>, kind: Kind, index: u32, scissor: [u32; 4], capture: Option<usize>) {
    if capture.is_none()
        && let Some(b) = batches.last_mut()
        && b.kind == kind
        && b.scissor == scissor
        && b.range.end == index
    {
        b.range.end += 1;
        return;
    }
    batches.push(Batch {
        kind,
        range: index..index + 1,
        scissor,
        capture,
    });
}

/// Ajoute une zone dessinée ; au-delà de 64, elles sont fusionnées (test plus grossier mais borné).
fn mark(dirty: &mut Vec<[f32; 4]>, r: [f32; 4]) {
    if dirty.len() >= 64 {
        let all = dirty.iter().fold(r, |acc, d| union(&acc, d));
        dirty.clear();
        dirty.push(all);
    } else {
        dirty.push(r);
    }
}

fn overlaps(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

fn union(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// Boîte englobante en pixels de `r` agrandi de `margin` points, transformé (quatre coins).
fn transform_bounds(t: &Transform2D, r: &Rect, margin: f32, s: f32) -> [f32; 4] {
    let (x0, y0, x1, y1) = (
        r.x() - margin,
        r.y() - margin,
        r.max_x() + margin,
        r.max_y() + margin,
    );
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        let p = t.apply(veka_foundation::Point::new(x, y));
        b = [
            b[0].min(p.x * s),
            b[1].min(p.y * s),
            b[2].max(p.x * s),
            b[3].max(p.y * s),
        ];
    }
    b
}

/// Rectangle, rayons des coins et sorte (0 boîte, 1 ellipse) d'une forme pour les shaders.
fn geometry(shape: &Shape, rect: &Rect) -> (Rect, [f32; 4], f32) {
    // Le cercle est dessiné comme une boîte carrée centrée aux coins entièrement arrondis.
    let rect = if matches!(shape, Shape::Circle) {
        rect.centered(veka_foundation::Size::new(
            rect.size.min_side(),
            rect.size.min_side(),
        ))
    } else {
        *rect
    };
    let r = shape.corner_radii(&rect);
    let kind = if matches!(shape, Shape::Ellipse) { 1.0 } else { 0.0 };
    (
        rect,
        [r.top_left, r.top_right, r.bottom_right, r.bottom_left],
        kind,
    )
}

fn paint_color(p: &Paint) -> veka_foundation::Color {
    match p {
        // Un jeton n'aurait pas dû arriver jusqu'ici : bien visible.
        Paint::Token(..) => veka_colora::UNKNOWN,
        p => p.average(),
    }
}

fn premul(c: veka_foundation::Color, alpha: f32) -> [f32; 4] {
    c.opacity(alpha).to_linear_premultiplied()
}

/// Position d'une rampe pour le shader : (x du premier texel, v du milieu de la ligne).
fn ramp_uv(slot: ramps::Slot) -> (f32, f32) {
    (slot.x as f32, (slot.y as f32 + 0.5) / ramps::SIZE as f32)
}

#[allow(clippy::too_many_arguments)]
fn shape_instance(
    atlas: &mut Ramps,
    queue: &wgpu::Queue,
    shape: &Shape,
    rect: &Rect,
    paint: &Paint,
    t: &Transform2D,
    s: f32,
    alpha: f32,
    stroke: f32,
) -> Result<ShapeInstance, RampsFull> {
    let (rect, radii, kind) = geometry(shape, rect);
    let gradient_alpha = [alpha; 4];
    let (color, fill, geo, geo2) = match paint {
        Paint::Linear(g) => {
            let (u, v) = ramp_uv(atlas.ramp(queue, &g.gradient)?);
            let (a, b) = (g.start.in_rect(&rect), g.end.in_rect(&rect));
            (gradient_alpha, [1.0, u, v, 0.0], [a.x, a.y, b.x, b.y], [0.0; 4])
        }
        Paint::Radial(g) => {
            let (u, v) = ramp_uv(atlas.ramp(queue, &g.gradient)?);
            let c = g.center.in_rect(&rect);
            (
                gradient_alpha,
                [2.0, u, v, 0.0],
                [c.x, c.y, g.start_radius, g.end_radius],
                [0.0; 4],
            )
        }
        Paint::Elliptical(g) => {
            let (u, v) = ramp_uv(atlas.ramp(queue, &g.gradient)?);
            let c = g.center.in_rect(&rect);
            (
                gradient_alpha,
                [3.0, u, v, 0.0],
                [c.x, c.y, rect.width() * 0.5, rect.height() * 0.5],
                [g.start_fraction, g.end_fraction, 0.0, 0.0],
            )
        }
        Paint::Angular(g) => {
            let (u, v) = ramp_uv(atlas.ramp(queue, &g.gradient)?);
            let c = g.center.in_rect(&rect);
            (
                gradient_alpha,
                [4.0, u, v, 0.0],
                [
                    c.x,
                    c.y,
                    g.start_angle.to_radians(),
                    (g.end_angle - g.start_angle).to_radians(),
                ],
                [0.0; 4],
            )
        }
        Paint::Mesh(m) if m.is_valid() => {
            let slot = atlas.mesh(queue, m)?;
            let n = ramps::SIZE as f32;
            // Une demi-cellule de marge pour ne pas lire la voisine.
            let (x0, y0) = (slot.x as f32 + 0.5, slot.y as f32 + 0.5);
            let (x1, y1) = (
                (slot.x + ramps::MESH) as f32 - 0.5,
                (slot.y + ramps::MESH) as f32 - 0.5,
            );
            (
                gradient_alpha,
                [5.0, 0.0, 0.0, 0.0],
                [rect.x(), rect.y(), rect.width(), rect.height()],
                [x0 / n, y0 / n, x1 / n, y1 / n],
            )
        }
        p => (premul(paint_color(p), alpha), [0.0; 4], [0.0; 4], [0.0; 4]),
    };
    Ok(ShapeInstance {
        rect: [rect.x(), rect.y(), rect.width(), rect.height()],
        radii,
        color,
        xform: [t.a * s, t.b * s, t.c * s, t.d * s],
        extra: [t.tx * s, t.ty * s, kind, stroke],
        fill,
        geo,
        geo2,
    })
}

fn transform_rect(t: &Transform2D, r: &Rect, s: f32) -> [f32; 4] {
    let a = t.apply(r.origin);
    let b = t.apply(veka_foundation::Point::new(r.max_x(), r.max_y()));
    [
        a.x.min(b.x) * s,
        a.y.min(b.y) * s,
        a.x.max(b.x) * s,
        a.y.max(b.y) * s,
    ]
}

fn intersect(cur: [u32; 4], r: [f32; 4], full: [u32; 4]) -> [u32; 4] {
    let x0 = (r[0].floor().max(0.0) as u32).max(cur[0]);
    let y0 = (r[1].floor().max(0.0) as u32).max(cur[1]);
    let x1 = (r[2].ceil().max(0.0) as u32).min(cur[0] + cur[2]).min(full[2]);
    let y1 = (r[3].ceil().max(0.0) as u32).min(cur[1] + cur[3]).min(full[3]);
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

/// Copie des instances dans un tampon (agrandi si besoin). `None` s'il n'y a rien.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slot: &mut Option<wgpu::Buffer>,
    bytes: &[u8],
    label: &str,
) -> Option<wgpu::Buffer> {
    if bytes.is_empty() {
        return None;
    }
    if slot.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
        *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (bytes.len() as u64).next_power_of_two().max(4096),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    let buf = slot.as_ref().expect("tampon").clone();
    queue.write_buffer(&buf, 0, bytes);
    Some(buf)
}
