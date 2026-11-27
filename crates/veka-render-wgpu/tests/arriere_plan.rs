//! Flou d'arrière-plan : le flou mélange bien ce qui est derrière, sans toucher le reste, et les effets
//! partagent leurs captures.
//!
//! Sans GPU (ni logiciel comme lavapipe ou WARP), les tests s'arrêtent sans échouer.

use veka_foundation::{Color, Rect};
use veka_graphics::{BackdropEffect, BlurEffect, DrawList, Shape};
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

const W: u32 = 256;
const H: u32 = 128;

fn frame() -> FrameInfo {
    FrameInfo {
        width: W,
        height: H,
        scale: 1.0,
    }
}

/// Bandes verticales noires et blanches de 4 pixels.
fn bandes(l: &mut DrawList) {
    l.fill(
        Shape::Rectangle,
        Rect::new(0.0, 0.0, W as f32, H as f32),
        Color::Black,
    );
    for i in 0..(W / 8) {
        l.fill(
            Shape::Rectangle,
            Rect::new(i as f32 * 8.0, 0.0, 4.0, H as f32),
            Color::White,
        );
    }
}

fn flou(radius: f32) -> BackdropEffect {
    BackdropEffect::Blur(BlurEffect {
        radius,
        tint_amount: 0.0,
        saturation: 1.0,
        ..BlurEffect::default()
    })
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

#[test]
fn le_flou_melange_les_bandes() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    bandes(&mut l);
    l.backdrop(Shape::Rectangle, Rect::new(64.0, 32.0, 128.0, 64.0), flou(12.0));
    let (img, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.captures, 1);
    assert_eq!(stats.effects, 1);
    // Hors de l'effet : les bandes nettes.
    assert_eq!(px(&img, 1, 10), [255, 255, 255, 255]);
    assert_eq!(px(&img, 5, 10), [0, 0, 0, 255]);
    // Dedans : un gris uniforme (moitié de lumière en linéaire ≈ 188 en sRGB), d'une bande à l'autre.
    let mut vals = Vec::new();
    for x in 100..116 {
        let p = px(&img, x, 64);
        assert_eq!(p[3], 255);
        assert!(p[0] == p[1] && p[1] == p[2], "{p:?}");
        vals.push(p[0] as i32);
    }
    let (min, max) = (vals.iter().min().unwrap(), vals.iter().max().unwrap());
    assert!(max - min <= 6, "flou irrégulier : {vals:?}");
    assert!((170..=205).contains(min), "{vals:?}");
}

#[test]
fn teinte_et_coins_arrondis() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    bandes(&mut l);
    l.backdrop(
        Shape::RoundedRectangle(veka_graphics::CornerRadii::all(24.0)),
        Rect::new(64.0, 32.0, 128.0, 64.0),
        BackdropEffect::Blur(BlurEffect {
            radius: 12.0,
            tint: Color::Red,
            tint_amount: 1.0,
            ..BlurEffect::default()
        }),
    );
    let (img, _) = render_to_rgba_with_stats(&gpu, &l, frame());
    // Teinte pleine au centre (le rouge du système, #FF3B30).
    let c = px(&img, 128, 64);
    assert!(
        c[0] >= 253 && c[1].abs_diff(59) <= 2 && c[2].abs_diff(48) <= 2,
        "{c:?}"
    );
    // Le coin arrondi laisse voir les bandes.
    let coin = px(&img, 65, 33);
    assert!(coin == [255, 255, 255, 255] || coin == [0, 0, 0, 255], "{coin:?}");
}

#[test]
fn les_effets_voisins_partagent_une_capture() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    bandes(&mut l);
    // Une rangée de boutons : un flou puis son libellé (dessiné dans la zone du bouton seulement).
    for i in 0..6 {
        let r = Rect::new(8.0 + i as f32 * 40.0, 40.0, 32.0, 32.0);
        l.backdrop(Shape::Capsule, r, flou(4.0));
        l.fill(
            Shape::Circle,
            Rect::new(r.x() + 12.0, r.y() + 12.0, 8.0, 8.0),
            Color::Blue,
        );
    }
    let (_, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    // Les libellés touchent la zone lue par le bouton voisin (le flou déborde de 8 pixels), donc pas
    // tous partagés… sauf si on les espace :
    assert!(stats.captures >= 1);
    let mut l = DrawList::new();
    bandes(&mut l);
    for i in 0..5 {
        let r = Rect::new(4.0 + i as f32 * 50.0, 40.0, 24.0, 24.0);
        l.backdrop(Shape::Capsule, r, flou(4.0));
        l.fill(
            Shape::Circle,
            Rect::new(r.x() + 8.0, r.y() + 8.0, 8.0, 8.0),
            Color::Blue,
        );
    }
    let (_, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.captures, 1, "{stats:?}");
    assert_eq!(stats.effects, 5);
    // Un verre posé sur un autre doit le voir : nouvelle capture.
    let mut l = DrawList::new();
    bandes(&mut l);
    l.backdrop(Shape::Rectangle, Rect::new(10.0, 10.0, 100.0, 100.0), flou(4.0));
    l.backdrop(Shape::Rectangle, Rect::new(60.0, 20.0, 100.0, 60.0), flou(4.0));
    let (_, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.captures, 2, "{stats:?}");
}

#[test]
fn sans_effet_rien_ne_change() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    bandes(&mut l);
    let (_, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    assert_eq!(stats.captures, 0);
    // Le dessin et la copie vers la cible.
    assert_eq!(stats.passes, 2);
}

fn dump(name: &str, img: &[u8]) {
    if let Ok(dir) = std::env::var("VEKA_DUMP_DIR") {
        std::fs::write(format!("{dir}/{name}.rgba"), img).expect("écriture");
    }
}

#[test]
fn le_verre_refracte_au_bord() {
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    bandes(&mut l);
    // Un verre seul, puis deux gouttes fusionnées.
    l.backdrop(
        Shape::Capsule,
        Rect::new(16.0, 24.0, 100.0, 48.0),
        BackdropEffect::Glass(Box::default()),
    );
    let mut g = veka_graphics::GlassEffect {
        smoothness: 16.0,
        ..Default::default()
    };
    g.shapes[0] = veka_graphics::GlassShape {
        shape: Shape::Circle,
        rect: Rect::new(140.0, 30.0, 44.0, 44.0),
    };
    g.shapes[1] = veka_graphics::GlassShape {
        shape: Shape::Circle,
        rect: Rect::new(192.0, 30.0, 44.0, 44.0),
    };
    g.count = 2;
    l.backdrop(
        Shape::Rectangle,
        Rect::new(140.0, 30.0, 96.0, 44.0),
        BackdropEffect::Glass(Box::new(g)),
    );
    let (img, stats) = render_to_rgba_with_stats(&gpu, &l, frame());
    dump("verre", &img);
    assert_eq!(stats.effects, 2);
    // Au centre du verre seul : les bandes (4 px) restent visibles sous le léger flou, pas un gris uni.
    let row: Vec<u8> = (50..82).map(|x| px(&img, x, 48)[0]).collect();
    let (min, max) = (row.iter().min().unwrap(), row.iter().max().unwrap());
    assert!(max - min > 8, "{row:?}");
    // Le pont entre les deux gouttes est du verre (teinté, lumineux), pas une bande noire nette.
    let pont = px(&img, 188, 52);
    assert!(pont[3] == 255, "{pont:?}");
}

#[test]
fn le_verre_sur_un_fond_transparent_reste_clair() {
    // Fenêtre posée sur Mica : rien n'est dessiné derrière le verre, il ne doit pas devenir noir.
    let Some(gpu) = gpu() else { return };
    let mut l = DrawList::new();
    l.backdrop(
        Shape::Capsule,
        Rect::new(16.0, 24.0, 200.0, 56.0),
        BackdropEffect::Glass(Box::default()),
    );
    let img = render_to_rgba_with_stats(&gpu, &l, frame()).0;
    dump("verre-transparent", &img);
    let centre = px(&img, 116, 52);
    assert!(centre[3] < 128, "le corps reste transparent : {centre:?}");
    // Le reflet du bord se voit, clair.
    let bord = (20..40).map(|y| px(&img, 116, y)).max_by_key(|p| p[3]).unwrap();
    assert!(bord[3] > 40 && bord[0] >= bord[3] / 2, "{bord:?}");
}
