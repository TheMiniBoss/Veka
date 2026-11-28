//! Les exemples `.vk` : `cargo run --example vk -- compteur` (ou `colora`, `anima`, `feux`, `verre`, `fusion`, `clavier`).
//!
//! Modifiez le fichier `examples/vk/<nom>.vk` pendant que la fenêtre est ouverte : elle se met
//! à jour à l'enregistrement, sans perdre ses états. Une erreur s'affiche en bandeau rouge.

use veka::prelude::*;

fn main() {
    let nom = std::env::args().nth(1).unwrap_or_else(|| "compteur".into());
    let (vue, taille) = match nom.as_str() {
        "compteur" => ("Compteur", Size::new(480.0, 320.0)),
        "colora" => ("Colora", Size::new(680.0, 520.0)),
        "anima" => ("Anima", Size::new(860.0, 720.0)),
        "feux" => ("Feux", Size::new(640.0, 200.0)),
        "verre" => ("Verre", Size::new(900.0, 560.0)),
        "fusion" => ("Fusion", Size::new(980.0, 600.0)),
        "clavier" => ("Clavier", Size::new(560.0, 360.0)),
        "formulaire" => ("Formulaire", Size::new(420.0, 520.0)),
        _ => {
            eprintln!(
                "Veka : exemple inconnu `{nom}` (compteur, colora, anima, feux, verre, fusion, clavier, formulaire)"
            );
            std::process::exit(2);
        }
    };
    let chemin = format!("{}/examples/vk/{nom}.vk", env!("CARGO_MANIFEST_DIR"));
    let fenetre = Window::new(format!("{vue} (.vk)")).size(taille);
    if let Err(e) = App::new().window_vk_with(fenetre, chemin, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
