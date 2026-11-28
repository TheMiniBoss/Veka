//! Le défilement (étape 10), sans fenêtre : molette, pavé tactile, élan, bords élastiques, barres,
//! `scroll_to`, le focus qui fait défiler et les `ScrollView` imbriquées.

use std::cell::Cell;
use std::rc::Rc;

use veka_foundation::Instant;
use veka_ui::prelude::*;
use veka_ui::{InputEvent, PointerButton};

/// 50 lignes de 40 points (2000 points) dans une fenêtre de 300 × 200.
fn lignes(n: usize) -> Element {
    let rows = (0..n)
        .map(|i| Text(format!("Ligne {i}")).frame(280, 40).id(i as i64))
        .collect();
    veka_ui::views::stack(Axis::Vertical, Some(0.0), rows)
}

struct T {
    s: Surface,
    t: f64,
}

impl T {
    fn new(view: impl View + 'static) -> T {
        let mut s = Surface::new(view);
        s.resize(Size::new(300.0, 200.0), 1.0);
        let mut t = T { s, t: 1.0 };
        t.frame();
        t
    }

    fn frame(&mut self) {
        self.s.frame_at(Instant::from_nanos((self.t * 1e9) as u64));
    }

    /// Avance le temps de `secs`, image par image (60 par seconde).
    fn run(&mut self, secs: f64) {
        let end = self.t + secs;
        while self.t < end {
            self.t += 1.0 / 60.0;
            self.frame();
        }
    }

    fn scroll(&mut self, dy: f32, phase: ScrollPhase) {
        self.s.set_now(Instant::from_nanos((self.t * 1e9) as u64));
        self.s.handle(InputEvent::Scroll {
            position: Point::new(150.0, 100.0),
            delta: Vector::new(0.0, dy),
            phase,
        });
    }

    /// Le haut de la ligne `i`, à l'écran.
    fn top(&self, i: usize) -> f32 {
        let rows: Vec<Rect> = self
            .s
            .rects_of("frame")
            .into_iter()
            .filter(|r| r.width() == 280.0 && r.height() == 40.0)
            .collect();
        rows[i].y()
    }

    /// Le décalage : de combien la première ligne est montée.
    fn offset(&self) -> f32 {
        self.s.rects_of("ScrollView")[0].y() - self.top(0)
    }
}

fn liste() -> impl View {
    ScrollView![lignes(50)]
}

#[test]
fn la_molette_glisse_en_douceur() {
    let mut t = T::new(component(liste));
    assert_eq!(t.offset(), 0.0);
    t.scroll(60.0, ScrollPhase::Wheel);
    t.run(1.0 / 60.0);
    let first = t.offset();
    assert!(first > 0.0 && first < 60.0, "un cran glisse au lieu de sauter : {first}");
    t.run(0.3);
    assert!((t.offset() - 60.0).abs() < 0.5, "{}", t.offset());
    assert!(!t.s.is_animating() || t.s.needs_frame());
}

#[test]
fn des_crans_rapproches_s_additionnent() {
    let mut t = T::new(component(liste));
    for _ in 0..3 {
        t.scroll(60.0, ScrollPhase::Wheel);
        t.run(0.03);
    }
    t.run(0.5);
    assert!((t.offset() - 180.0).abs() < 0.5, "{}", t.offset());
}

#[test]
fn la_molette_s_arrete_au_bout_et_rebondit() {
    let mut t = T::new(component(liste));
    // Vers le haut, déjà au début : un petit rebond, puis retour à 0.
    t.scroll(-60.0, ScrollPhase::Wheel);
    t.run(0.05);
    assert!(t.offset() < 0.0, "le contenu dépasse un peu : {}", t.offset());
    t.run(1.0);
    assert_eq!(t.offset(), 0.0);
    // Tout en bas : jamais plus loin que le bout une fois arrêté.
    for _ in 0..60 {
        t.scroll(100.0, ScrollPhase::Wheel);
        t.run(0.02);
    }
    t.run(1.5);
    assert!((t.offset() - 1800.0).abs() < 0.5, "{}", t.offset());
}

#[test]
fn le_pave_suit_le_doigt_puis_l_elan_continue() {
    let mut t = T::new(component(liste));
    t.scroll(0.0, ScrollPhase::Began);
    for _ in 0..10 {
        t.t += 1.0 / 60.0;
        t.scroll(20.0, ScrollPhase::Changed);
        t.frame();
    }
    assert!((t.offset() - 200.0).abs() < 0.5, "le contenu suit le doigt : {}", t.offset());
    t.scroll(0.0, ScrollPhase::Ended);
    t.run(0.2);
    let mid = t.offset();
    assert!(mid > 250.0, "l'élan continue : {mid}");
    t.run(3.0);
    let end = t.offset();
    assert!(end > mid && end <= 1800.0, "{end}");
    // L'élan finit par s'arrêter.
    t.run(0.5);
    assert_eq!(t.offset(), end);
}

#[test]
fn les_doigts_posés_arretent_l_elan() {
    let mut t = T::new(component(liste));
    t.scroll(0.0, ScrollPhase::Began);
    for _ in 0..10 {
        t.t += 1.0 / 60.0;
        t.scroll(30.0, ScrollPhase::Changed);
        t.frame();
    }
    t.scroll(0.0, ScrollPhase::Ended);
    t.run(0.1);
    t.scroll(0.0, ScrollPhase::Began);
    let at = t.offset();
    t.run(0.3);
    assert_eq!(t.offset(), at);
}

#[test]
fn au_doigt_le_bord_est_elastique() {
    let mut t = T::new(component(liste));
    t.scroll(0.0, ScrollPhase::Began);
    for _ in 0..20 {
        t.t += 1.0 / 60.0;
        t.scroll(-30.0, ScrollPhase::Changed);
        t.frame();
    }
    let pulled = t.offset();
    assert!(pulled < 0.0 && pulled > -300.0, "le contenu dépasse de moins que le doigt : {pulled}");
    t.scroll(0.0, ScrollPhase::Ended);
    t.run(1.0);
    assert_eq!(t.offset(), 0.0, "le ressort le ramène");
}

#[test]
fn sans_elastique_le_bord_est_net() {
    let mut t = T::new(component(|| ScrollView![lignes(50)].scroll_bounce(ScrollBounce::Never)));
    t.scroll(0.0, ScrollPhase::Began);
    t.t += 1.0 / 60.0;
    t.scroll(-50.0, ScrollPhase::Changed);
    t.frame();
    assert_eq!(t.offset(), 0.0);
    t.scroll(-60.0, ScrollPhase::Wheel);
    t.run(0.3);
    assert_eq!(t.offset(), 0.0);
}

#[test]
fn un_pave_sans_phases_finit_seul() {
    // Windows : que des `Changed`, et le bord revient quand plus rien n'arrive.
    let mut t = T::new(component(liste));
    for _ in 0..10 {
        t.t += 1.0 / 60.0;
        t.scroll(-20.0, ScrollPhase::Changed);
        t.frame();
    }
    assert!(t.offset() < 0.0);
    t.run(1.0);
    assert_eq!(t.offset(), 0.0);
    assert!(!t.s.needs_frame() || t.s.is_animating());
}

#[test]
fn le_contenu_est_coupe_et_ne_prend_pas_les_clics_hors_de_la_vue() {
    let clics = Rc::new(Cell::new(0));
    let c = clics.clone();
    let vue = move || {
        let c = c.clone();
        VStack![spacing: 0;
            ScrollView![VStack![spacing: 0;
                Rectangle().frame(280, 150),
                Button("Bas", move || c.set(c.get() + 1)).frame(280, 100),
            ]]
            .frame(300, 100),
            Rectangle().frame(300, 100),
        ]
    };
    let mut t = T::new(component(vue));
    // Le bouton est sous la vue qui défile (y 150..250 dans le contenu, la vue fait 100 de haut).
    let b = t.s.rects_of("Button")[0];
    assert!(b.y() > 100.0);
    let p = Point::new(b.center().x, b.y() + 10.0);
    t.s.handle(InputEvent::PointerDown {
        position: p,
        button: PointerButton::Primary,
    });
    t.s.handle(InputEvent::PointerUp {
        position: p,
        button: PointerButton::Primary,
    });
    assert_eq!(clics.get(), 0, "caché, le bouton ne prend pas le clic");
    // Le dessin est coupé.
    assert!(t.s.draw_list().to_string().contains("clip Rectangle"));
}

#[test]
fn scroll_to_va_jusqu_a_la_vue() {
    let vue = || {
        ScrollViewReader(|proxy| {
            VStack![spacing: 0;
                Button("Aller", move || proxy.scroll_to(30, Some(UnitPoint::TOP))).frame(300, 40),
                ScrollView![lignes(50)].frame(300, 160),
            ]
        })
    };
    let mut t = T::new(component(vue));
    let b = t.s.rects_of("Button")[0].center();
    t.s.handle(InputEvent::PointerDown {
        position: b,
        button: PointerButton::Primary,
    });
    t.s.handle(InputEvent::PointerUp {
        position: b,
        button: PointerButton::Primary,
    });
    t.frame();
    // Sans animation : tout de suite, la ligne 30 en haut de la vue.
    let sv = t.s.rects_of("ScrollView")[0];
    assert!((t.top(30) - sv.y()).abs() < 0.5, "{} {}", t.top(30), sv.y());
}

#[test]
fn tab_fait_defiler_jusqu_au_champ() {
    let vue = || {
        let a = State::new(String::new());
        let b = State::new(String::new());
        ScrollView![VStack![spacing: 0;
            TextField("Haut", a),
            Rectangle().frame(280, 600),
            TextField("Bas", b),
        ]]
    };
    let mut t = T::new(component(vue));
    for _ in 0..2 {
        t.s.handle(InputEvent::Key {
            key: KeyEquivalent::Tab,
            modifiers: Modifiers::NONE,
            pressed: true,
            repeat: false,
        });
        t.run(0.6);
    }
    let field = t.s.rects_of("TextField")[1];
    assert!(field.y() >= 0.0 && field.max_y() <= 200.0, "le champ du bas est visible : {field:?}");
}

#[test]
fn page_suivante_et_fin() {
    let mut t = T::new(component(liste));
    t.s.handle(InputEvent::Key {
        key: KeyEquivalent::PageDown,
        modifiers: Modifiers::NONE,
        pressed: true,
        repeat: false,
    });
    t.run(0.5);
    assert!((t.offset() - 160.0).abs() < 0.5, "{}", t.offset());
    t.s.handle(InputEvent::Key {
        key: KeyEquivalent::End,
        modifiers: Modifiers::NONE,
        pressed: true,
        repeat: false,
    });
    t.run(0.8);
    assert!((t.offset() - 1800.0).abs() < 0.5, "{}", t.offset());
}

#[test]
fn imbriquees_chaque_vue_garde_son_axe() {
    let vue = || {
        let rangee = ScrollView![.horizontal; HStack![spacing: 0;
            Rectangle().frame(400, 80), Rectangle().frame(400, 80),
        ]]
        .frame(300, 80);
        ScrollView![VStack![spacing: 0; rangee, lignes(50)]]
    };
    let mut t = T::new(component(vue));
    // Un défilement vertical sur la rangée horizontale passe à la vue verticale.
    t.scroll(60.0, ScrollPhase::Wheel);
    t.run(0.5);
    let views = t.s.rects_of("ScrollView");
    assert!((views[0].y() - t.s.rects_of("ScrollView")[1].y() - 60.0).abs() < 0.5 || views[1].y() < 0.0);
    // Et un défilement horizontal reste à la rangée.
    t.s.set_now(Instant::from_nanos((t.t * 1e9) as u64));
    t.t += 1.0;
    t.s.handle(InputEvent::Scroll {
        position: Point::new(150.0, 10.0),
        delta: Vector::new(100.0, 0.0),
        phase: ScrollPhase::Wheel,
    });
    t.run(0.5);
    let r = t.s.rects_of("Rectangle")[0];
    assert!(r.x() < -50.0, "la rangée a défilé : {r:?}");
}

#[test]
fn le_decalage_est_signale() {
    let vu = Rc::new(Cell::new(0.0f32));
    let v = vu.clone();
    let vue = move || {
        let v = v.clone();
        ScrollView![lignes(50)].on_scroll_offset_change(move |o| v.set(o.y))
    };
    let mut t = T::new(component(vue));
    t.scroll(60.0, ScrollPhase::Wheel);
    t.run(0.5);
    assert!((vu.get() - 60.0).abs() < 0.5, "{}", vu.get());
}

#[test]
fn les_barres_apparaissent_en_defilant_puis_s_effacent() {
    let mut t = T::new(component(liste));
    let barres = |t: &T| {
        t.s.draw_list()
            .to_string()
            .lines()
            .filter(|l| l.starts_with("fill RoundedRectangle"))
            .count()
    };
    let avant = barres(&t);
    t.scroll(60.0, ScrollPhase::Wheel);
    t.run(0.3);
    assert_eq!(barres(&t), avant + 1, "{}", t.s.draw_list());
    t.run(2.0);
    assert_eq!(barres(&t), avant);
    assert!(!t.s.needs_frame(), "plus rien ne bouge : plus d'image");
}

#[test]
fn glisser_la_barre_fait_defiler() {
    let mut t = T::new(component(liste));
    // La barre verticale est au bord droit ; on la tient et on descend de 90 points.
    let x = 300.0 - 4.0;
    let p = Point::new(x, 10.0);
    t.s.handle(InputEvent::PointerMoved { position: p });
    t.s.handle(InputEvent::PointerDown {
        position: p,
        button: PointerButton::Primary,
    });
    t.s.handle(InputEvent::PointerMoved {
        position: Point::new(x, 100.0),
    });
    t.s.handle(InputEvent::PointerUp {
        position: Point::new(x, 100.0),
        button: PointerButton::Primary,
    });
    t.run(0.1);
    assert!(t.offset() > 500.0, "{}", t.offset());
}
