//! Anima : une carte à ressort, une liste avec transitions, une couleur animée, la galerie des
//! courbes, une pastille à géométrie liée et une pulsation en timeline. `cargo run --example anima`

use veka::prelude::*;

pub fn carte() -> impl View {
    let ouvert = State::new(false);
    let o = ouvert.get();
    VStack![spacing: 8;
        RoundedRectangle(if o { 24 } else { 12 })
            .fill(if o {
                Paint::from(LinearGradient::new([Token::Indigo, Token::Pink]).from(UnitPoint::TOP_LEADING).to(UnitPoint::BOTTOM_TRAILING))
            } else {
                Paint::from(LinearGradient::new([Token::Blue, Token::Teal]).from(UnitPoint::TOP_LEADING).to(UnitPoint::BOTTOM_TRAILING))
            })
            .frame(if o { 300 } else { 140 }, if o { 120 } else { 64 })
            .on_tap(move || with_animation(Animation::bouncy(), || ouvert.update(|v| *v = !*v))),
        Text("Touche la carte (ressort « bouncy »)")
            .font(Font::Footnote)
            .opacity(if o { 0.4 } else { 1.0 })
            .animation(Animation::ease_in_out(0.25), o),
    ]
}

pub fn liste() -> impl View {
    let prochain = State::new(4u32);
    let ids = State::new(vec![1u32, 2, 3]);
    let lignes: Vec<Element> = ids
        .get()
        .iter()
        .map(|&i| {
            HStack![spacing: 8;
                Circle().fill(Token::Accent).frame(10, 10),
                Text(format!("Élément {i}")),
            ]
            .padding_hv(12.0, 6.0)
            .background_shape(Token::Fill, Shape::Capsule)
            .id(i as i64)
            .transition(Transition::asymmetric(
                // Descend un peu en apparaissant (les lignes s'ajoutent en tête, sous les boutons),
                // part en se réduisant.
                Transition::offset(0.0, -12.0)
                    .combined(Transition::scale(0.9))
                    .combined(Transition::opacity()),
                Transition::scale(0.8).combined(Transition::opacity()),
            ))
        })
        .collect();
    VStack![spacing: 8;
        HStack![spacing: 8;
            Button("Ajouter", move || {
                with_animation(Animation::snappy(), || {
                    let i = prochain.get();
                    prochain.set(i + 1);
                    ids.update(|v| v.insert(0, i));
                })
            }),
            Button("Retirer", move || with_animation(Animation::smooth(), || ids.update(|v| { v.pop(); }))),
        ],
        veka::ui::views::stack(Axis::Vertical, Some(6.0), lignes),
    ]
}

pub fn couleur() -> impl View {
    let chaud = State::new(false);
    let c = chaud.get();
    HStack![spacing: 12;
        Button(if c { "Froid" } else { "Chaud" }, move || {
            with_animation(Animation::ease_in_out(0.6), || chaud.update(|v| *v = !*v))
        })
        .tint(if c { Color::hex(0xFF6A00) } else { Color::hex(0x0A84FF) }),
        Circle()
            .fill(if c { Color::hex(0xFF2D55) } else { Color::hex(0x00C7BE) })
            .frame(36, 36)
            .scale_effect(if c { 1.2 } else { 1.0 })
            .rotation_effect(if c { 90.0 } else { 0.0 }),
        Text("Couleurs en Oklab").font(Font::Footnote).grayscale(if c { 0.0 } else { 1.0 }),
    ]
}

pub fn galerie() -> impl View {
    let droite = State::new(false);
    let d = droite.get();
    let courbes = [
        ("Linéaire", Animation::linear(1.0)),
        ("Ease in-out", Animation::ease_in_out(1.0)),
        ("Circ. out", Animation::circ_ease_out(1.0)),
        ("Bézier", Animation::timing_curve(0.3, 1.6, 0.6, 1.0, 1.0)),
        ("Ressort", Animation::bouncy()),
    ];
    let lignes: Vec<Element> = courbes
        .into_iter()
        .map(|(nom, spec)| {
            HStack![spacing: 12;
                Text(nom).font(Font::Footnote).frame_width(80),
                ZStack![
                    Capsule().fill(Token::Fill).frame(240, 4),
                    Circle().fill(Token::Accent).frame(16, 16).offset(if d { 112.0 } else { -112.0 }, 0.0),
                ]
                .animation(spec, d),
            ]
        })
        .collect();
    VStack![spacing: 6;
        Button("Lancer les courbes", move || droite.update(|v| *v = !*v)),
        veka::ui::views::stack(Axis::Vertical, Some(6.0), lignes),
    ]
}

pub fn geometrie() -> impl View {
    let droite = State::new(false);
    let d = droite.get();
    let pastille = Circle()
        .fill(Token::Orange)
        .frame(if d { 44 } else { 20 }, if d { 44 } else { 20 })
        .matched_geometry_effect("pastille")
        .id(if d { 2 } else { 1 });
    let rail = Capsule().fill(Token::Fill).frame(180, 6).id(0);
    let ligne = if d {
        vec![rail, pastille]
    } else {
        vec![pastille, rail]
    };
    VStack![spacing: 8;
        Button("Déplacer", move || {
            with_animation(Animation::spring(0.45, 0.75), || droite.toggle())
        }),
        veka::ui::views::stack(Axis::Horizontal, Some(12.0), ligne),
    ]
}

pub fn pulsation() -> impl View {
    let battements = State::new(0u32);
    let tl = TimelineState::playing(
        Timeline::new()
            .track(
                "échelle",
                [
                    Keyframe::new(0.0, 1.0).curve(Curve::EaseOut),
                    Keyframe::new(0.15, 1.35),
                    Keyframe::new(0.5, 1.0).curve(Curve::Linear),
                ],
            )
            .track(
                "opacité",
                [
                    Keyframe::new(0.0, 0.6).curve(Curve::EaseOut),
                    Keyframe::new(0.15, 1.0),
                    Keyframe::new(0.9, 0.6),
                ],
            )
            .event(0.15, "battement")
            .length(0.9)
            .looping(true),
    );
    tl.on_event(move |_| {
        let mut b = battements;
        b += 1;
    });
    HStack![spacing: 12;
        Button(if tl.is_playing() { "Pause" } else { "Lecture" }, move || {
            if tl.is_playing() { tl.pause() } else { tl.play() }
        }),
        Circle()
            .fill(Token::Pink)
            .frame(28, 28)
            .scale_effect(tl.value("échelle"))
            .opacity(tl.value("opacité"))
            .frame(44, 44),
        Text(format!("Battements : {battements}")).font(Font::Footnote),
    ]
}

pub fn vue() -> impl View {
    let fond = Rectangle().fill(Token::WindowBackground);
    let contenu = HStack![spacing: 32;
        VStack![spacing: 24;
            Text("Anima").font(Font::LargeTitle),
            component(carte),
            component(couleur),
            component(galerie),
            component(geometrie),
            component(pulsation),
        ],
        component(liste),
    ]
    .padding(24);
    ZStack![fond, contenu]
}

fn main() {
    let fenetre = Window::new("Anima").size(Size::new(860.0, 720.0));
    if let Err(e) = App::new().window_with(fenetre, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
