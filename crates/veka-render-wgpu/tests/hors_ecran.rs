//! Rendu hors écran du compteur : on vérifie la couleur de quelques pixels.
//!
//! Sans GPU (ni logiciel comme lavapipe ou WARP), le test s'arrête sans échouer.
//! `VEKA_DUMP=chemin.rgba` écrit l'image brute pour la regarder.

use veka::prelude::*;
use veka_render_wgpu::{FrameInfo, Gpu, render_to_rgba};

fn compteur() -> impl View {
    let mut clics = State::new(0);
    VStack![spacing: 16;
        Text("Bonjour Veka").font(Font::Title),
        HStack![spacing: 12;
            RoundedRectangle(12).fill(Color::Blue).frame(80, 48),
            Capsule().fill(Color::Orange).frame(96, 48),
            Circle().fill(Color::Green).frame(48, 48),
        ],
        Button(format!("Clics : {clics}"), move || clics += 1),
    ]
    .padding(24)
}

fn pixel(img: &[u8], width: u32, scale: f32, p: Point) -> [u8; 4] {
    let x = (p.x * scale) as u32;
    let y = (p.y * scale) as u32;
    let i = ((y * width + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

fn proche(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3)
}

fn srgb8(c: Color) -> [u8; 4] {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [f(c.r), f(c.g), f(c.b), f(c.a)]
}

#[test]
fn compteur_hors_ecran() {
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("pas de GPU, test ignoré : {e:?}");
            return;
        }
    };
    let scale = 2.0;
    let mut surface = Surface::new(component(compteur));
    surface.resize(Size::new(480.0, 320.0), scale);
    let frame = FrameInfo {
        width: 960,
        height: 640,
        scale,
    };
    let list = surface.frame().clone();
    let img = render_to_rgba(&gpu, &list, frame);
    assert_eq!(img.len(), 960 * 640 * 4);

    if let Ok(path) = std::env::var("VEKA_DUMP") {
        std::fs::write(path, &img).unwrap();
    }

    let bleu = surface.rects_of("RoundedRectangle")[0];
    let orange = surface.rects_of("Capsule")[0];
    let vert = surface.rects_of("Circle")[0];
    assert!(proche(pixel(&img, 960, scale, bleu.center()), srgb8(Color::Blue)));
    assert!(proche(
        pixel(&img, 960, scale, orange.center()),
        srgb8(Color::Orange)
    ));
    assert!(proche(
        pixel(&img, 960, scale, vert.center()),
        srgb8(Color::Green)
    ));

    // Le coin du cercle est hors de la forme : on y voit le fond, pas du vert.
    let coin = Point::new(vert.origin.x + 2.0, vert.origin.y + 2.0);
    assert!(!proche(pixel(&img, 960, scale, coin), srgb8(Color::Green)));

    // Le texte du titre a laissé des pixels sombres dans sa boîte.
    let titre = surface.rects_of("Text")[0];
    let mut sombres = 0;
    for y in (titre.origin.y as u32)..(titre.max_y() as u32) {
        for x in (titre.origin.x as u32)..(titre.max_x() as u32) {
            let [r, g, b, _] = pixel(&img, 960, scale, Point::new(x as f32, y as f32));
            if (r as u32 + g as u32 + b as u32) < 200 {
                sombres += 1;
            }
        }
    }
    assert!(
        sombres > 20,
        "le titre devrait être dessiné ({sombres} pixels sombres)"
    );
}

fn fenetre_perso() -> impl View {
    VStack![spacing: 0;
        TitleBarWith("Veka", veka::ui::window::TitleBarStyle::Windows),
        Text("Contenu").padding(24),
        Spacer(),
    ]
}

#[test]
fn barre_de_titre_hors_ecran() {
    let Ok(gpu) = Gpu::headless() else {
        eprintln!("pas de GPU, test ignoré");
        return;
    };
    let scale = 2.0;
    let mut surface = Surface::new(component(fenetre_perso));
    surface.resize(Size::new(480.0, 200.0), scale);
    surface.set_window_info(WindowInfo { active: true, ..WindowInfo::default() });
    let list = surface.frame().clone();
    let img = render_to_rgba(&gpu, &list, FrameInfo { width: 960, height: 400, scale });
    if let Ok(path) = std::env::var("VEKA_DUMP_BARRE") {
        std::fs::write(path, &img).unwrap();
    }
    // La croix de fermeture passe par le centre du bouton : pixel sombre.
    let fermer = surface.regions()[3].rect;
    let [r, g, b, _] = pixel(&img, 960, scale, fermer.center());
    assert!((r as u32 + g as u32 + b as u32) < 300, "croix dessinée au centre ({r}, {g}, {b})");
    // Un peu à côté de la croix : le fond.
    let [r, _, _, _] = pixel(&img, 960, scale, Point::new(fermer.origin.x + 4.0, fermer.center().y));
    assert!(r > 200);
}
