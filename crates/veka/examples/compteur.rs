//! L'exemple de l'étape 1.
//!
//! En attendant les fenêtres natives (PR B), il tourne sans fenêtre : il simule trois clics et
//! affiche la liste de dessin produite. `cargo run --example compteur`

use veka::prelude::*;
use veka::ui::{InputEvent, PointerButton};

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
    let mut surface = Surface::new(component(compteur));
    surface.resize(Size::new(480.0, 320.0), 2.0);
    surface.frame();

    let bouton = surface.rects_of("Button")[0].center();
    for _ in 0..3 {
        surface.handle(InputEvent::PointerDown {
            position: bouton,
            button: PointerButton::Primary,
        });
        surface.handle(InputEvent::PointerUp {
            position: bouton,
            button: PointerButton::Primary,
        });
    }
    let liste = surface.frame();
    println!("{liste}");
    println!("Textes : {:?}", surface.texts());
}
