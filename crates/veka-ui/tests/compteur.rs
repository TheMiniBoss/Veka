//! L'exemple de l'étape 1, sans fenêtre : layout, état, clics, liste de dessin.

use veka_ui::prelude::*;
use veka_ui::schema::{Params, Schema, Value};
use veka_ui::{InputEvent, PointerButton};

fn compteur() -> impl View {
    let mut clics = State::new(0);

    VStack![spacing: 16;
        Text("Bonjour Veka").font(Font::Title),
        HStack![spacing: 12;
            RoundedRectangle(12).fill(Color::Blue).frame(80, 48),
            Capsule().fill(Color::Orange).frame(96, 48),
            Circle().fill(Color::Green).frame(48, 48),
        ],
        Button(format!("Clics : {clics}"), move || clics += 1),
    ]
    .padding(24)
}

/// Le même compteur, construit par le chemin « données » du schéma (celui de `.vk` et de l'API C).
fn compteur_donnees() -> impl View {
    let s = Schema::builtin();
    let mut clics = State::new(0);
    let b = |name: &str, params: Params, children: Vec<Element>| s.build(name, params, children).unwrap();
    let num = |v: f64| Value::Float(v);
    let color = Value::Color;

    let forme = |nom: &str, params: Params, c: Color, w: f64, h: f64| {
        let f = b(
            "fill",
            Params::new().with("color", color(c)),
            vec![b(nom, params, vec![])],
        );
        b(
            "frame",
            Params::new().with("width", num(w)).with("height", num(h)),
            vec![f],
        )
    };
    let titre = b(
        "font",
        Params::new().with("style", Value::Enum("title".into())),
        vec![b(
            "Text",
            Params::new().with("content", Value::String("Bonjour Veka".into())),
            vec![],
        )],
    );
    let formes = b(
        "HStack",
        Params::new().with("spacing", num(12.0)),
        vec![
            forme(
                "RoundedRectangle",
                Params::new().with("cornerRadius", num(12.0)),
                Color::Blue,
                80.0,
                48.0,
            ),
            forme("Capsule", Params::new(), Color::Orange, 96.0, 48.0),
            forme("Circle", Params::new(), Color::Green, 48.0, 48.0),
        ],
    );
    let bouton = b(
        "Button",
        Params::new()
            .with("title", Value::String(format!("Clics : {clics}").into()))
            .with("action", Value::Action(veka_ui::Action::new(move || clics += 1))),
        vec![],
    );
    let pile = b(
        "VStack",
        Params::new().with("spacing", num(16.0)),
        vec![titre, formes, bouton],
    );
    b("padding", Params::new().with("all", num(24.0)), vec![pile])
}

fn surface(view: Element) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(480.0, 320.0), 2.0);
    s.frame();
    s
}

fn click(s: &mut Surface, p: Point) {
    s.handle(InputEvent::PointerMoved { position: p });
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

fn button_rect(s: &Surface) -> Rect {
    s.rects_of("Button")[0]
}

#[test]
fn layout_du_compteur() {
    let s = surface(component(compteur));
    // Formes : 80 + 12 + 96 + 12 + 48 = 248 de large, centrées dans la fenêtre.
    let r = s.rects_of("RoundedRectangle")[0];
    let c = s.rects_of("Capsule")[0];
    let o = s.rects_of("Circle")[0];
    assert_eq!((r.width(), r.height()), (80.0, 48.0));
    assert_eq!(c.x() - r.max_x(), 12.0);
    assert_eq!(o.x() - c.max_x(), 12.0);
    assert!(
        ((r.x() + o.max_x()) / 2.0 - 240.0).abs() < 0.01,
        "pile centrée horizontalement"
    );
    // Pile verticale : titre, formes, bouton, espacés de 16.
    let titre = s.rects_of("Text")[0];
    let b = button_rect(&s);
    assert!((r.y() - titre.max_y() - 16.0).abs() < 0.01);
    assert!((b.y() - r.max_y() - 16.0).abs() < 0.01);
    // Le tout est centré verticalement.
    assert!(((titre.y() + b.max_y()) / 2.0 - 160.0).abs() < 0.01);
}

#[test]
fn trois_clics() {
    let mut s = surface(component(compteur));
    assert_eq!(s.texts(), vec!["Bonjour Veka", "Clics : 0"]);
    let created = s.nodes_created();
    let centre = button_rect(&s).center();
    for _ in 0..3 {
        click(&mut s, centre);
    }
    assert_eq!(s.texts(), vec!["Bonjour Veka", "Clics : 3"]);
    assert_eq!(
        s.nodes_created(),
        created,
        "les nœuds sont réutilisés, pas recréés"
    );
}

#[test]
fn clic_dans_le_coin_arrondi_ne_compte_pas() {
    let mut s = surface(component(compteur));
    let b = button_rect(&s);
    click(&mut s, Point::new(b.x() + 1.0, b.y() + 1.0));
    assert_eq!(s.texts()[1], "Clics : 0");
}

#[test]
fn appui_dedans_relache_dehors_ne_compte_pas() {
    let mut s = surface(component(compteur));
    let b = button_rect(&s);
    s.handle(InputEvent::PointerDown {
        position: b.center(),
        button: PointerButton::Primary,
    });
    s.handle(InputEvent::PointerMoved {
        position: Point::new(5.0, 5.0),
    });
    s.handle(InputEvent::PointerUp {
        position: Point::new(5.0, 5.0),
        button: PointerButton::Primary,
    });
    s.frame();
    assert_eq!(s.texts()[1], "Clics : 0");
}

#[test]
fn rien_a_dessiner_au_repos() {
    let mut s = surface(component(compteur));
    assert!(!s.needs_frame());
    // Bouger hors de tout ne demande rien.
    assert!(!s.handle(InputEvent::PointerMoved {
        position: Point::new(2.0, 2.0)
    }));
    // Survoler le bouton demande une image (il change de couleur), puis plus rien.
    assert!(s.handle(InputEvent::PointerMoved {
        position: button_rect(&s).center()
    }));
    s.frame();
    assert!(!s.needs_frame());
    assert!(!s.handle(InputEvent::PointerMoved {
        position: button_rect(&s).center()
    }));
}

#[test]
fn survol_et_appui_changent_la_couleur() {
    let mut s = surface(component(compteur));
    let fond = |s: &Surface| {
        s.draw_list()
            .commands
            .iter()
            .find_map(|c| match c {
                veka_graphics::DrawCommand::FillShape {
                    shape: Shape::Capsule,
                    paint,
                    rect,
                } if rect.height() < 48.0 => Some(*paint),
                _ => None,
            })
            .unwrap()
    };
    let repos = fond(&s);
    s.handle(InputEvent::PointerMoved {
        position: button_rect(&s).center(),
    });
    s.frame();
    let survol = fond(&s);
    s.handle(InputEvent::PointerDown {
        position: button_rect(&s).center(),
        button: PointerButton::Primary,
    });
    s.frame();
    let appui = fond(&s);
    assert_ne!(repos, survol);
    assert_ne!(survol, appui);
}

#[test]
fn liste_de_dessin() {
    let mut s = surface(component(compteur));
    insta::assert_snapshot!("compteur_0", s.draw_list().to_string());
    let centre = button_rect(&s).center();
    click(&mut s, centre);
    insta::assert_snapshot!("compteur_1", s.draw_list().to_string());
}

#[test]
fn rust_et_donnees_donnent_la_meme_liste() {
    let mut a = surface(component(compteur));
    let mut b = surface(component(compteur_donnees));
    assert_eq!(a.draw_list().to_string(), b.draw_list().to_string());
    let (ca, cb) = (button_rect(&a).center(), button_rect(&b).center());
    click(&mut a, ca);
    click(&mut b, cb);
    assert_eq!(b.texts()[1], "Clics : 1");
    assert_eq!(a.draw_list().to_string(), b.draw_list().to_string());
}

#[test]
fn tout_est_dans_le_schema() {
    let schema = Schema::builtin();
    // Le contenu de la vue, sans la vue composée elle-même.
    let clics_free = VStack![spacing: 16;
        Text("Bonjour Veka").font(Font::Title).font_weight(Weight::Bold),
        HStack![
            RoundedRectangle(12).fill(Color::Blue).frame(80, 48),
            Capsule().stroke(Color::Orange, 2).frame(96, 48),
            Circle().fill(Color::Green).frame(48, 48),
            Ellipse(), Rectangle(), Spacer(),
        ],
        ZStack![Button("Ok", || {}).tint(Color::Red)],
        Text("x").foreground(Color::Gray).background(Color::White).frame_width(10).on_tap(|| {}),
    ]
    .padding(24);
    for name in clics_free.primitive_names() {
        assert!(schema.get(name).is_some(), "`{name}` n'est pas dans le schéma");
    }
}

#[test]
fn erreurs_du_schema() {
    use veka_ui::schema::SchemaError;
    let s = Schema::builtin();
    assert!(matches!(
        s.build("Nope", Params::new(), vec![]),
        Err(SchemaError::Unknown(_))
    ));
    assert!(matches!(
        s.build("Text", Params::new(), vec![]),
        Err(SchemaError::MissingParam { .. })
    ));
    assert!(matches!(
        s.build("Text", Params::new().with("content", Value::Int(3)), vec![]),
        Err(SchemaError::WrongType { .. })
    ));
    assert!(matches!(
        s.build(
            "font",
            Params::new().with("style", Value::Enum("énorme".into())),
            vec![veka_ui::views::Text("x")]
        ),
        Err(SchemaError::WrongType { .. })
    ));
    assert!(matches!(
        s.build("padding", Params::new().with("all", Value::Int(1)), vec![]),
        Err(SchemaError::Children { .. })
    ));
}
