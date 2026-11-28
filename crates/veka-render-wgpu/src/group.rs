//! Calques (étape 8) : un groupe (`blend_mode`, `compositing_group`) est dessiné dans une texture à part,
//! puis composé sur ce qui est derrière lui (`group.wgsl`). La zone de derrière est lue dans la capture,
//! comme pour le verre.

use bytemuck::{Pod, Zeroable};

/// Calques imbriqués au plus ; au-delà, un groupe est dessiné directement (sans mélange).
pub(crate) const MAX_DEPTH: usize = 8;

/// Un calque à composer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GroupInstance {
    /// Zone, en pixels : min.xy, max.xy.
    pub rect: [f32; 4],
    /// Mode, opacité.
    pub params: [f32; 4],
}

const GROUP_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4];

pub(crate) struct Groups {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,
    size: (u32, u32),
    /// Textures des calques, par profondeur (la première est la profondeur 1).
    layers: Vec<(wgpu::Texture, wgpu::TextureView)>,
    /// Par calque : sa texture et la capture de derrière.
    bind_groups: Vec<wgpu::BindGroup>,
    pub instance_buf: Option<wgpu::Buffer>,
}

impl Groups {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("veka calques"),
            source: wgpu::ShaderSource::Wgsl(include_str!("group.wgsl").into()),
        });
        let entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("veka calques"),
            entries: &[entry(0), entry(1)],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("veka calques"),
            bind_group_layouts: &[Some(globals_layout), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = crate::painter::pipeline(
            device,
            &module,
            &pl,
            "group_vs",
            "group_fs",
            &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GroupInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &GROUP_ATTRS,
            })],
            format,
            // Le shader calcule le résultat complet (fond compris).
            None,
        );
        Self {
            format,
            layout,
            pipeline,
            size: (0, 0),
            layers: Vec::new(),
            bind_groups: Vec::new(),
            instance_buf: None,
        }
    }

    /// Les calques jusqu'à `depth`, à la taille de la fenêtre. `backdrop` : le niveau net de la capture.
    pub fn ensure(
        &mut self,
        device: &wgpu::Device,
        w: u32,
        h: u32,
        depth: usize,
        backdrop: &wgpu::TextureView,
    ) {
        if self.size != (w, h) {
            self.layers.clear();
            self.bind_groups.clear();
            self.size = (w, h);
        }
        while self.layers.len() < depth {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("veka calque"),
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
            self.layers.push((tex, view));
        }
        // La capture est recréée avec la fenêtre : les liaisons aussi, à chaque image (peu coûteux).
        self.bind_groups = self
            .layers
            .iter()
            .map(|(_, view)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("veka calque"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(backdrop),
                        },
                    ],
                })
            })
            .collect();
    }

    /// La texture du calque de profondeur `depth` (à partir de 1).
    pub fn layer(&self, depth: usize) -> Option<&(wgpu::Texture, wgpu::TextureView)> {
        self.layers.get(depth.checked_sub(1)?)
    }

    pub fn bind_group(&self, depth: usize) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(depth.checked_sub(1)?)
    }
}
