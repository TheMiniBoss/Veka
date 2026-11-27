//! Atlas de glyphes : rastérisés par swash, rangés en étagères dans une texture R8.

use std::collections::HashMap;

use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::{Format, Vector};

/// Clé d'un glyphe rastérisé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub font: u64,
    pub index: u32,
    pub glyph: u32,
    /// Taille en pixels × 4.
    pub size_q: u32,
    /// Décalage horizontal sous le pixel, en quarts.
    pub subpx: u8,
}

/// Où est un glyphe dans l'atlas, et comment le placer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

pub(crate) struct Atlas {
    pub size: u32,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    entries: HashMap<GlyphKey, Option<GlyphEntry>>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_h: u32,
    scaler: ScaleContext,
}

/// L'atlas est plein : il a été agrandi et vidé, l'image doit refaire ses glyphes.
pub(crate) struct Full;

const PAD: u32 = 1;

impl Atlas {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("veka atlas de glyphes"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            size,
            texture,
            view,
            entries: HashMap::new(),
            shelf_x: 0,
            shelf_y: 0,
            shelf_h: 0,
            scaler: ScaleContext::new(),
        }
    }

    /// Le glyphe (rastérisé au besoin), ou `None` s'il n'a pas de pixels (espace).
    pub fn get(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: GlyphKey,
        data: &[u8],
    ) -> Result<Option<GlyphEntry>, Full> {
        if let Some(e) = self.entries.get(&key) {
            return Ok(*e);
        }
        let Some(font) = swash::FontRef::from_index(data, key.index as usize) else {
            self.entries.insert(key, None);
            return Ok(None);
        };
        let mut scaler = self
            .scaler
            .builder(font)
            .size(key.size_q as f32 / 4.0)
            .hint(false)
            .build();
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(Vector::new(key.subpx as f32 / 4.0, 0.0))
        .render(&mut scaler, key.glyph as u16);
        let Some(image) = image.filter(|i| i.placement.width > 0 && i.placement.height > 0) else {
            self.entries.insert(key, None);
            return Ok(None);
        };
        let (w, h) = (image.placement.width, image.placement.height);
        let Some((x, y)) = self.allocate(w, h) else {
            self.grow(device);
            return Err(Full);
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &image.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let e = GlyphEntry {
            x,
            y,
            width: w,
            height: h,
            left: image.placement.left,
            top: image.placement.top,
        };
        self.entries.insert(key, Some(e));
        Ok(Some(e))
    }

    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w + PAD > self.size || h + PAD > self.size {
            return None;
        }
        if self.shelf_x + w + PAD > self.size {
            self.shelf_y += self.shelf_h;
            self.shelf_x = 0;
            self.shelf_h = 0;
        }
        if self.shelf_y + h + PAD > self.size {
            return None;
        }
        let pos = (self.shelf_x, self.shelf_y);
        self.shelf_x += w + PAD;
        self.shelf_h = self.shelf_h.max(h + PAD);
        Some(pos)
    }

    /// Double la taille (dans la limite du GPU) et vide l'atlas.
    fn grow(&mut self, device: &wgpu::Device) {
        let max = device.limits().max_texture_dimension_2d;
        let size = (self.size * 2).min(max);
        *self = Atlas::new(device, size);
    }
}
