//! Champs de texte (étape 9), sans fenêtre : frappe, déplacements, sélection, copier-coller,
//! annulation, Entrée, champ secret, composition IME.

use std::cell::Cell;
use std::rc::Rc;

use veka_ui::prelude::*;
use veka_ui::{InputEvent, PointerButton, TextInput};

type Etat = Rc<Cell<Option<State<String>>>>;

fn formulaire(secret: bool, valide: Rc<Cell<u32>>) -> (Surface, Etat) {
    let etat: Etat = Rc::new(Cell::new(None));
    let e = etat.clone();
    let mut s = Surface::new(component(move || {
        let nom = State::new(String::new());
        e.set(Some(nom));
        let v = valide.clone();
        let champ = if secret {
            SecureField("Mot de passe", nom)
        } else {
            TextField("Nom", nom)
        };
        VStack![spacing: 10;
            champ.frame_width(200),
            Text(format!("[{}]", nom.get())),
        ]
        .on_submit(move || v.set(v.get() + 1))
    }));
    s.resize(Size::new(400.0, 200.0), 1.0);
    s.frame();
    (s, etat)
}

fn key(s: &mut Surface, key: KeyEquivalent, modifiers: Modifiers) {
    s.handle(InputEvent::Key {
        key,
        modifiers,
        pressed: true,
        repeat: false,
    });
    s.frame();
}

fn type_text(s: &mut Surface, t: &str) {
    for c in t.chars() {
        s.handle(InputEvent::Char(c));
    }
    s.frame();
}

fn click(s: &mut Surface, p: Point) {
    s.handle(InputEvent::PointerDown {
        position: p,
        button: PointerButton::Primary,
    });
    s.handle(InputEvent::PointerUp {
        position: p,
        button: PointerButton::Primary,
    });
    s.frame();
}

fn champ(s: &Surface, nom: &str) -> Rect {
    s.rects_of(nom)[0]
}

#[test]
fn taper_dans_un_champ() {
    let (mut s, etat) = formulaire(false, Rc::default());
    let c = champ(&s, "TextField");
    // Sans focus, la frappe ne va nulle part.
    type_text(&mut s, "x");
    assert_eq!(etat.get().unwrap().get(), "");
    click(&mut s, c.center());
    assert!(s.text_input_area().is_some());
    type_text(&mut s, "Bonjour");
    assert_eq!(etat.get().unwrap().get(), "Bonjour");
    assert!(s.texts().contains(&"[Bonjour]".to_string()), "{:?}", s.texts());
    // Le champ dessine son texte, son curseur et l'anneau du focus.
    let d = s.draw_list().to_string();
    assert!(d.contains("glyphs"), "{d}");
    key(&mut s, KeyEquivalent::Backspace, Modifiers::NONE);
    key(&mut s, KeyEquivalent::Home, Modifiers::NONE);
    type_text(&mut s, "« ");
    assert_eq!(etat.get().unwrap().get(), "« Bonjou");
}

#[test]
fn selection_copier_coller_annuler() {
    let (mut s, etat) = formulaire(false, Rc::default());
    let r = champ(&s, "TextField");
    click(&mut s, r.center());
    type_text(&mut s, "un deux");
    key(&mut s, KeyEquivalent::Character('a'), Modifiers::COMMAND);
    key(&mut s, KeyEquivalent::Character('c'), Modifiers::COMMAND);
    assert_eq!(veka_ui::clipboard_text().as_deref(), Some("un deux"));
    key(&mut s, KeyEquivalent::End, Modifiers::NONE);
    key(&mut s, KeyEquivalent::Character('v'), Modifiers::COMMAND);
    assert_eq!(etat.get().unwrap().get(), "un deuxun deux");
    // Annuler revient avant le collage, refaire le remet.
    key(&mut s, KeyEquivalent::Character('z'), Modifiers::COMMAND);
    assert_eq!(etat.get().unwrap().get(), "un deux");
    key(&mut s, KeyEquivalent::Character('z'), Modifiers::COMMAND | Modifiers::SHIFT);
    assert_eq!(etat.get().unwrap().get(), "un deuxun deux");
    // La frappe s'annule d'un bloc.
    key(&mut s, KeyEquivalent::Character('z'), Modifiers::COMMAND);
    key(&mut s, KeyEquivalent::Character('z'), Modifiers::COMMAND);
    assert_eq!(etat.get().unwrap().get(), "");
    // Maj+flèche sélectionne, couper retire.
    type_text(&mut s, "abc");
    key(&mut s, KeyEquivalent::ArrowLeft, Modifiers::SHIFT);
    key(&mut s, KeyEquivalent::Character('x'), Modifiers::COMMAND);
    assert_eq!(etat.get().unwrap().get(), "ab");
    assert_eq!(veka_ui::clipboard_text().as_deref(), Some("c"));
}

#[test]
fn entree_valide_et_une_touche_seule_ne_fait_pas_de_raccourci() {
    let valide = Rc::new(Cell::new(0));
    let (mut s, etat) = formulaire(false, valide.clone());
    let r = champ(&s, "TextField");
    click(&mut s, r.center());
    key(&mut s, KeyEquivalent::Character('r'), Modifiers::NONE);
    type_text(&mut s, "r");
    key(&mut s, KeyEquivalent::Enter, Modifiers::NONE);
    assert_eq!(valide.get(), 1);
    assert_eq!(etat.get().unwrap().get(), "r");
}

#[test]
fn le_texte_change_par_le_code_s_affiche() {
    let (mut s, etat) = formulaire(false, Rc::default());
    etat.get().unwrap().set("du code".into());
    s.frame();
    let r = champ(&s, "TextField");
    click(&mut s, r.center());
    key(&mut s, KeyEquivalent::End, Modifiers::NONE);
    type_text(&mut s, " !");
    assert_eq!(etat.get().unwrap().get(), "du code !");
}

#[test]
fn champ_secret() {
    let (mut s, etat) = formulaire(true, Rc::default());
    let r = champ(&s, "SecureField");
    click(&mut s, r.center());
    type_text(&mut s, "secret");
    assert_eq!(etat.get().unwrap().get(), "secret");
    // Rien n'est copié depuis un champ secret.
    veka_ui::set_clipboard_text("avant");
    key(&mut s, KeyEquivalent::Character('a'), Modifiers::COMMAND);
    key(&mut s, KeyEquivalent::Character('c'), Modifiers::COMMAND);
    assert_eq!(veka_ui::clipboard_text().as_deref(), Some("avant"));
}

#[test]
fn composition_ime() {
    let (mut s, etat) = formulaire(false, Rc::default());
    let r = champ(&s, "TextField");
    click(&mut s, r.center());
    s.text_input(TextInput::Compose {
        text: "にほ".into(),
        cursor: None,
    });
    s.frame();
    // La composition n'est pas encore dans l'état.
    assert_eq!(etat.get().unwrap().get(), "");
    s.text_input(TextInput::Commit("日本".into()));
    s.frame();
    assert_eq!(etat.get().unwrap().get(), "日本");
}
