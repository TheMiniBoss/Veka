//! Échantillonnage des dégradés pour les renderers : rampes et images de dégradés maillés, en
//! composantes **linéaires prémultipliées**.

use std::hash::{DefaultHasher, Hash, Hasher};

use veka_foundation::Color;

use crate::paint::{Gradient, Ink, MeshGradient};
use crate::space::{ColorSpace, from_space, to_space};

/// Rampe de `n` texels de 0 à 1.
pub fn ramp(gradient: &Gradient, n: usize) -> Vec<[f32; 4]> {
    let n = n.max(2);
    (0..n)
        .map(|i| {
            gradient
                .sample(i as f32 / (n - 1) as f32)
                .to_linear_premultiplied()
        })
        .collect()
}

/// Clé stable du contenu d'une rampe (pour les caches des renderers).
pub fn ramp_key(gradient: &Gradient) -> u64 {
    let mut h = DefaultHasher::new();
    0u8.hash(&mut h);
    gradient.space.hash(&mut h);
    for s in &gradient.stops {
        hash_ink(&s.ink, &mut h);
        s.location.to_bits().hash(&mut h);
    }
    h.finish()
}

/// Clé stable du contenu d'un dégradé maillé.
pub fn mesh_key(mesh: &MeshGradient) -> u64 {
    let mut h = DefaultHasher::new();
    1u8.hash(&mut h);
    (mesh.width, mesh.height, mesh.smooth, mesh.space).hash(&mut h);
    for p in &mesh.points {
        p.position.x.to_bits().hash(&mut h);
        p.position.y.to_bits().hash(&mut h);
        hash_ink(&p.ink, &mut h);
    }
    h.finish()
}

fn hash_ink(ink: &Ink, h: &mut DefaultHasher) {
    match ink {
        Ink::Color(c) => [c.r, c.g, c.b, c.a].map(f32::to_bits).hash(h),
        Ink::Token(t, l) => (t, l).hash(h),
    }
}

/// Image `size × size` d'un dégradé maillé résolu, ligne par ligne (repère de la forme : (0, 0) en haut à
/// gauche). Les zones que le maillage ne couvre pas prennent la couleur moyenne.
pub fn mesh_image(mesh: &MeshGradient, size: usize) -> Vec<[f32; 4]> {
    let size = size.max(1);
    if !mesh.is_valid() {
        return vec![[0.0; 4]; size * size];
    }
    // Les teintes (OkLCH) ne s'interpolent pas en deux dimensions : Oklab à la place.
    let space = match mesh.space {
        ColorSpace::OkLch => ColorSpace::Oklab,
        s => s,
    };
    let (w, h) = (mesh.width, mesh.height);
    let pos: Vec<[f32; 2]> = mesh.points.iter().map(|p| [p.position.x, p.position.y]).collect();
    let col: Vec<[f32; 4]> = mesh
        .points
        .iter()
        .map(|p| to_space(p.ink.color().unwrap_or(Color::Clear), space))
        .collect();

    let mut acc = vec![[0.0f32; 4]; size * size];
    let mut hit = vec![false; size * size];
    // Chaque cellule du maillage découpée en SUB × SUB quadrilatères, deux triangles chacun.
    const SUB: usize = 8;
    let (gw, gh) = ((w - 1) * SUB + 1, (h - 1) * SUB + 1);
    let mut grid_p = Vec::with_capacity(gw * gh);
    let mut grid_c = Vec::with_capacity(gw * gh);
    for j in 0..gh {
        for i in 0..gw {
            let u = i as f32 / SUB as f32;
            let v = j as f32 / SUB as f32;
            grid_p.push(eval(&pos, w, h, u, v, mesh.smooth));
            grid_c.push(eval(&col, w, h, u, v, mesh.smooth));
        }
    }
    let px = |p: [f32; 2]| [p[0] * size as f32, p[1] * size as f32];
    for j in 0..gh - 1 {
        for i in 0..gw - 1 {
            let k = |a: usize, b: usize| b * gw + a;
            let (a, b, c, d) = (k(i, j), k(i + 1, j), k(i, j + 1), k(i + 1, j + 1));
            for tri in [[a, b, c], [c, b, d]] {
                raster(
                    [px(grid_p[tri[0]]), px(grid_p[tri[1]]), px(grid_p[tri[2]])],
                    [grid_c[tri[0]], grid_c[tri[1]], grid_c[tri[2]]],
                    size,
                    &mut acc,
                    &mut hit,
                );
            }
        }
    }
    let mean = {
        let mut s = [0.0f32; 4];
        for c in &col {
            for (x, y) in s.iter_mut().zip(c) {
                *x += y;
            }
        }
        s.map(|x| x / col.len() as f32)
    };
    acc.iter()
        .zip(&hit)
        .map(|(v, &h)| from_space(if h { *v } else { mean }, space).to_linear_premultiplied())
        .collect()
}

fn raster(p: [[f32; 2]; 3], c: [[f32; 4]; 3], size: usize, acc: &mut [[f32; 4]], hit: &mut [bool]) {
    let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let min_x = p
        .iter()
        .map(|q| q[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_x = p
        .iter()
        .map(|q| q[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(size as f32) as usize;
    let min_y = p
        .iter()
        .map(|q| q[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_y = p
        .iter()
        .map(|q| q[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(size as f32) as usize;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let (sx, sy) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((p[1][0] - sx) * (p[2][1] - sy) - (p[2][0] - sx) * (p[1][1] - sy)) / area;
            let w1 = ((p[2][0] - sx) * (p[0][1] - sy) - (p[0][0] - sx) * (p[2][1] - sy)) / area;
            let w2 = 1.0 - w0 - w1;
            const EPS: f32 = -1e-4;
            if w0 < EPS || w1 < EPS || w2 < EPS {
                continue;
            }
            let i = y * size + x;
            if hit[i] {
                continue;
            }
            for k in 0..4 {
                acc[i][k] = w0 * c[0][k] + w1 * c[1][k] + w2 * c[2][k];
            }
            hit[i] = true;
        }
    }
}

/// Valeur de la grille en (u, v), en coordonnées de points (0 à w - 1, 0 à h - 1).
fn eval<const N: usize>(v: &[[f32; N]], w: usize, h: usize, u: f32, t: f32, smooth: bool) -> [f32; N] {
    let at = |x: isize, y: isize| {
        v[(y.clamp(0, h as isize - 1) as usize) * w + x.clamp(0, w as isize - 1) as usize]
    };
    let (x0, y0) = (
        (u.floor() as isize).min(w as isize - 2),
        (t.floor() as isize).min(h as isize - 2),
    );
    let (fx, fy) = (u - x0 as f32, t - y0 as f32);
    let mut out = [0.0; N];
    if smooth {
        let mut rows = [[0.0f32; N]; 4];
        for (r, row) in rows.iter_mut().enumerate() {
            let y = y0 - 1 + r as isize;
            *row = catmull([at(x0 - 1, y), at(x0, y), at(x0 + 1, y), at(x0 + 2, y)], fx);
        }
        out = catmull(rows, fy);
    } else {
        let (a, b, c, d) = (at(x0, y0), at(x0 + 1, y0), at(x0, y0 + 1), at(x0 + 1, y0 + 1));
        for k in 0..N {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bot = c[k] + (d[k] - c[k]) * fx;
            out[k] = top + (bot - top) * fy;
        }
    }
    out
}

fn catmull<const N: usize>(p: [[f32; N]; 4], t: f32) -> [f32; N] {
    let (t2, t3) = (t * t, t * t * t);
    let mut out = [0.0; N];
    for k in 0..N {
        let (p0, p1, p2, p3) = (p[0][k], p[1][k], p[2][k], p[3][k]);
        out[k] = 0.5
            * (2.0 * p1
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::MeshGradient;

    #[test]
    fn rampe_aux_extremites() {
        let r = ramp(&Gradient::new([Color::Red, Color::Blue]), 256);
        assert_eq!(r.len(), 256);
        assert_eq!(r[0], Color::Red.to_linear_premultiplied());
        assert_eq!(r[255], Color::Blue.to_linear_premultiplied());
    }

    #[test]
    fn cles() {
        let a = Gradient::new([Color::Red, Color::Blue]);
        let b = Gradient::new([Color::Red, Color::Green]);
        assert_eq!(ramp_key(&a), ramp_key(&a.clone()));
        assert_ne!(ramp_key(&a), ramp_key(&b));
    }

    #[test]
    fn maillage_coins() {
        let m = MeshGradient::grid(2, 2, [Color::Red, Color::Green, Color::Blue, Color::White]).smooth(false);
        let img = mesh_image(&m, 64);
        let close = |a: [f32; 4], b: Color| {
            let b = b.to_linear_premultiplied();
            (0..4).all(|k| (a[k] - b[k]).abs() < 0.08)
        };
        assert!(close(img[0], Color::Red), "{:?}", img[0]);
        assert!(close(img[63], Color::Green), "{:?}", img[63]);
        assert!(close(img[63 * 64], Color::Blue), "{:?}", img[63 * 64]);
        assert!(close(img[64 * 64 - 1], Color::White), "{:?}", img[64 * 64 - 1]);
    }

    #[test]
    fn maillage_lisse_couvre_tout() {
        let m = MeshGradient::grid(3, 3, [Color::Red, Color::Orange, Color::Yellow, Color::Purple]);
        let img = mesh_image(&m, 32);
        assert!(img.iter().all(|p| p[3] > 0.99));
    }
}
