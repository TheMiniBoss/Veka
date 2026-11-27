//! Atlas des dégradés : chaque dégradé est précalculé sur le CPU (par `veka-colora`, dans son espace
//! de couleur) puis rangé ici. Le shader des formes n'a plus qu'à calculer la position `t` et à lire.
//!
//! - lignes 0 à 255 : rampes de 256 texels, 4 par ligne ;
//! - lignes 256 à 1023 : images de dégradés maillés de 64 × 64.

use std::collections::HashMap;

use veka_colora::{Gradient, MeshGradient, mesh_image, mesh_key, ramp, ramp_key};

pub const SIZE: u32 = 1024;
pub const RAMP: u32 = 256;
pub const MESH: u32 = 64;
const RAMP_ROWS: u32 = 256;
const RAMPS_PER_ROW: u32 = SIZE / RAMP;
const RAMP_SLOTS: u32 = RAMP_ROWS * RAMPS_PER_ROW;
const MESH_PER_ROW: u32 = SIZE / MESH;
const MESH_SLOTS: u32 = MESH_PER_ROW * ((SIZE - RAMP_ROWS) / MESH);

/// L'atlas est plein : il est vidé et l'image est recommencée.
#[derive(Debug)]
pub struct RampsFull;

/// Où lire un dégradé, en texels.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
}

pub struct Ramps {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    ramps: HashMap<u64, Slot>,
    meshes: HashMap<u64, Slot>,
}

impl Ramps {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("veka dégradés"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            ramps: HashMap::new(),
            meshes: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.ramps.clear();
        self.meshes.clear();
    }

    /// La rampe d'un dégradé résolu (calculée et envoyée au GPU la première fois).
    pub fn ramp(&mut self, queue: &wgpu::Queue, g: &Gradient) -> Result<Slot, RampsFull> {
        let key = ramp_key(g);
        if let Some(s) = self.ramps.get(&key) {
            return Ok(*s);
        }
        let n = self.ramps.len() as u32;
        if n >= RAMP_SLOTS {
            return Err(RampsFull);
        }
        let slot = Slot {
            x: (n % RAMPS_PER_ROW) * RAMP,
            y: n / RAMPS_PER_ROW,
        };
        self.write(queue, slot, RAMP, 1, &ramp(g, RAMP as usize));
        self.ramps.insert(key, slot);
        Ok(slot)
    }

    /// L'image d'un dégradé maillé résolu.
    pub fn mesh(&mut self, queue: &wgpu::Queue, m: &MeshGradient) -> Result<Slot, RampsFull> {
        let key = mesh_key(m);
        if let Some(s) = self.meshes.get(&key) {
            return Ok(*s);
        }
        let n = self.meshes.len() as u32;
        if n >= MESH_SLOTS {
            return Err(RampsFull);
        }
        let slot = Slot {
            x: (n % MESH_PER_ROW) * MESH,
            y: RAMP_ROWS + (n / MESH_PER_ROW) * MESH,
        };
        self.write(queue, slot, MESH, MESH, &mesh_image(m, MESH as usize));
        self.meshes.insert(key, slot);
        Ok(slot)
    }

    fn write(&self, queue: &wgpu::Queue, slot: Slot, w: u32, h: u32, texels: &[[f32; 4]]) {
        let bytes: Vec<u8> = texels
            .iter()
            .flat_map(|t| t.iter().flat_map(|&v| f16_bits(v).to_le_bytes()))
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: slot.x,
                    y: slot.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 8),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// `f32` → demi-précision (IEEE 754 binary16), arrondi au plus proche.
pub fn f16_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xFF) as i32;
    let mant = b & 0x7F_FFFF;
    if exp == 0xFF {
        // Infini ou NaN.
        return sign | 0x7C00 | if mant != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7C00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // Sous-normal.
        let m = mant | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        return sign | ((m + half) >> shift) as u16;
    }
    let rounded = mant + 0x1000;
    if rounded & 0x80_0000 != 0 {
        let e = e + 1;
        if e >= 31 {
            return sign | 0x7C00;
        }
        return sign | ((e as u16) << 10);
    }
    sign | ((e as u16) << 10) | (rounded >> 13) as u16
}

#[cfg(test)]
mod tests {
    use super::f16_bits;

    #[test]
    fn demi_precision() {
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(1.0), 0x3C00);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(-2.0), 0xC000);
        assert_eq!(f16_bits(65504.0), 0x7BFF);
        assert_eq!(f16_bits(1e9), 0x7C00);
        // Plus petit sous-normal.
        assert_eq!(f16_bits(5.960_464_5e-8), 1);
    }
}
