//! Lecture de l'arrière-plan (étape 6) : captures, pyramide de flou et composition des effets.
//!
//! Le painter coupe son dessin aux captures. Une capture copie une zone de la texture intermédiaire dans
//! le niveau 0 de la pyramide, puis calcule les niveaux flous dont ses effets ont besoin. Plusieurs effets
//! partagent une capture tant que rien n'a été dessiné sur leur zone depuis (VekaUI 4a).

use bytemuck::{Pod, Zeroable};

/// Niveaux de la pyramide : 0 (net) à 7 (1/128, flou d'environ 200 pixels).
pub(crate) const LEVELS: usize = 8;

/// Flou d'un niveau k ≈ `SIGMA_PER_LEVEL` · 2^k pixels (réduction puis gaussienne de 1,5 texel).
pub(crate) const SIGMA_PER_LEVEL: f32 = 1.6;

/// Un effet à composer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct EffectInstance {
    pub rect: [f32; 4],
    pub radii: [f32; 4],
    pub xform: [f32; 4],
    /// tx, ty, sorte (0 boîte, 1 ellipse), opacité.
    pub extra: [f32; 4],
    /// Niveau de flou (fractionnaire), saturation, grain, qualité réduite (1).
    pub params: [f32; 4],
    /// Teinte linéaire ; a = part de la teinte.
    pub tint: [f32; 4],
    /// Zone capturée, en pixels : min.xy, max.xy.
    pub region: [f32; 4],
    /// Ondulation : force (pixels), 1 / taille des vagues (pixels), temps.
    pub flow: [f32; 4],
}

const EFFECT_ATTRS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
    4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
];

/// Un verre liquide à composer (ses formes sont dans [`GlassShapeGpu`]).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GlassInstance {
    /// Boîte de toutes les formes, repère local.
    pub rect: [f32; 4],
    pub xform: [f32; 4],
    /// tx, ty, première forme, nombre de formes.
    pub extra: [f32; 4],
    /// Niveau de flou, réfraction (px), largeur du bord (px), aberration.
    pub p1: [f32; 4],
    /// Reflet, fusion (repère local), saturation, opacité.
    pub p2: [f32; 4],
    pub tint: [f32; 4],
    pub region: [f32; 4],
    /// Pointeur (px), pointeur présent (1), qualité réduite (1).
    pub light: [f32; 4],
    /// Bord sombre, lumière interne, ombre, rayon de l'ombre (px).
    pub p3: [f32; 4],
}

const GLASS_ATTRS: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
    4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4
];

/// Une forme d'un verre, lue par le shader dans un tampon de stockage.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GlassShapeGpu {
    pub rect: [f32; 4],
    pub radii: [f32; 4],
    pub kind: [f32; 4],
}

/// Une capture : la zone copiée (pixels entiers) et le plus haut niveau de flou demandé.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Capture {
    pub region: [f32; 4],
    pub levels: u32,
}

impl Capture {
    fn pixels(&self, w: u32, h: u32) -> [u32; 4] {
        let x0 = (self.region[0].floor().max(0.0) as u32).min(w);
        let y0 = (self.region[1].floor().max(0.0) as u32).min(h);
        let x1 = (self.region[2].ceil().max(0.0) as u32).min(w);
        let y1 = (self.region[3].ceil().max(0.0) as u32).min(h);
        [x0, y0, x1, y1]
    }
}

/// Le niveau de flou (fractionnaire) d'un rayon en pixels.
/// Niveau k ≈ flou de `SIGMA_PER_LEVEL` · 2^k pixels ; entre deux niveaux, on mélange.
pub(crate) fn blur_level(radius_px: f32) -> f32 {
    if radius_px <= SIGMA_PER_LEVEL {
        return 0.0;
    }
    (radius_px / SIGMA_PER_LEVEL)
        .log2()
        .clamp(0.0, (LEVELS - 1) as f32)
}

/// Uniformes d'une passe de la pyramide, à 256 octets (alignement des décalages dynamiques).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PassUniform {
    src_size: [f32; 2],
    dir: [f32; 2],
    bounds: [f32; 4],
    _pad: [f32; 56],
}

const PASS_STRIDE: u64 = std::mem::size_of::<PassUniform>() as u64;

struct Pyramid {
    size: (u32, u32),
    /// Niveau 0 (copie) puis les niveaux flous.
    levels: Vec<(wgpu::Texture, wgpu::TextureView)>,
    /// Résultat intermédiaire du flou horizontal (index 0 inutilisé).
    temps: Vec<(wgpu::Texture, wgpu::TextureView)>,
    level_bg: Vec<wgpu::BindGroup>,
    temp_bg: Vec<wgpu::BindGroup>,
    effect_bg: wgpu::BindGroup,
}

/// Pipelines et textures des effets d'arrière-plan.
pub(crate) struct Backdrop {
    format: wgpu::TextureFormat,
    tex_layout: wgpu::BindGroupLayout,
    pass_layout: wgpu::BindGroupLayout,
    levels_layout: wgpu::BindGroupLayout,
    down_pipeline: wgpu::RenderPipeline,
    gauss_pipeline: wgpu::RenderPipeline,
    pub effect_pipeline: wgpu::RenderPipeline,
    pub glass_pipeline: wgpu::RenderPipeline,
    shapes_layout: wgpu::BindGroupLayout,
    shapes: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    pub glass_buf: Option<wgpu::Buffer>,
    sampler: wgpu::Sampler,
    pass_buf: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    pyramid: Option<Pyramid>,
    /// Passes de la pyramide par capture, pour le décalage de leurs uniformes.
    pass_base: Vec<u32>,
    pub instance_buf: Option<wgpu::Buffer>,
}

impl Backdrop {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let pass_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("veka pyramide"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop_pass.wgsl").into()),
        });
        let effect_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("veka effets"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop_effect.wgsl").into()),
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("veka pyramide : source"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });
        let pass_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("veka pyramide : passe"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(PASS_STRIDE),
                },
                count: None,
            }],
        });
        let mut entries: Vec<_> = (0..LEVELS as u32).map(texture_entry).collect();
        entries.push(sampler_entry(LEVELS as u32));
        let levels_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("veka pyramide : niveaux"),
            entries: &entries,
        });
        let pass_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka pyramide"),
            bind_group_layouts: &[Some(&tex_layout), Some(&pass_layout)],
            immediate_size: 0,
        });
        let effect_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka effets"),
            bind_group_layouts: &[Some(globals_layout), Some(&levels_layout)],
            immediate_size: 0,
        });
        let shapes_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("veka verre : formes"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let glass_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka verre"),
            bind_group_layouts: &[Some(globals_layout), Some(&levels_layout), Some(&shapes_layout)],
            immediate_size: 0,
        });
        let glass_pipeline = crate::painter::pipeline(
            device,
            &effect_module,
            &glass_pl,
            "glass_vs",
            "glass_fs",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlassInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &GLASS_ATTRS,
            })],
            format,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let pass_pipeline =
            |fs| crate::painter::pipeline(device, &pass_module, &pass_pl, "full_vs", fs, &[], format, None);
        let down_pipeline = pass_pipeline("down_fs");
        let gauss_pipeline = pass_pipeline("gauss_fs");
        let effect_pipeline = crate::painter::pipeline(
            device,
            &effect_module,
            &effect_pl,
            "effect_vs",
            "effect_fs",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<EffectInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &EFFECT_ATTRS,
            })],
            format,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("veka pyramide"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            format,
            tex_layout,
            pass_layout,
            levels_layout,
            down_pipeline,
            gauss_pipeline,
            effect_pipeline,
            glass_pipeline,
            shapes_layout,
            shapes: None,
            glass_buf: None,
            sampler,
            pass_buf: None,
            pyramid: None,
            pass_base: Vec::new(),
            instance_buf: None,
        }
    }

    /// Les textures de la pyramide, à la taille de la fenêtre (créées au premier effet).
    fn ensure(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        if self.pyramid.as_ref().is_some_and(|p| p.size == (w, h)) {
            return;
        }
        let texture = |label, k: usize, usage| {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.div_ceil(1 << k).max(1),
                    height: h.div_ceil(1 << k).max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            (tex, view)
        };
        let levels: Vec<_> = (0..LEVELS)
            .map(|k| {
                let usage = wgpu::TextureUsages::TEXTURE_BINDING
                    | if k == 0 {
                        wgpu::TextureUsages::COPY_DST
                    } else {
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                    };
                texture("veka pyramide", k, usage)
            })
            .collect();
        let temps: Vec<_> = (0..LEVELS)
            .map(|k| {
                texture(
                    "veka pyramide (flou horizontal)",
                    k.max(1),
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                )
            })
            .collect();
        let source = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("veka pyramide : source"),
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let level_bg = levels.iter().map(|(_, v)| source(v)).collect();
        let temp_bg = temps.iter().map(|(_, v)| source(v)).collect();
        let mut entries: Vec<_> = levels
            .iter()
            .enumerate()
            .map(|(k, (_, v))| wgpu::BindGroupEntry {
                binding: k as u32,
                resource: wgpu::BindingResource::TextureView(v),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: LEVELS as u32,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        });
        let effect_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("veka pyramide : niveaux"),
            layout: &self.levels_layout,
            entries: &entries,
        });
        self.pyramid = Some(Pyramid {
            size: (w, h),
            levels,
            temps,
            level_bg,
            temp_bg,
            effect_bg,
        });
    }

    /// Prépare les textures et les uniformes de toutes les captures de l'image.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        captures: &[Capture],
        w: u32,
        h: u32,
    ) {
        self.pass_base.clear();
        if captures.is_empty() {
            return;
        }
        self.ensure(device, w, h);
        let mut passes = Vec::new();
        for c in captures {
            self.pass_base.push(passes.len() as u32);
            let px = c.pixels(w, h);
            for k in 1..=c.levels as usize {
                // Réduction : lit le niveau k-1 dans la zone capturée.
                let src = k - 1;
                passes.push(PassUniform {
                    src_size: level_size(w, h, src),
                    dir: [0.0, 0.0],
                    bounds: level_bounds(px, src),
                    _pad: [0.0; 56],
                });
            }
            for k in 1..=c.levels as usize {
                for dir in [[1.0, 0.0], [0.0, 1.0]] {
                    passes.push(PassUniform {
                        src_size: level_size(w, h, k),
                        dir,
                        bounds: level_bounds(px, k),
                        _pad: [0.0; 56],
                    });
                }
            }
        }
        if passes.is_empty() {
            passes.push(PassUniform::zeroed());
        }
        let bytes: &[u8] = bytemuck::cast_slice(&passes);
        if self
            .pass_buf
            .as_ref()
            .is_none_or(|(b, _)| b.size() < bytes.len() as u64)
        {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("veka pyramide : passes"),
                size: (bytes.len() as u64).next_power_of_two().max(PASS_STRIDE * 16),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("veka pyramide : passes"),
                layout: &self.pass_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buf,
                        offset: 0,
                        size: wgpu::BufferSize::new(PASS_STRIDE),
                    }),
                }],
            });
            self.pass_buf = Some((buf, bg));
        }
        queue.write_buffer(&self.pass_buf.as_ref().expect("passes").0, 0, bytes);
    }

    /// Copie la zone de la capture `index` depuis `source` et calcule ses niveaux flous.
    pub fn capture(
        &self,
        enc: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        captures: &[Capture],
        index: usize,
    ) {
        let (Some(p), Some((_, pass_bg))) = (&self.pyramid, &self.pass_buf) else {
            return;
        };
        let c = &captures[index];
        let (w, h) = p.size;
        let px = c.pixels(w, h);
        if px[2] <= px[0] || px[3] <= px[1] {
            return;
        }
        enc.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: source,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: px[0],
                    y: px[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &p.levels[0].0,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: px[0],
                    y: px[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: px[2] - px[0],
                height: px[3] - px[1],
                depth_or_array_layers: 1,
            },
        );
        let mut pass_index = self.pass_base[index];
        let n = c.levels as usize;
        let mut run = |enc: &mut wgpu::CommandEncoder,
                       pipeline: &wgpu::RenderPipeline,
                       target: &wgpu::TextureView,
                       src_bg: &wgpu::BindGroup,
                       k: usize| {
            let b = level_bounds(px, k);
            // Une marge d'un texel : le niveau suivant lit aussi les bords.
            let x0 = (b[0] as u32).saturating_sub(1);
            let y0 = (b[1] as u32).saturating_sub(1);
            let (lw, lh) = (w.div_ceil(1 << k).max(1), h.div_ceil(1 << k).max(1));
            let x1 = (b[2] as u32 + 1).min(lw);
            let y1 = (b[3] as u32 + 1).min(lh);
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("veka pyramide"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, src_bg, &[]);
            pass.set_bind_group(1, pass_bg, &[pass_index * PASS_STRIDE as u32]);
            pass.set_scissor_rect(x0, y0, x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
            pass.draw(0..3, 0..1);
            pass_index += 1;
        };
        for k in 1..=n {
            run(enc, &self.down_pipeline, &p.levels[k].1, &p.level_bg[k - 1], k);
        }
        for k in 1..=n {
            run(enc, &self.gauss_pipeline, &p.temps[k].1, &p.level_bg[k], k);
            run(enc, &self.gauss_pipeline, &p.levels[k].1, &p.temp_bg[k], k);
        }
    }

    /// Envoie les formes des verres de l'image.
    pub fn upload_shapes(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, shapes: &[GlassShapeGpu]) {
        if shapes.is_empty() {
            return;
        }
        let bytes: &[u8] = bytemuck::cast_slice(shapes);
        if self
            .shapes
            .as_ref()
            .is_none_or(|(b, _)| b.size() < bytes.len() as u64)
        {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("veka verre : formes"),
                size: (bytes.len() as u64).next_power_of_two().max(1024),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("veka verre : formes"),
                layout: &self.shapes_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            self.shapes = Some((buf, bg));
        }
        queue.write_buffer(&self.shapes.as_ref().expect("formes").0, 0, bytes);
    }

    pub fn shapes_bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.shapes.as_ref().map(|(_, bg)| bg)
    }

    pub fn effect_bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.pyramid.as_ref().map(|p| &p.effect_bg)
    }
}

fn level_size(w: u32, h: u32, k: usize) -> [f32; 2] {
    [w.div_ceil(1 << k).max(1) as f32, h.div_ceil(1 << k).max(1) as f32]
}

/// La zone `px` (pixels) au niveau `k`, en texels entiers.
fn level_bounds(px: [u32; 4], k: usize) -> [f32; 4] {
    let f = (1u32 << k) as f32;
    [
        (px[0] as f32 / f).floor(),
        (px[1] as f32 / f).floor(),
        (px[2] as f32 / f).ceil(),
        (px[3] as f32 / f).ceil(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_radius() {
        assert_eq!(blur_level(0.0), 0.0);
        assert!((blur_level(SIGMA_PER_LEVEL * 2.0) - 1.0).abs() < 1e-4);
        assert!((blur_level(SIGMA_PER_LEVEL * 8.0) - 3.0).abs() < 1e-4);
        assert_eq!(blur_level(1e6), (LEVELS - 1) as f32);
        // Continu (animable) autour du seuil.
        let a = blur_level(SIGMA_PER_LEVEL - 1e-3);
        let b = blur_level(SIGMA_PER_LEVEL + 1e-3);
        assert!((a - b).abs() < 0.01, "{a} {b}");
    }

    #[test]
    fn bounds_cover_the_region() {
        assert_eq!(level_bounds([3, 5, 17, 9], 2), [0.0, 1.0, 5.0, 3.0]);
    }
}
