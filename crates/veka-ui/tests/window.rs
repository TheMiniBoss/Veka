//! Fenêtres dessinées par Veka : régions, actions, état de la fenêtre.

use std::cell::Cell;
use std::rc::Rc;

use veka_ui::prelude::*;
use veka_ui::window::TitleBarStyle;
use veka_ui::{InputEvent, PointerButton, RegionKind, WindowRegion};

fn clic(s: &mut Surface, p: Point) {
    s.handle(InputEvent::PointerDown {
        position: p,
        button: PointerButton::Primary,
    });
    s.handle(InputEvent::PointerUp {
        position: p,
        button: PointerButton::Primary,
    });
}

fn fenetre() -> impl View {
    VStack![spacing: 0;
        TitleBarWith("Veka", TitleBarStyle::Windows),
        Text("Contenu").padding(24),
        Spacer(),
    ]
}

fn surface(view: fn() -> Element) -> Surface {
    let mut s = Surface::new(component(view));
    s.resize(Size::new(480.0, 320.0), 1.0);
    s.frame();
    s
}

#[test]
fn barre_windows_regions() {
    let mut s = surface(|| fenetre().into_element());
    let regions = s.take_regions_if_changed().expect("premières régions");
    let kinds: Vec<RegionKind> = regions.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![
            RegionKind::Drag,
            RegionKind::Control(WindowControl::Minimize),
            RegionKind::Control(WindowControl::Maximize),
            RegionKind::Control(WindowControl::Close),
        ]
    );
    // La barre fait toute la largeur, 32 points de haut, en haut de la fenêtre.
    assert_eq!(regions[0].rect, Rect::new(0.0, 0.0, 480.0, 32.0));
    // Les trois boutons de 46 × 32 sont collés à droite.
    assert_eq!(regions[3].rect, Rect::new(480.0 - 46.0, 0.0, 46.0, 32.0));
    assert_eq!(regions[2].rect, Rect::new(480.0 - 92.0, 0.0, 46.0, 32.0));
    // Pas de changement : rien à renvoyer.
    s.frame();
    assert!(s.take_regions_if_changed().is_none());
    // Nouvelle taille : les régions suivent.
    s.resize(Size::new(600.0, 320.0), 1.0);
    s.frame();
    let regions = s.take_regions_if_changed().expect("nouvelles régions");
    assert_eq!(regions[3].rect.origin.x, 600.0 - 46.0);
}

#[test]
fn boutons_de_fenetre_et_actions() {
    let mut s = surface(|| fenetre().into_element());
    let regions: Vec<WindowRegion> = s.regions().to_vec();
    let centre = |c: WindowControl| {
        regions
            .iter()
            .find(|r| r.kind == RegionKind::Control(c))
            .unwrap()
            .rect
            .center()
    };
    clic(&mut s, centre(WindowControl::Maximize));
    clic(&mut s, centre(WindowControl::Close));
    assert_eq!(
        s.take_window_actions(),
        vec![WindowAction::ToggleMaximize, WindowAction::Close]
    );
    assert!(s.take_window_actions().is_empty());

    // Un clic sur la barre elle-même ne demande rien (c'est le système qui déplace la fenêtre).
    clic(&mut s, Point::new(100.0, 16.0));
    assert!(s.take_window_actions().is_empty());
}

#[test]
fn etat_de_la_fenetre() {
    let mut s = surface(|| fenetre().into_element());
    let normale = s.draw_list().to_string();
    s.set_window_info(WindowInfo {
        maximized: true,
        ..WindowInfo::default()
    });
    assert!(s.needs_frame());
    let agrandie = s.frame().to_string();
    assert_ne!(normale, agrandie, "l'icône agrandir devient « restaurer »");

    s.set_window_info(WindowInfo {
        maximized: true,
        active: false,
        fullscreen: false,
    });
    let inactive = s.frame().to_string();
    assert_ne!(agrandie, inactive, "le titre est atténué");

    // Même état : rien à refaire.
    s.set_window_info(WindowInfo {
        maximized: true,
        active: false,
        fullscreen: false,
    });
    assert!(!s.needs_frame());
}

#[test]
fn zone_de_deplacement_avec_bouton() {
    thread_local!(static CLICS: Rc<Cell<u32>> = Rc::new(Cell::new(0)));
    fn vue() -> Element {
        HStack![
            Text("Titre"),
            Spacer(),
            Button("Menu", || CLICS.with(|c| c.set(c.get() + 1))),
        ]
        .frame_height(40)
        .window_drag_area()
        .into_element()
    }
    let mut s = surface(vue);
    let kinds: Vec<RegionKind> = s.regions().iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec![RegionKind::Drag, RegionKind::Client]);
    let bouton = s.regions()[1].rect.center();
    clic(&mut s, bouton);
    assert_eq!(
        CLICS.with(|c| c.get()),
        1,
        "le bouton dans la barre reste cliquable"
    );
}

#[test]
fn window_control_sur_une_vue() {
    thread_local!(static CLICS: Rc<Cell<u32>> = Rc::new(Cell::new(0)));
    fn vue() -> Element {
        Button("Fermer", || CLICS.with(|c| c.set(c.get() + 1)))
            .window_control(WindowControl::Close)
            .into_element()
    }
    let mut s = surface(vue);
    assert_eq!(s.regions()[0].kind, RegionKind::Control(WindowControl::Close));
    let centre = s.regions()[0].rect.center();
    clic(&mut s, centre);
    assert_eq!(s.take_window_actions(), vec![WindowAction::Close]);
    assert_eq!(
        CLICS.with(|c| c.get()),
        0,
        "la vue entière est le bouton de fenêtre"
    );
}

#[test]
fn barre_mac() {
    fn vue() -> Element {
        TitleBarWith("Veka", TitleBarStyle::Mac).into_element()
    }
    let s = surface(vue);
    let kinds: Vec<RegionKind> = s.regions().iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![
            RegionKind::Drag,
            RegionKind::Control(WindowControl::Close),
            RegionKind::Control(WindowControl::Minimize),
            RegionKind::Control(WindowControl::Fullscreen),
        ]
    );
    // Feux à gauche, 12 points.
    assert_eq!(s.regions()[1].rect.size, Size::new(12.0, 12.0));
    assert!(s.regions()[1].rect.origin.x < 20.0);
}

#[test]
fn schema_construit_la_barre() {
    use veka_ui::schema::{Params, Schema, Value};
    let schema = Schema::builtin();
    let p = Params::new()
        .with("title", Value::String("Veka".into()))
        .with("style", Value::Enum("windows".into()));
    let barre = schema.build("TitleBar", p, vec![]).unwrap();
    let mut s = Surface::new(barre);
    s.resize(Size::new(300.0, 100.0), 1.0);
    s.frame();
    assert_eq!(s.regions().len(), 4);

    let p = Params::new().with("control", Value::Enum("minimize".into()));
    let e = schema.build(
        "windowControl",
        p,
        vec![schema.build("Rectangle", Params::new(), vec![]).unwrap()],
    );
    assert!(e.is_ok());
}

#[test]
fn barre_maison_deplace_par_le_vide() {
    // Comme `fenetre_transparente` : l'espace libre entre le titre et ✕ déplace la fenêtre.
    fn vue() -> Element {
        VStack![spacing: 16;
            HStack![
                Text("Veka — fenêtre transparente").font(Font::Headline),
                Spacer(),
                WindowButtonWith(WindowControl::Close, TitleBarStyle::Windows),
            ]
            .window_drag_area(),
            Text("Tout autour, c'est le bureau."),
        ]
        .padding(24)
        .padding(16)
    }
    let mut s = Surface::new(component(vue));
    s.resize(Size::new(440.0, 240.0), 1.0);
    s.frame();
    let regions = s.take_regions_if_changed().expect("régions");
    let barre = regions
        .iter()
        .find(|r| r.kind == RegionKind::Drag)
        .expect("zone")
        .rect;
    // Entre le titre et le bouton, au milieu de la barre.
    let p = Point::new(300.0, barre.y() + barre.height() / 2.0);
    let kind = regions.iter().rev().find(|r| r.rect.contains(p)).map(|r| r.kind);
    assert_eq!(kind, Some(RegionKind::Drag));
}
