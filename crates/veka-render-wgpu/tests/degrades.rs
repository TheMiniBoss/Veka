//! Rendu hors écran des dégradés : chaque pixel doit valoir ce que calcule `veka-colora`.
//!
//! Sans GPU (ni logiciel), le test s'arrête sans échouer.

use veka::prelude::*;
use veka_colora::mesh_image;
use veka_foundation::linear_to_srgb;
use veka_graphics::DrawList;
use veka_render_wgpu::{FrameInfo, Gpu, render_to_rgba};

const W: u32 = 256;

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("pas de GPU, test ignoré : {e:?}");
            None
        }
    }
}

fn render(gpu: &Gpu, paint: Paint) -> Vec<u8> {
    let mut list = DrawList::new();
    list.fill(Shape::Rectangle, Rect::new(0.0, 0.0, W as f32, W as f32), paint);
    render_to_rgba(
        gpu,
        &list,
        FrameInfo {
            width: W,
            height: W,
            scale: 1.0,
        },
    )
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn srgb8(c: Color) -> [u8; 4] {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [f(c.r), f(c.g), f(c.b), f(c.a)]
}

fn proche(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

#[test]
fn lineaire_radial_angulaire() {
    let Some(gpu) = gpu() else { return };
    let g = Gradient::new([Color::Red, Color::Yellow, Color::Blue]);

    let img = render(&gpu, LinearGradient::with(g.clone()).into());
    for x in [0u32, 64, 128, 200, 255] {
        let t = (x as f32 + 0.5) / W as f32;
        let attendu = srgb8(g.sample(t));
        assert!(
            proche(px(&img, x, 100), attendu, 3),
            "linéaire x={x} {:?} ≠ {attendu:?}",
            px(&img, x, 100)
        );
    }

    let img = render(&gpu, RadialGradient::with(g.clone(), 128.0).into());
    let centre = px(&img, 128, 128);
    assert!(proche(centre, srgb8(Color::Red), 3), "radial centre {centre:?}");
    assert!(proche(px(&img, 0, 0), srgb8(Color::Blue), 3), "radial coin");

    let img = render(&gpu, AngularGradient::with(g.clone()).into());
    // Juste sous le centre (90°, sens horaire depuis +X) : un quart du tour.
    let attendu = srgb8(g.sample(0.25));
    assert!(
        proche(px(&img, 128, 250), attendu, 6),
        "angulaire {:?} ≠ {attendu:?}",
        px(&img, 128, 250)
    );
}

#[test]
fn oklab_par_defaut() {
    let Some(gpu) = gpu() else { return };
    let ok = render(&gpu, LinearGradient::new([Color::Blue, Color::Yellow]).into());
    let srgb = render(
        &gpu,
        LinearGradient::new([Color::Blue, Color::Yellow])
            .space(ColorSpace::Srgb)
            .into(),
    );
    // Au milieu, Oklab est plus lumineux que sRGB (pas de gris terne).
    let lum = |p: [u8; 4]| p[0] as u32 + p[1] as u32 + p[2] as u32;
    assert!(lum(px(&ok, 128, 10)) > lum(px(&srgb, 128, 10)) + 30);
}

#[test]
fn maillage() {
    let Some(gpu) = gpu() else { return };
    let m = MeshGradient::grid(2, 2, [Color::Red, Color::Green, Color::Blue, Color::White]);
    let img = render(&gpu, m.clone().into());
    let attendu = mesh_image(&m, 64);
    let to8 = |v: [f32; 4]| {
        let a = v[3].max(1e-6);
        srgb8(Color::rgba(
            linear_to_srgb(v[0] / a),
            linear_to_srgb(v[1] / a),
            linear_to_srgb(v[2] / a),
            v[3],
        ))
    };
    for (x, y) in [(2u32, 2u32), (128, 128), (250, 6)] {
        let ix = (x * 64 / W) as usize;
        let iy = (y * 64 / W) as usize;
        let e = to8(attendu[iy * 64 + ix]);
        assert!(
            proche(px(&img, x, y), e, 12),
            "maillage ({x},{y}) {:?} ≠ {e:?}",
            px(&img, x, y)
        );
    }
}
