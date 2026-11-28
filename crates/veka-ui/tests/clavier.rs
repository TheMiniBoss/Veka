//! Clavier et focus (étape 9), sans fenêtre : touches, Tab, `.focused`, raccourcis.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use veka_ui::prelude::*;
use veka_ui::{InputEvent, PointerButton};

fn surface(view: impl View + 'static) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(400.0, 300.0), 1.0);
    s.frame();
    s
}

fn press(s: &mut Surface, key: KeyEquivalent, modifiers: Modifiers) {
    s.handle(InputEvent::Key {
        key,
        modifiers,
        pressed: true,
        repeat: false,
    });
    s.handle(InputEvent::Key {
        key,
        modifiers,
        pressed: false,
        repeat: false,
    });
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

fn case(n: u32) -> Element {
    Rectangle()
        .fill(Color::Gray)
        .frame(60, 40)
        .focusable()
        .id(n as i64)
}

#[test]
fn tab_passe_d_une_vue_a_l_autre() {
    let mut s = surface(component(|| HStack![spacing: 10; case(1), case(2), case(3)]));
    let cases = s.rects_of("focusable");
    assert_eq!(cases.len(), 3);
    assert_eq!(s.focused_rect(), None);
    press(&mut s, KeyEquivalent::Tab, Modifiers::NONE);
    assert_eq!(s.focused_rect(), Some(cases[0]));
    press(&mut s, KeyEquivalent::Tab, Modifiers::NONE);
    assert_eq!(s.focused_rect(), Some(cases[1]));
    // Maj+Tab revient en arrière, et le tour boucle.
    press(&mut s, KeyEquivalent::Tab, Modifiers::SHIFT);
    press(&mut s, KeyEquivalent::Tab, Modifiers::SHIFT);
    assert_eq!(s.focused_rect(), Some(cases[2]));
    // L'anneau du focus est dessiné.
    assert!(s.draw_list().to_string().contains("stroke"), "{}", s.draw_list());
}

#[test]
fn un_clic_donne_et_retire_le_focus() {
    let mut s = surface(component(
        || VStack![spacing: 20; case(1), Rectangle().frame(60, 40)],
    ));
    let c = s.rects_of("focusable")[0];
    click(&mut s, c.center());
    assert_eq!(s.focused_rect(), Some(c));
    click(&mut s, Point::new(2.0, 2.0));
    assert_eq!(s.focused_rect(), None);
}

#[test]
fn la_touche_remonte_jusqu_a_qui_la_prend() {
    let log = Rc::new(RefCell::new(Vec::<&str>::new()));
    let l = log.clone();
    let mut s = surface(component(move || {
        let (a, b, c) = (l.clone(), l.clone(), l.clone());
        VStack![case(1).on_key_press(KeyEquivalent::Character('x'), move |_| {
            a.borrow_mut().push("case x");
            KeyPressResult::Handled
        }),]
        .on_key_press(KeyEquivalent::Character('x'), move |_| {
            b.borrow_mut().push("pile x");
            KeyPressResult::Handled
        })
        .on_any_key_press(move |k| {
            c.borrow_mut().push(if k.key == KeyEquivalent::Enter {
                "pile entrée"
            } else {
                "pile autre"
            });
            KeyPressResult::Ignored
        })
    }));
    // Sans focus, la vue la plus profonde la prend.
    press(&mut s, KeyEquivalent::Character('x'), Modifiers::NONE);
    assert_eq!(*log.borrow(), ["case x"]);
    log.borrow_mut().clear();
    // Avec le focus sur la case : sa touche, puis celles des parents.
    press(&mut s, KeyEquivalent::Tab, Modifiers::NONE);
    log.borrow_mut().clear();
    press(&mut s, KeyEquivalent::Enter, Modifiers::NONE);
    assert_eq!(*log.borrow(), ["pile entrée"]);
}

#[test]
fn focused_relie_le_focus_a_un_etat() {
    let etat: Rc<Cell<Option<State<Option<u32>>>>> = Rc::new(Cell::new(None));
    let e = etat.clone();
    let mut s = surface(component(move || {
        let focus = State::new(None::<u32>);
        e.set(Some(focus));
        let label = format!("focus {:?}", focus.get());
        VStack![spacing: 10;
            Text(label),
            case(1).focused_equals(focus, 1),
            case(2).focused_equals(focus, 2),
        ]
    }));
    let focus = etat.get().unwrap();
    let cases = s.rects_of("focusable");
    // L'utilisateur donne le focus : l'état suit.
    click(&mut s, cases[1].center());
    assert_eq!(focus.get(), Some(2));
    assert!(
        s.texts().contains(&"focus Some(2)".to_string()),
        "{:?}",
        s.texts()
    );
    // Le code donne le focus : la vue suit.
    focus.set(Some(1));
    s.frame();
    assert_eq!(s.focused_rect(), Some(cases[0]));
    // Le code le retire.
    focus.set(None);
    s.frame();
    assert_eq!(s.focused_rect(), None);
}

#[test]
fn focused_avec_un_booleen_des_le_depart() {
    let s = surface(component(|| {
        let actif = State::new(true);
        VStack![spacing: 10; case(1), case(2).focused(actif)]
    }));
    let cases = s.rects_of("focusable");
    assert_eq!(s.focused_rect(), Some(cases[1]));
}

#[test]
fn un_raccourci_declenche_le_bouton() {
    let n = Rc::new(Cell::new(0));
    let annule = Rc::new(Cell::new(false));
    let (n2, a2) = (n.clone(), annule.clone());
    let mut s = surface(component(move || {
        let (n3, a3) = (n2.clone(), a2.clone());
        VStack![
            Button("Enregistrer", move || n3.set(n3.get() + 1))
                .keyboard_shortcut(KeyEquivalent::Character('s'), Modifiers::COMMAND),
            Button("Annuler", move || a3.set(true)).keyboard_shortcut(KeyEquivalent::Escape, Modifiers::NONE),
        ]
    }));
    press(&mut s, KeyEquivalent::Character('s'), Modifiers::COMMAND);
    assert_eq!(n.get(), 1);
    // Sans le bon modificateur : rien.
    press(&mut s, KeyEquivalent::Character('s'), Modifiers::NONE);
    assert_eq!(n.get(), 1);
    // Une touche répétée ne relance pas le raccourci.
    s.handle(InputEvent::Key {
        key: KeyEquivalent::Character('s'),
        modifiers: Modifiers::COMMAND,
        pressed: true,
        repeat: true,
    });
    assert_eq!(n.get(), 1);
    press(&mut s, KeyEquivalent::Escape, Modifiers::NONE);
    assert!(annule.get());
}
