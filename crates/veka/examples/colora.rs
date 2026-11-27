//! Colora : jetons du thème, les cinq dégradés, un thème maison, le mode clair / sombre et les
//! réglages de couleur. `cargo run --example colora`

use veka::prelude::*;

const PALETTE: [Token; 12] = [
    Token::Red,
    Token::Orange,
    Token::Yellow,
    Token::Green,
    Token::Mint,
    Token::Teal,
    Token::Cyan,
    Token::Blue,
    Token::Indigo,
    Token::Purple,
    Token::Pink,
    Token::Brown,
];

fn nuancier() -> Element {
    let pastilles = PALETTE
        .iter()
        .map(|t| Circle().fill(t.clone()).frame(28, 28))
        .collect();
    veka::ui::views::stack(Axis::Horizontal, Some(8.0), pastilles)
}

fn carte(titre: &str, peinture: impl Into<Paint>) -> Element {
    VStack![spacing: 6;
        RoundedRectangle(14).fill(peinture).frame(112, 84),
        Text(titre).font(Font::Footnote).foreground(Token::Foreground.secondary()),
    ]
}

fn degrades() -> Element {
    let arc = [
        Token::Red,
        Token::Yellow,
        Token::Green,
        Token::Cyan,
        Token::Blue,
        Token::Purple,
        Token::Red,
    ];
    let maille = MeshGradient::grid(
        3,
        3,
        [
            Color::hex(0xFF2D55),
            Color::hex(0xFF9500),
            Color::hex(0xFFCC00),
            Color::hex(0xAF52DE),
            Color::hex(0xFFFFFF),
            Color::hex(0x34C759),
            Color::hex(0x5856D6),
            Color::hex(0x007AFF),
            Color::hex(0x00C7BE),
        ],
    );
    HStack![spacing: 12;
        carte(
            "Linéaire",
            LinearGradient::new([Token::Blue, Token::Purple])
                .from(UnitPoint::TOP_LEADING)
                .to(UnitPoint::BOTTOM_TRAILING),
        ),
        carte("Radial", RadialGradient::new([Token::Yellow, Token::Orange, Token::Red], 60.0)),
        carte("Elliptique", EllipticalGradient::new([Color::White, Color::hex(0x5AC8FA), Color::hex(0x0040DD)])),
        carte("Angulaire", AngularGradient::new(arc)),
        carte("Maillé", maille),
    ]
}

fn marque() -> Theme {
    Theme::new("Marque")
        .color(Token::Tint, Color::hex(0xFF6A00), Color::hex(0xFF8A33))
        .color(
            Token::custom("Marque.Bandeau"),
            LinearGradient::new([Color::hex(0xFF6A00), Color::hex(0xFF2D55)]),
            LinearGradient::new([Color::hex(0x7A2E00), Color::hex(0x4A0018)]),
        )
}

pub fn vue() -> impl View {
    // 0 = comme le système, 1 = clair, 2 = sombre.
    let mode = State::new(0);
    let gris = State::new(false);
    let libelle = match mode.get() {
        0 => "Mode : système",
        1 => "Mode : clair",
        _ => "Mode : sombre",
    };

    let contenu = VStack![spacing: 20;
        Text("Colora").font(Font::LargeTitle),
        VStack![spacing: 8;
            Text("Jetons").font(Font::Headline),
            nuancier(),
        ],
        VStack![spacing: 8;
            Text("Dégradés (Oklab)").font(Font::Headline),
            degrades(),
        ],
        VStack![spacing: 8;
            Text("Thème « Marque » sur cette partie").font(Font::Headline),
            HStack![spacing: 12;
                Button("Valider", || {}),
                Capsule().fill(Token::custom("Marque.Bandeau")).frame(160, 36),
            ]
            .theme(marque()),
        ],
        HStack![spacing: 12;
            Button(libelle, move || mode.update(|m| *m = (*m + 1) % 3)),
            Button(if gris.get() { "Couleurs" } else { "Niveaux de gris" }, move || gris.update(|g| *g = !*g)),
        ],
    ]
    .padding(24);

    let contenu = match mode.get() {
        1 => contenu.color_scheme(ColorScheme::Light),
        2 => contenu.color_scheme(ColorScheme::Dark),
        _ => contenu,
    };
    let contenu = if gris.get() {
        contenu.grayscale(1.0)
    } else {
        contenu
    };
    // Le fond de toute la fenêtre suit le mode choisi.
    let fond = Rectangle().fill(Token::WindowBackground);
    let fond = match mode.get() {
        1 => fond.color_scheme(ColorScheme::Light),
        2 => fond.color_scheme(ColorScheme::Dark),
        _ => fond,
    };
    ZStack![fond, contenu]
}

fn main() {
    let fenetre = Window::new("Colora").size(Size::new(680.0, 520.0));
    if let Err(e) = App::new().window_with(fenetre, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
