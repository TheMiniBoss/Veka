//! Une fenêtre entièrement dessinée par Veka : barre de titre, boutons, contenu.
//! Elle se déplace, se redimensionne et garde les Snap Layouts de Windows 11.
//! `cargo run --example fenetre_perso`

use veka::prelude::*;

fn fenetre() -> impl View {
    let mut clics = State::new(0);

    VStack![spacing: 0;
        TitleBar("Veka — fenêtre personnalisée"),
        VStack![spacing: 16;
            Text("Aucune décoration du système").font(Font::Title2),
            Text("Tire la barre pour déplacer, double-clique pour agrandir."),
            HStack![spacing: 12;
                RoundedRectangle(12).fill(Color::Blue).frame(80, 48),
                Capsule().fill(Color::Orange).frame(96, 48),
                Circle().fill(Color::Green).frame(48, 48),
            ],
            Button(format!("Clics : {clics}"), move || clics += 1),
        ]
        .padding(24),
        Spacer(),
    ]
}

fn main() {
    let fenetre_perso = Window::new("Veka")
        .size(Size::new(560.0, 380.0))
        .min_size(Size::new(360.0, 240.0))
        .chrome(Chrome::custom());
    if let Err(e) = App::new().window_with(fenetre_perso, fenetre).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
