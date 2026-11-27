//! L'exemple `verre` rendu hors écran : le panneau dépoli floute bien le fond, et le repli
//! « Réduire la transparence » donne une teinte unie.
//!
//! `VEKA_DUMP=chemin` écrit les images brutes (`chemin.rgba` et `chemin-repli.rgba`, 1800 × 1120).

use veka::prelude::*;
use veka_render_wgpu::{FrameInfo, Gpu, render_to_rgba_with_stats};

#[path = "../../veka/examples/verre.rs"]
#[allow(dead_code)]
mod verre;

#[test]
fn verre_hors_ecran() {
    let gpu = match Gpu::headless() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("pas de GPU, test ignoré : {e:?}");
            return;
        }
    };
    let scale = 2.0;
    let frame = FrameInfo {
        width: 1800,
        height: 1120,
        scale,
    };
    let mut surface = Surface::new(component(verre::vue));
    surface.resize(Size::new(900.0, 560.0), scale);
    let list = surface.frame().clone();
    let (img, stats) = render_to_rgba_with_stats(&gpu, &list, frame);
    surface.set_reduce_transparency(true);
    let list = surface.frame().clone();
    let (repli, stats_repli) = render_to_rgba_with_stats(&gpu, &list, frame);
    if let Ok(path) = std::env::var("VEKA_DUMP") {
        std::fs::write(format!("{path}.rgba"), &img).expect("écriture");
        std::fs::write(format!("{path}-repli.rgba"), &repli).expect("écriture");
    }
    // Dépoli, carte, goutte (verre et eau), barre (un seul verre pour ses trois onglets).
    assert_eq!(stats.effects, 5, "{stats:?}");
    // Chaque effet lit autour de lui une zone où du texte vient d'être dessiné : pas de partage ici.
    assert!(stats.captures <= 5, "{stats:?}");
    assert_eq!(stats_repli.captures, 0);
    // Au centre du panneau (sous le texte, côté gauche, hors des lettres) : flou, donc proche de la
    // moyenne de ses voisins à 20 pixels.
    let px = |img: &[u8], x: u32, y: u32| {
        let i = ((y * 1800 + x) * 4) as usize;
        [img[i] as i32, img[i + 1] as i32, img[i + 2] as i32]
    };
    let (cx, cy) = (2 * 120, 2 * 175);
    let c = px(&img, cx, cy);
    let n = px(&img, cx + 20, cy);
    assert!(c.iter().zip(n).all(|(a, b)| (a - b).abs() < 24), "{c:?} {n:?}");
    // Le repli est uni : deux points du panneau ont la même couleur.
    assert_eq!(px(&repli, cx, cy), px(&repli, cx + 20, cy));
}
