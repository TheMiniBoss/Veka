//! Rendu dans une fenêtre : surface wgpu créée depuis les handles de la plateforme.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use veka_graphics::DrawList;
use veka_render::{FrameInfo, RenderError, Renderer};

use crate::{Gpu, Painter};

/// Pour passer une fenêtre de taille inconnue (`&dyn PlatformWindow`) à wgpu, qui veut un type sized.
struct ByRef<'a, W: ?Sized>(&'a W);

impl<W: HasWindowHandle + ?Sized> HasWindowHandle for ByRef<'_, W> {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.0.window_handle()
    }
}

impl<W: HasDisplayHandle + ?Sized> HasDisplayHandle for ByRef<'_, W> {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.0.display_handle()
    }
}

/// Renderer d'une fenêtre.
pub struct WindowRenderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    painter: Painter,
    frame: FrameInfo,
}

impl std::fmt::Debug for WindowRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowRenderer")
            .field("gpu", &self.gpu)
            .field("frame", &self.frame)
            .finish()
    }
}

impl WindowRenderer {
    /// Crée le renderer d'une fenêtre (bloque le temps d'ouvrir le GPU).
    ///
    /// # Safety
    ///
    /// La fenêtre désignée par `window` doit rester valide tant que ce renderer existe.
    pub unsafe fn new<W: HasWindowHandle + HasDisplayHandle + ?Sized>(
        window: &W,
        frame: FrameInfo,
    ) -> Result<Self, RenderError> {
        // SAFETY : garanti par l'appelant (la fenêtre vit plus longtemps que le renderer).
        unsafe { pollster::block_on(Self::new_async(window, frame)) }
    }

    /// Version asynchrone (nécessaire sur le web).
    ///
    /// # Safety
    ///
    /// Comme [`new`](Self::new).
    pub async unsafe fn new_async<W: HasWindowHandle + HasDisplayHandle + ?Sized>(
        window: &W,
        frame: FrameInfo,
    ) -> Result<Self, RenderError> {
        let instance = Gpu::default_instance();
        // SAFETY : la fenêtre reste valide tant que le renderer existe (contrat de la fonction).
        let target =
            unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(&ByRef(window), &ByRef(window)) }
                .map_err(|e| RenderError::Other(e.to_string()))?;
        // SAFETY : la fenêtre reste valide tant que le renderer existe (contrat de la fonction).
        let surface = unsafe { instance.create_surface_unsafe(target) }
            .map_err(|e| RenderError::Other(e.to_string()))?;
        let gpu = Gpu::new(instance, Some(&surface)).await?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps.alpha_modes[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: frame.width.max(1),
            height: frame.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let painter = Painter::new(&gpu.device, format);
        Ok(Self {
            gpu,
            surface,
            config,
            painter,
            frame,
        })
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }
}

impl Renderer for WindowRenderer {
    fn resize(&mut self, frame: FrameInfo) {
        self.frame = frame;
        let (w, h) = (frame.width.max(1), frame.height.max(1));
        if (w, h) != (self.config.width, self.config.height) {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.gpu.device, &self.config);
        }
    }

    fn render(&mut self, list: &DrawList) -> Result<(), RenderError> {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return Err(RenderError::Lost);
            }
            _ => return Err(RenderError::Lost),
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let frame = FrameInfo {
            width: self.config.width,
            height: self.config.height,
            scale: self.frame.scale,
        };
        self.painter
            .draw(&self.gpu.device, &self.gpu.queue, list, frame, &view);
        self.gpu.queue.present(texture);
        Ok(())
    }
}
