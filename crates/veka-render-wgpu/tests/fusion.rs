//! Modes de fusion et calques (étape 8) : le GPU donne les mêmes couleurs que les formules de référence
//! (`veka_graphics::composite`), un groupe à 50 % ne laisse pas voir ses recouvrements, les groupes
//! s'imbriquent.
//!
//! Sans GPU (ni logiciel comme lavapipe ou WARP), les tests s'arrêtent sans échouer.

use veka_foundation::{Color, Rect};
use veka_graphics::{BackdropEffect, BlendMode, BlurEffect, DrawCommand, DrawList, Shape, composite};
use veka_render_wgpu::{FrameInfo, Gpu, render_to_rgba_with_stats};

fn gpu() -> Option<Gpu> {
    match Gpu::headless() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("pas de GPU, test ignoré : {e:?}");
            None
        }
    }
}

const W: u32 = 64;
const H: u32 = 32;

fn frame() -> FrameInfo {
    FrameInfo {
        width: W,
        height: H,
        scale: 1.0,
    }
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::rgb(r, g, b)
}

fn encode(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Le pixel attendu (sRGB 8 bits) : la formule de référence sur les couleurs du test.
fn expected(mode: BlendMode, back: Color, front: Color, alpha: f32) -> [u8; 4] {
    let b = back.to_linear_premultiplied();
    let s = front.opacity(alpha).to_linear_premultiplied();
    let o = composite(mode, b, s);
    // La texture garde des valeurs prémultipliées (encodées en sRGB).
    let c = [encode(o[0]), encode(o[1]), encode(o[2]), o[3]];
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn close(a: [u8; 4], b: [u8; 4], tol: i32) -> bool {
    a.iter()
        .zip(&b)
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= tol)
}

#[test]
fn chaque_mode_suit_la_formule() {
    let Some(gpu) = gpu() else { return };
    let back = rgb(0.2, 0.5, 0.8);
    let front = rgb(0.9, 0.4, 0.1);
    for mode in BlendMode::ALL {
        let mut l = DrawList::new();
        l.fill(Shape::Rectangle, Rect::new(0.0, 0.0, W as f32, H as f32), back);
        l.push(DrawCommand::PushGroup {
            blend: mode,
            opacity: 0.75,
        });
        l.fill(Shape::Rectangle, Rect::new(16.0, 0.0, 32.0, H as f32), front);
        l.push(DrawCommand::PopGroup);
        let (img, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
        assert_eq!(stats.groups, 1, "{mode:?}");
        let want = expected(mode, back, front, 0.75);
        let got = px(&img, 32, 16);
        assert!(close(got, want, 2), "{mode:?} : {got:?} au lieu de {want:?}");
        // Hors du groupe, le fond ne bouge pas.
        assert!(
            close(px(&img, 4, 16), expected(BlendMode::Normal, back, back, 0.0), 1),
            "{mode:?}"
        );
    }
}

#[test]
fn un_groupe_normal_opaque_ne_cree_pas_de_calque() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    l.push(DrawCommand::PushGroup {
        blend: BlendMode::Normal,
        opacity: 1.0,
    });
    l.fill(
        Shape::Rectangle,
        Rect::new(0.0, 0.0, 8.0, 8.0),
        rgb(1.0, 0.0, 0.0),
    );
    l.push(DrawCommand::PopGroup);
    let (img, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.groups, 0);
    assert_eq!(stats.captures, 0);
    assert_eq!(px(&img, 4, 4), [255, 0, 0, 255]);
}

#[test]
fn l_opacite_d_un_groupe_cache_les_recouvrements() {
    let Some(gpu) = gpu() else { return };
    let draw = |group: bool| {
        let mut l = DrawList::new();
        l.fill(
            Shape::Rectangle,
            Rect::new(0.0, 0.0, W as f32, H as f32),
            Color::White,
        );
        l.push(DrawCommand::PushLayer { opacity: 0.5 });
        if group {
            l.push(DrawCommand::PushGroup {
                blend: BlendMode::Normal,
                opacity: 1.0,
            });
        }
        l.fill(
            Shape::Rectangle,
            Rect::new(0.0, 0.0, 40.0, H as f32),
            Color::Black,
        );
        l.fill(
            Shape::Rectangle,
            Rect::new(24.0, 0.0, 40.0, H as f32),
            Color::Black,
        );
        if group {
            l.push(DrawCommand::PopGroup);
        }
        l.push(DrawCommand::PopLayer);
        render_to_rgba_with_stats(&gpu, &l, frame())
    };
    // Sans groupe, chaque forme est à 50 % : le recouvrement est plus sombre.
    let (img, _) = draw(false);
    assert!(px(&img, 32, 16)[0] < px(&img, 8, 16)[0]);
    // Avec le groupe, tout est à 50 % d'un bloc.
    let (img, stats) = draw(true);
    assert_eq!(stats.groups, 1);
    assert!(
        close(px(&img, 32, 16), px(&img, 8, 16), 1),
        "{:?} {:?}",
        px(&img, 32, 16),
        px(&img, 8, 16)
    );
    assert!(close(px(&img, 8, 16), px(&img, 56, 16), 1));
}

#[test]
fn destination_out_perce_le_groupe() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    l.fill(
        Shape::Rectangle,
        Rect::new(0.0, 0.0, W as f32, H as f32),
        rgb(0.0, 0.0, 1.0),
    );
    l.push(DrawCommand::PushGroup {
        blend: BlendMode::Normal,
        opacity: 0.999,
    });
    l.fill(
        Shape::Rectangle,
        Rect::new(0.0, 0.0, W as f32, H as f32),
        rgb(1.0, 0.0, 0.0),
    );
    l.push(DrawCommand::PushGroup {
        blend: BlendMode::DestinationOut,
        opacity: 1.0,
    });
    l.fill(Shape::Rectangle, Rect::new(16.0, 8.0, 32.0, 16.0), Color::White);
    l.push(DrawCommand::PopGroup);
    l.push(DrawCommand::PopGroup);
    let (img, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.groups, 2);
    // Le rouge est percé : on voit le bleu du fond.
    assert!(
        close(px(&img, 32, 16), [0, 0, 255, 255], 1),
        "{:?}",
        px(&img, 32, 16)
    );
    assert!(close(px(&img, 4, 4), [255, 0, 0, 255], 4), "{:?}", px(&img, 4, 4));
}

#[test]
fn un_flou_dans_un_groupe_lit_le_groupe() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    // Fond vert, groupe en multiply : dedans, un flou sur du blanc reste blanc (il ne voit pas le vert).
    l.fill(
        Shape::Rectangle,
        Rect::new(0.0, 0.0, W as f32, H as f32),
        rgb(0.0, 1.0, 0.0),
    );
    l.push(DrawCommand::PushGroup {
        blend: BlendMode::Multiply,
        opacity: 1.0,
    });
    l.fill(
        Shape::Rectangle,
        Rect::new(0.0, 0.0, W as f32, H as f32),
        Color::White,
    );
    l.backdrop(
        Shape::Rectangle,
        Rect::new(16.0, 8.0, 32.0, 16.0),
        BackdropEffect::Blur(BlurEffect {
            radius: 6.0,
            tint_amount: 0.0,
            saturation: 1.0,
            noise: 0.0,
            ..BlurEffect::default()
        }),
    );
    l.push(DrawCommand::PopGroup);
    let (img, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.groups, 1);
    // Blanc × vert = vert, partout (flou compris).
    let g = px(&img, 32, 16);
    assert!(close(g, px(&img, 4, 4), 2), "{g:?}");
    assert!(g[1] > 200 && g[0] < 10 && g[2] < 10, "{g:?}");
}

#[path = "../../veka/examples/fusion.rs"]
#[allow(dead_code)]
mod exemple;

/// L'exemple `fusion` : une découpe perce bien la case jusqu'au fond de la fenêtre, et l'opacité d'un
/// groupe ne laisse pas voir le recouvrement. `VEKA_DUMP=chemin.rgba` écrit l'image.
#[test]
fn l_exemple_fusion() {
    use veka::prelude::*;
    let Some(gpu) = gpu() else { return };
    let (w, h, scale) = (980u32, 600u32, 1.0f32);
    let mut surface = Surface::new(component(exemple::vue));
    surface.resize(Size::new(w as f32, h as f32), scale);
    let list = surface.frame().clone();
    let (img, stats) = render_to_rgba_with_stats(
        &gpu,
        &list,
        FrameInfo {
            width: w,
            height: h,
            scale,
        },
    );
    if let Ok(path) = std::env::var("VEKA_DUMP") {
        std::fs::write(path, &img).unwrap();
    }
    // `normal` dans une case opaque ne crée pas de calque ; les autres oui (et la case autour).
    assert!(stats.groups >= 40, "{stats:?}");
    let at = |p: Point| {
        let i = (((p.y * scale) as u32 * w + (p.x * scale) as u32) * 4) as usize;
        [img[i], img[i + 1], img[i + 2], img[i + 3]]
    };
    let fond = at(Point::new(4.0, 4.0));
    // Les cases, dans l'ordre : la 19e est `destinationOut` (elle perce la case au centre).
    let cases = surface.rects_of("RoundedRectangle");
    assert_eq!(cases.len(), 21);
    let trou = at(cases[18].center());
    assert!(close(trou, fond, 2), "{trou:?} {fond:?}");
    let plein = at(cases[0].center());
    assert!(!close(plein, fond, 8), "{plein:?}");
}
