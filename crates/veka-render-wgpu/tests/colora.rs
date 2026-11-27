//! L'exemple `colora` rendu hors écran, en clair et en sombre.
//! `VEKA_DUMP_DIR=dossier` y écrit les images brutes (RGBA).

#[allow(dead_code)]
#[path = "../../veka/examples/colora.rs"]
mod exemple;

use veka::prelude::*;
use veka_render_wgpu::{FrameInfo, Gpu, render_to_rgba};

#[test]
fn colora_clair_et_sombre() {
    let gpu = match Gpu::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("pas de GPU, test ignoré : {e:?}");
            return;
        }
    };
    let (w, h, scale) = (680u32, 520u32, 1.0);
    for (nom, scheme) in [("clair", ColorScheme::Light), ("sombre", ColorScheme::Dark)] {
        let mut s = Surface::new(component(exemple::vue));
        s.set_system_appearance(scheme, None);
        s.resize(Size::new(w as f32, h as f32), scale);
        let list = s.frame().clone();
        let img = render_to_rgba(
            &gpu,
            &list,
            FrameInfo {
                width: w,
                height: h,
                scale,
            },
        );
        if let Ok(dir) = std::env::var("VEKA_DUMP_DIR") {
            std::fs::write(format!("{dir}/colora_{nom}.rgba"), &img).unwrap();
        }
        // Le coin de la fenêtre : fond de fenêtre du mode.
        let fond = &img[..4];
        match scheme {
            ColorScheme::Light => assert_eq!(fond, [0xF2, 0xF2, 0xF7, 0xFF]),
            ColorScheme::Dark => assert_eq!(fond, [0x1C, 0x1C, 0x1E, 0xFF]),
        }
    }
}
