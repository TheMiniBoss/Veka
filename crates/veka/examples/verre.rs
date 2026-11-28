//! Verre : verre dépoli, verre liquide (façon 27), gouttes qui fusionnent et eau, au-dessus d'un fond
//! coloré qui bouge.
//! `cargo run --example verre` ; `-- mica`, `-- mica-alt` ou `-- acrylic` : sans le fond coloré, sur le
//! matériau du système (Windows 11, macOS).

use veka::prelude::*;

/// Un fond vif : dégradé et pastilles qui tournent lentement.
pub fn fond() -> impl View {
    let t = TimelineState::playing(
        Timeline::new()
            .track(
                "angle",
                [
                    Keyframe::new(0.0, 0.0).curve(Curve::Linear),
                    Keyframe::new(20.0, 360.0),
                ],
            )
            .length(20.0)
            .looping(true),
    );
    let a = t.value("angle");
    let pastille = |c: Token, d: f32, x: f32, y: f32| {
        Circle()
            .fill(RadialGradient::new(
                [Ink::from(c), Ink::from(Color::Clear)],
                d * 0.5,
            ))
            .frame(d, d)
            .offset(x, y)
    };
    ZStack![
        Rectangle().fill(
            LinearGradient::new([Token::Indigo, Token::Teal])
                .from(UnitPoint::TOP_LEADING)
                .to(UnitPoint::BOTTOM_TRAILING)
        ),
        ZStack![
            pastille(Token::Pink, 320.0, -180.0, -90.0),
            pastille(Token::Orange, 260.0, 190.0, 110.0),
            pastille(Token::Yellow, 200.0, 40.0, -150.0),
        ]
        .rotation_effect(a),
        // Des traits nets qui passent derrière le panneau et la barre : le flou se voit.
        veka::ui::views::stack(
            Axis::Horizontal,
            Some(14.0),
            (0..40)
                .map(|i| {
                    Capsule()
                        .fill(if i % 2 == 0 { Color::White } else { Color::Black })
                        .frame(6, 260)
                })
                .collect(),
        )
        .rotation_effect(-20.0),
        HStack![spacing: 18;
            Text("Veka").font(Font::Title).foreground(Color::White),
            Text("verre").font(Font::Title).foreground(Color::Black),
        ]
        .offset(-300.0, 230.0),
    ]
}

pub fn panneau() -> impl View {
    let flou = State::new(true);
    let f = flou.get();
    VStack![spacing: 10;
        Text("Verre dépoli").font(Font::Title2),
        Text("Le fond est flouté, teinté et un peu granuleux.").font(Font::Footnote),
        Button(if f { "Flou léger" } else { "Flou fort" }, move || {
            with_animation(Animation::smooth(), || flou.toggle())
        }),
    ]
    .padding(20)
    .frame(300, 150)
    .backdrop(BackdropBlur::frosted().radius(if f { 30.0 } else { 8.0 }))
}

/// Une barre d'onglets en verre liquide : trois boutons qui fusionnent quand on les rapproche.
pub fn barre() -> impl View {
    let serre = State::new(false);
    let sr = serre.get();
    let onglet = |nom: &'static str| {
        Text(nom)
            .padding_hv(18.0, 10.0)
            .glass_with(Glass::regular().interactive(true))
    };
    VStack![spacing: 12;
        HStack![spacing: if sr { 2.0 } else { 18.0 };
            onglet("Accueil"),
            onglet("Recherche"),
            onglet("Réglages"),
        ]
        .glass_group_with(14.0, Glass::regular().interactive(true)),
        Button(if sr { "Écarter" } else { "Rapprocher" }, move || {
            with_animation(Animation::spring(0.5, 0.7), || serre.toggle())
        }),
    ]
}

/// Une carte en verre clair : le bord tord le fond, le reflet suit le pointeur.
pub fn carte() -> impl View {
    let clarte = State::new(1usize);
    let niveaux = [0.0f32, 0.4, 0.9];
    let noms = ["Teinté", "Par défaut", "Clair"];
    let c = clarte.get();
    VStack![spacing: 10;
        Text("Liquid Glass").font(Font::Title2),
        Text(format!("Clarté : {}", noms[c])).font(Font::Footnote),
        Button("Changer", move || {
            with_animation(Animation::smooth(), || clarte.update(|v| *v = (*v + 1) % 3))
        }),
    ]
    .padding(22)
    .frame(240, 150)
    .glass_with(
        Glass::regular()
            .clarity(niveaux[c])
            .shape(Shape::RoundedRectangle(CornerRadii::all(28.0)))
            .interactive(true),
    )
}

/// Une goutte d'eau : le fond ondule dedans.
pub fn goutte() -> impl View {
    Text("Eau")
        .font(Font::Headline)
        .frame(110, 110)
        .liquid_distortion(10.0, 90.0, 1.0, Shape::Circle)
        .glass_with(Glass::clear().shape(Shape::Circle))
}

fn verres() -> Element {
    VStack![spacing: 28;
        HStack![spacing: 28;
            component(panneau),
            component(carte),
            component(goutte),
        ],
        component(barre),
    ]
    .into_element()
}

pub fn vue() -> impl View {
    ZStack![component(fond), verres()]
}

fn main() {
    let materiau = match std::env::args().nth(1).as_deref() {
        Some("mica") => SystemBackdrop::Mica,
        Some("mica-alt") => SystemBackdrop::MicaAlt,
        Some("acrylic") => SystemBackdrop::Acrylic,
        _ => SystemBackdrop::None,
    };
    let fenetre = Window::new("Verre")
        .size(Size::new(900.0, 560.0))
        .backdrop(materiau);
    let app = if materiau == SystemBackdrop::None {
        App::new().window_with(fenetre, vue)
    } else {
        App::new().window_with(fenetre, verres)
    };
    if let Err(e) = app.run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
