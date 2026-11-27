//! L'exemple de l'étape 1 : une fenêtre avec du texte, des formes, des piles et un bouton qui
//! compte les clics. `cargo run --example compteur`

use veka::prelude::*;

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

fn main() {
    if let Err(e) = App::new().window("Compteur", compteur).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
