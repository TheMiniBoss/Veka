//! Clavier et focus (étape 9) : Tab passe d'une case à l'autre, Entrée sur une case ajoute 10, Ctrl+R
//! (Cmd+R sur Mac) compte, Échap remet à zéro. `cargo run --example clavier` (même dessin que `vk/clavier.vk`,
//! testé).

use veka::prelude::*;

fn tuile(nom: &'static str, couleur: Token, focus: State<Option<String>>, n: State<i64>) -> Element {
    ZStack![
        RoundedRectangle(12).fill(couleur).frame(100, 70),
        Text(nom).font(Font::Headline).foreground(Color::White),
    ]
    .focusable()
    .focused_equals(focus, nom.to_string())
    .on_key_press(KeyEquivalent::Enter, move |_| {
        n.update(|v| *v += 10);
        KeyPressResult::Handled
    })
}

pub fn vue() -> impl View {
    let mut n = State::new(0i64);
    let focus = State::new(None::<String>);
    let actuel = focus.get().unwrap_or_else(|| "aucun".into());
    VStack![spacing: 16;
        Text("Tab : case suivante · Entrée : +10 · Ctrl+R (Cmd+R sur Mac) : +1 · Échap : zéro").font(Font::Caption),
        HStack![spacing: 12;
            tuile("Rouge", Token::Red, focus, n),
            tuile("Vert", Token::Green, focus, n),
            tuile("Bleu", Token::Blue, focus, n),
        ],
        Text(format!("Focus : {actuel}")),
        Text(format!("Compteur : {n}")).font(Font::Title),
        HStack![spacing: 12;
            Button("Compter", move || n += 1)
                .keyboard_shortcut(KeyEquivalent::Character('r'), Modifiers::COMMAND),
            Button("Remettre à zéro", move || n.set(0))
                .keyboard_shortcut(KeyEquivalent::Escape, Modifiers::NONE),
        ],
    ]
    .padding(24)
}

#[allow(dead_code)]
fn main() {
    if let Err(e) = App::new().window("Clavier", vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
