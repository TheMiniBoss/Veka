//! # Veka Render wgpu
//!
//! Le premier renderer de Veka : DirectX 12 sur Windows, Metal sur macOS (Vulkan ailleurs).
//!
//! - Formes : quads instanciés, **SDF analytique** dans le shader (mêmes formules que
//!   `veka-graphics`), antialiasing par la dérivée de la distance.
//! - Texte : glyphes rastérisés par `swash` dans un atlas.
//! - Tout est dessiné dans une **texture intermédiaire** puis copié vers la cible : c'est ce qui
//!   permettra aux effets (`Backdrop`, verre) de lire ce qui est déjà dessiné.

mod atlas;
mod painter;
mod ramps;
mod window;

pub use painter::Painter;
pub use veka_render::{FrameInfo, RenderError, Renderer};
pub use window::{SurfaceOptions, WindowRenderer};

/// Instance, adaptateur, appareil et file du GPU.
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("adapter", &self.adapter.get_info().name)
            .finish()
    }
}

impl Gpu {
    /// Ouvre le GPU. `surface` : une surface que l'adaptateur doit pouvoir afficher.
    pub async fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: surface,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| RenderError::NoAdapter(e.to_string()))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("veka"),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::Other(e.to_string()))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Instance avec les backends natifs de l'OS (DX12, Metal, Vulkan).
    pub fn default_instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        })
    }

    /// Instance pour une fenêtre transparente : sur Windows, DX12 présente par DirectComposition.
    pub fn transparent_instance() -> wgpu::Instance {
        let mut desc = wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        };
        desc.backend_options.dx12.presentation_system = wgpu::Dx12SwapchainKind::DxgiFromVisual;
        wgpu::Instance::new(desc)
    }

    /// GPU sans fenêtre (tests, rendu hors écran).
    pub fn headless() -> Result<Self, RenderError> {
        pollster::block_on(Self::new(Self::default_instance(), None))
    }
}

/// Dessine `list` hors écran et renvoie l'image en RGBA 8 bits (sRGB), ligne par ligne.
pub fn render_to_rgba(gpu: &Gpu, list: &veka_graphics::DrawList, frame: FrameInfo) -> Vec<u8> {
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut painter = Painter::new(&gpu.device, format);
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("veka hors écran"),
        size: wgpu::Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    painter.draw(&gpu.device, &gpu.queue, list, frame, &view);

    let row = frame.width * 4;
    let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("veka lecture"),
        size: (padded * frame.height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(frame.height),
            },
        },
        wgpu::Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([enc.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("lecture de l'image"));
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("attente du GPU");
    let data = slice.get_mapped_range().expect("accès à l'image");
    let mut out = Vec::with_capacity((row * frame.height) as usize);
    for y in 0..frame.height {
        let start = (y * padded) as usize;
        out.extend_from_slice(&data[start..start + row as usize]);
    }
    out
}
