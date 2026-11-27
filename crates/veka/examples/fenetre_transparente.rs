//! Une fenêtre transparente : le bureau se voit autour de la carte et à travers le disque.
//! `cargo run --example fenetre_transparente`

use veka::prelude::*;

fn vue() -> impl View {
    let mut clics = State::new(0);

    VStack![spacing: 16;
        HStack![
            Text("Veka — fenêtre transparente").font(Font::Headline),
            Spacer(),
            WindowButton(WindowControl::Close),
        ]
        .window_drag_area(),
        Text("Tout autour, c'est le bureau."),
        HStack![spacing: 12;
            Circle().fill(Color::rgba(0.0, 0.48, 1.0, 0.5)).frame(64, 64),
            Button(format!("Clics : {clics}"), move || clics += 1),
        ],
    ]
    .padding(24)
    // Le fond de fenêtre du thème : clair ou sombre comme le texte (un blanc fixe rendait le texte
    // blanc du mode sombre illisible).
    .background_shape(
        Token::WindowBackground,
        Shape::RoundedRectangle(CornerRadii::all(16.0)),
    )
    .padding(16)
}

fn main() {
    let fenetre_transparente = Window::new("Veka transparente")
        .size(Size::new(440.0, 240.0))
        .chrome(Chrome::transparent());
    if let Err(e) = App::new().window_with(fenetre_transparente, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
