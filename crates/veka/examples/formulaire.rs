//! Champs de texte (étape 9) : nom, e-mail et mot de passe. Tab passe d'un champ à l'autre, Entrée valide et
//! Échap efface. `cargo run --example formulaire` (même dessin que `vk/formulaire.vk`, testé).

use veka::prelude::*;

pub fn vue() -> impl View {
    let nom = State::new(String::new());
    let email = State::new(String::new());
    let mdp = State::new(String::new());
    let message = State::new(String::from("Remplissez le formulaire, puis Entrée."));
    let valider = move || {
        if nom.get().is_empty() {
            message.set("Il manque le nom.".into());
        } else {
            message.set(format!("Bienvenue, {} ({}) !", nom.get(), email.get()));
        }
    };
    let effacer = move || {
        nom.set(String::new());
        email.set(String::new());
        mdp.set(String::new());
        message.set("Formulaire effacé.".into());
    };
    VStack![alignment: Alignment::Leading, spacing: 12;
        Text("Inscription").font(Font::Title),
        Text("Nom"),
        TextField("Ana Martin", nom).frame_width(280),
        Text("E-mail"),
        TextField("ana@exemple.fr", email).frame_width(280),
        Text("Mot de passe"),
        SecureField("Au moins 8 caractères", mdp).frame_width(280),
        Text(message.get()).font(Font::Headline),
        HStack![spacing: 12;
            Button("Effacer", effacer).keyboard_shortcut(KeyEquivalent::Escape, Modifiers::NONE),
            Button("Valider", valider),
        ],
        Text("Tab : champ suivant · Entrée : valider · Échap : effacer").font(Font::Caption),
    ]
    .on_submit(valider)
    .padding(24)
}

#[allow(dead_code)]
fn main() {
    if let Err(e) = App::new().window("Formulaire", vue).run() {
        eprintln!("Veka : {e}");
        std::process::exit(1);
    }
}
