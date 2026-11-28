//! Modes de fusion : un même motif dans chaque case, mélangé avec le dégradé de la case par un mode
//! différent ; en bas, une opacité sans et avec `compositing_group`. `cargo run --example fusion`
//! (même dessin que `vk/fusion.vk`, testé).

use veka::prelude::*;

pub fn case(mode: BlendMode, angle: f32) -> impl View {
    let cercle = |c: Token, x: f32, y: f32| Circle().fill(c).frame(56, 56).offset(x, y);
    VStack![spacing: 6;
        ZStack![
            RoundedRectangle(12.0).fill(
                LinearGradient::new([Token::Yellow, Token::Teal])
                    .from(UnitPoint::TOP_LEADING)
                    .to(UnitPoint::BOTTOM_TRAILING)
            ),
            ZStack![cercle(Token::Pink, -14.0, -8.0), cercle(Token::Indigo, 14.0, 8.0)]
                .rotation_effect(angle)
                .blend_mode(mode),
        ]
        .frame(120, 96)
        // Le mélange reste dans la case (les découpes ne percent pas la fenêtre).
        .compositing_group(),
        Text(mode.name()).font(Font::Caption),
    ]
}

fn cercles() -> Element {
    ZStack![
        Circle().fill(Token::Blue).frame(60, 60).offset(-18.0, 0.0),
        Circle().fill(Token::Blue).frame(60, 60).offset(18.0, 0.0),
    ]
    .frame(120, 64)
}

pub fn opacite(groupe: bool) -> impl View {
    if groupe {
        VStack![spacing: 6;
            cercles().compositing_group().opacity(0.5),
            Text("compositingGroup().opacity(0.5)").font(Font::Caption),
        ]
    } else {
        VStack![spacing: 6;
            cercles().opacity(0.5),
            Text("opacity(0.5)").font(Font::Caption),
        ]
    }
}

pub fn vue() -> impl View {
    let tour = State::new(0.0f32);
    let angle = tour.get();
    let lignes = (0..3)
        .map(|r| {
            veka::ui::views::stack(
                Axis::Horizontal,
                Some(10.0),
                (0..7)
                    .map(|c| component(move || case(BlendMode::ALL[r * 7 + c], angle)))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    ZStack![
        Rectangle().fill(Token::WindowBackground),
        veka::ui::views::stack(
            Axis::Vertical,
            Some(16.0),
            [HStack![spacing: 16;
                Text("Modes de fusion").font(Font::Title2),
                Button("Tourner", move || {
                    let mut tour = tour;
                    with_animation(Animation::smooth(), move || tour += 90.0)
                }),
            ],]
            .into_iter()
            .chain(lignes)
            .chain([HStack![spacing: 40;
                component(|| opacite(false)),
                component(|| opacite(true)),
            ]])
            .collect(),
        )
        .padding(24),
    ]
}

fn main() {
    let fenetre = Window::new("Modes de fusion").size(Size::new(980.0, 600.0));
    if let Err(e) = App::new().window_with(fenetre, vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
