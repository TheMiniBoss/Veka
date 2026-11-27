//! Anima : une carte à ressort, une liste avec transitions, une couleur animée et la galerie des
//! courbes. `cargo run --example anima`

use veka::prelude::*;

fn carte() -> impl View {
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

fn liste() -> impl View {
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
                // Arrive d'en haut (les lignes s'ajoutent en tête), part en se réduisant.
                Transition::move_edge(Edge::Top).combined(Transition::opacity()),
                Transition::scale(0.6).combined(Transition::opacity()),
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

fn couleur() -> impl View {
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

fn galerie() -> impl View {
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

pub fn vue() -> impl View {
    let fond = Rectangle().fill(Token::WindowBackground);
    let contenu = HStack![spacing: 32;
        VStack![spacing: 24;
            Text("Anima").font(Font::LargeTitle),
            component(carte),
            component(couleur),
            component(galerie),
        ],
        component(liste),
    ]
    .padding(24);
    ZStack![fond, contenu]
}

fn main() {
    let fenetre = Window::new("Anima").size(Size::new(820.0, 560.0));
    if let Err(e) = App::new().window_with(fenetre, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
