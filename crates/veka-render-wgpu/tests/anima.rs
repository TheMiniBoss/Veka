//! L'exemple `anima` rendu hors écran, image par image : départ, milieu et fin des animations.
//! `VEKA_DUMP_DIR=dossier` y écrit les images brutes (RGBA).

#[allow(dead_code)]
#[path = "../../veka/examples/anima.rs"]
mod exemple;

use veka::foundation::Instant;
use veka::prelude::*;
use veka::ui::{InputEvent, PointerButton};
use veka_render_wgpu::{FrameInfo, Gpu, render_to_rgba};

fn clic(s: &mut Surface, p: Point) {
    s.handle(InputEvent::PointerDown {
        position: p,
        button: PointerButton::Primary,
    });
    s.handle(InputEvent::PointerUp {
        position: p,
        button: PointerButton::Primary,
    });
}

#[test]
fn anima_image_par_image() {
    let gpu = match Gpu::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("pas de GPU, test ignoré : {e:?}");
            return;
        }
    };
    let (w, h, scale) = (820u32, 560u32, 1.0);
    let mut s = Surface::new(component(exemple::vue));
    s.resize(Size::new(w as f32, h as f32), scale);
    let t = |s: &mut Surface, secs: f64| s.frame_at(Instant::from_nanos((secs * 1e9) as u64)).clone();
    let rendu = |list: &veka::graphics::DrawList| {
        render_to_rgba(
            &gpu,
            list,
            FrameInfo {
                width: w,
                height: h,
                scale,
            },
        )
    };
    let depart = rendu(&t(&mut s, 1.0));
    assert!(!s.is_animating(), "rien ne bouge au repos");

    // La carte, puis « Chaud », « Lancer les courbes » et « Ajouter ».
    let carte = s.rects_of("onTapGesture")[0].center();
    clic(&mut s, carte);
    let boutons = s.rects_of("Button");
    assert_eq!(boutons.len(), 4, "{boutons:?}");
    for b in [0usize, 1, 2] {
        let b = s.rects_of("Button")[b].center();
        clic(&mut s, b);
    }
    t(&mut s, 1.0);
    assert!(s.is_animating());
    let milieu = rendu(&t(&mut s, 1.25));
    let fin_list = t(&mut s, 4.0);
    assert!(!s.is_animating(), "tout est fini");
    let fin = rendu(&fin_list);
    assert_ne!(depart, milieu);
    assert_ne!(milieu, fin);
    assert_eq!(s.texts().iter().filter(|t| t.starts_with("Élément")).count(), 4);
    if let Ok(dir) = std::env::var("VEKA_DUMP_DIR") {
        for (nom, img) in [("depart", &depart), ("milieu", &milieu), ("fin", &fin)] {
            std::fs::write(format!("{dir}/anima_{nom}.rgba"), img).unwrap();
        }
    }
}
