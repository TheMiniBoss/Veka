use std::cell::Cell;
use std::rc::Rc;

use veka_ui::prelude::*;

#[test]
fn etat_conserve_et_vues_imbriquees() {
    fn ligne() -> impl View {
        let mut n = State::new(10);
        Button(format!("ligne {n}"), move || n += 5)
    }
    fn parent() -> impl View {
        let mut n = State::new(0);
        VStack![Button(format!("parent {n}"), move || n += 1), component(ligne)]
    }
    let mut s = Surface::new(component(parent));
    s.resize(Size::new(300.0, 300.0), 1.0);
    s.frame();
    let b = s.rects_of("Button");
    let (p, l) = (b[0].center(), b[1].center());
    let tap = |s: &mut Surface, at: Point| {
        s.handle(veka_ui::InputEvent::PointerDown {
            position: at,
            button: veka_ui::PointerButton::Primary,
        });
        s.handle(veka_ui::InputEvent::PointerUp {
            position: at,
            button: veka_ui::PointerButton::Primary,
        });
        s.frame();
    };
    tap(&mut s, l);
    assert_eq!(s.texts(), vec!["parent 0", "ligne 15"]);
    // Ré-évaluer le parent garde l'état de la vue enfant.
    tap(&mut s, p);
    assert_eq!(s.texts(), vec!["parent 1", "ligne 15"]);
}

#[test]
fn le_waker_est_appele_hors_evenement() {
    thread_local!(static ETAT: Cell<Option<State<i32>>> = const { Cell::new(None) });
    fn vue() -> impl View {
        let n = State::new(1);
        ETAT.with(|e| e.set(Some(n)));
        Text(format!("{n}"))
    }
    let mut s = Surface::new(component(vue));
    s.resize(Size::new(100.0, 100.0), 1.0);
    s.frame();
    let woken = Rc::new(Cell::new(0));
    let w = woken.clone();
    s.set_waker(move || w.set(w.get() + 1));
    let n = ETAT.with(|e| e.get()).unwrap();
    n.set(2);
    n.set(3);
    assert_eq!(woken.get(), 1, "un seul réveil pour plusieurs changements");
    assert!(s.needs_frame());
    s.frame();
    assert_eq!(s.texts(), vec!["3"]);
}

#[test]
fn etats_supprimes_avec_la_surface() {
    thread_local!(static ETAT: Cell<Option<State<i32>>> = const { Cell::new(None) });
    fn vue() -> impl View {
        let n = State::new(1);
        ETAT.with(|e| e.set(Some(n)));
        Text("x")
    }
    let s = Surface::new(component(vue));
    let n = ETAT.with(|e| e.get()).unwrap();
    assert!(n.is_alive());
    drop(s);
    assert!(!n.is_alive());
}

#[test]
#[should_panic(expected = "pendant l'évaluation d'une vue")]
fn state_hors_vue() {
    let _ = State::new(0);
}

#[test]
#[should_panic(expected = "même ordre")]
fn ordre_des_etats_verifie() {
    thread_local!(static FLAG: Cell<bool> = const { Cell::new(false) });
    fn vue() -> impl View {
        if FLAG.with(|f| f.get()) {
            let _texte = State::new(String::new());
        }
        let mut n = State::new(0);
        Button(format!("{n}"), move || {
            FLAG.with(|f| f.set(true));
            n += 1;
        })
    }
    let mut s = Surface::new(component(vue));
    s.resize(Size::new(100.0, 100.0), 1.0);
    s.frame();
    let at = s.rects_of("Button")[0].center();
    s.handle(veka_ui::InputEvent::PointerDown {
        position: at,
        button: veka_ui::PointerButton::Primary,
    });
    s.handle(veka_ui::InputEvent::PointerUp {
        position: at,
        button: veka_ui::PointerButton::Primary,
    });
    s.frame();
}
