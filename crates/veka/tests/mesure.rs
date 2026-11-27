//! Mesure du temps par image des exemples `.vk` animés, hors GPU (évaluation, layout, liste de dessin).
//!
//! `cargo test -p veka --release --test mesure -- --ignored --nocapture` (ou sans `--release` pour le
//! debug). La CI l'affiche sans seuil : ses machines sont trop variables pour en juger.

use veka::lang::Program;
use veka::prelude::*;

/// Temps moyen d'une image (ms) sur `n` images à 60 images/s, après 10 images de mise en route.
fn mesure(nom: &str, source: &str, vue: &str, taille: Size, n: u32) -> f64 {
    let p = Program::new(source);
    assert!(p.errors().is_empty(), "{nom} : {:?}", p.errors());
    let mut s = Surface::new(p.view(vue));
    s.resize(taille, 1.0);
    let at = |i: u32| veka::foundation::Instant::from_nanos(((1.0 + f64::from(i) / 60.0) * 1e9) as u64);
    for i in 0..10 {
        s.frame_at(at(i));
    }
    let t0 = std::time::Instant::now();
    for i in 10..10 + n {
        s.frame_at(at(i));
    }
    let ms = t0.elapsed().as_secs_f64() * 1e3 / f64::from(n);
    let profil = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    eprintln!(
        "mesure {profil} : {nom:<8} {ms:6.2} ms par image ({} commandes)",
        s.draw_list().commands.len()
    );
    ms
}

#[test]
#[ignore = "mesure : à lancer avec --ignored --nocapture"]
fn temps_par_image() {
    let n = std::env::var("VEKA_MESURE_IMAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let grand = Size::new(720.0, 480.0);
    mesure("feux", include_str!("../examples/vk/feux.vk"), "Feux", grand, n);
    mesure(
        "anima",
        include_str!("../examples/vk/anima.vk"),
        "Anima",
        Size::new(820.0, 560.0),
        n,
    );
    mesure(
        "verre",
        include_str!("../examples/vk/verre.vk"),
        "Verre",
        Size::new(900.0, 560.0),
        n,
    );
}
