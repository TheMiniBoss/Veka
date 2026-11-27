//! Verre : un flou d'arrière-plan et un panneau en verre dépoli au-dessus d'un fond coloré qui bouge.
//! `cargo run --example verre`

use veka::prelude::*;

/// Un fond vif : dégradé et pastilles qui tournent lentement.
pub fn fond() -> impl View {
    let t = TimelineState::playing(
        Timeline::new()
            .track(
                "angle",
                [
                    Keyframe::new(0.0, 0.0),
                    Keyframe::new(20.0, 360.0).curve(Curve::Linear),
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
            (0..24)
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
        .offset(0.0, 106.0),
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

pub fn barre() -> impl View {
    HStack![spacing: 16;
        Text("Accueil"),
        Text("Recherche"),
        Text("Réglages"),
    ]
    .padding_hv(24.0, 12.0)
    .backdrop(BackdropBlur::default().radius(16.0).shape(Shape::Capsule))
}

pub fn vue() -> impl View {
    ZStack![
        component(fond),
        VStack![spacing: 24;
            component(panneau),
            component(barre),
        ],
    ]
}

fn main() {
    let fenetre = Window::new("Verre").size(Size::new(720.0, 480.0));
    if let Err(e) = App::new().window_with(fenetre, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
