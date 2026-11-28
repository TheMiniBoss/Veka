//! Défilement (étape 10) : une longue liste, des cartes qui défilent de côté, et des boutons qui vont en
//! haut, au milieu et en bas. `cargo run --example liste` (même dessin que `vk/liste.vk`, testé).

use veka::prelude::*;

fn carte(c: usize) -> Element {
    ZStack![
        RoundedRectangle(12).fill(Token::Indigo),
        Text(format!("Carte {c}")).foreground(Color::White),
    ]
    .frame(120, 80)
}

fn ligne(i: usize) -> Element {
    HStack![spacing: 12;
        Circle().fill(Token::Green).frame(24, 24),
        Text(format!("Ligne {i}")),
        Spacer(),
    ]
    .padding_hv(16, 6)
    .id(i as i64)
}

pub fn vue() -> impl View {
    ScrollViewReader(|proxy| {
        let cartes = ScrollView![.horizontal;
            stack(Axis::Horizontal, Some(12.0), (0..12).map(carte).collect()).padding_hv(16, 0)
        ];
        let mut contenu = vec![cartes];
        contenu.extend((0..100).map(ligne));
        VStack![spacing: 12;
            Text("Liste").font(Font::Title),
            HStack![spacing: 12;
                Button("Haut", move || {
                    with_animation(Animation::smooth(), || proxy.scroll_to(0, Some(UnitPoint::TOP)))
                }),
                Button("Ligne 50", move || proxy.scroll_to(50, Some(UnitPoint::CENTER))),
                Button("Bas", move || {
                    with_animation(Animation::smooth(), || proxy.scroll_to(99, Some(UnitPoint::BOTTOM)))
                }),
            ],
            ScrollView![stack(Axis::Vertical, Some(8.0), contenu)],
            Text("Molette ou pavé tactile · Page préc. / suiv. · Origine / Fin").font(Font::Caption),
        ]
        .padding(24)
    })
}

#[allow(dead_code)]
fn main() {
    if let Err(e) = App::new().window("Liste", vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
