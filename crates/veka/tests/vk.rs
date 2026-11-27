//! Le langage `.vk` : un fichier donne le même dessin que la même vue écrite en Rust.

use veka::lang::{Program, parse, print};
use veka::prelude::*;
use veka::ui::{InputEvent, PointerButton};

const COMPTEUR: &str = include_str!("../examples/vk/compteur.vk");

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

fn compteur_rust() -> impl View {
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

fn program(source: &str) -> Program {
    let p = Program::new(source);
    let errors = p.errors();
    assert!(
        errors.is_empty(),
        "{}",
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    p
}

#[test]
fn aller_retour_texte() {
    let (m, errors) = parse(COMPTEUR);
    assert!(errors.is_empty(), "{errors:?}");
    let text = print(&m);
    let (m2, errors) = parse(&text);
    assert!(errors.is_empty(), "{errors:?}\n{text}");
    assert_eq!(m, m2);
    assert_eq!(print(&m2), text, "l'impression est stable");
}

#[test]
fn compteur_meme_dessin_que_rust() {
    let p = program(COMPTEUR);
    let mut vk = surface(p.view("Compteur"));
    let mut rs = surface(component(compteur_rust));
    assert_eq!(vk.draw_list().to_string(), rs.draw_list().to_string());
    let bouton = rs.rects_of("Button")[0].center();
    assert_eq!(vk.rects_of("Button")[0].center(), bouton);
    click(&mut vk, bouton);
    click(&mut rs, bouton);
    assert!(vk.texts().contains(&"Clics : 1".to_string()), "{:?}", vk.texts());
    assert_eq!(vk.draw_list().to_string(), rs.draw_list().to_string());
}

const COLORA: &str = include_str!("../examples/vk/colora.vk");

#[path = "../examples/colora.rs"]
#[allow(dead_code)]
mod colora_rs;

fn same(vk: &Surface, rs: &Surface) {
    let (a, b) = (vk.draw_list().to_string(), rs.draw_list().to_string());
    if a != b {
        let line = a.lines().zip(b.lines()).position(|(x, y)| x != y);
        panic!("dessins différents (ligne {line:?})\n.vk :\n{a}\nRust :\n{b}");
    }
}

#[test]
fn colora_meme_dessin_que_rust() {
    let p = program(COLORA);
    let mut vk = surface(p.view("Colora"));
    let mut rs = surface(component(colora_rs::vue));
    same(&vk, &rs);
    // Les deux boutons du bas : mode, puis gris.
    for i in [1, 1, 2, 2, 1] {
        let b = rs.rects_of("Button")[i].center();
        // Rust recrée le contenu quand `grayscale` apparaît (le survol est perdu) ; le .vk garde
        // le même arbre : on compare pointeur sorti.
        for s in [&mut vk, &mut rs] {
            click(s, b);
            s.handle(InputEvent::PointerMoved {
                position: Point::new(1.0, 1.0),
            });
            s.frame();
        }
        same(&vk, &rs);
    }
}

const ANIMA: &str = include_str!("../examples/vk/anima.vk");

#[path = "../examples/anima.rs"]
#[allow(dead_code)]
mod anima_rs;

fn at(s: &mut Surface, t: f64) {
    s.frame_at(veka::foundation::Instant::from_nanos((t * 1e9) as u64));
}

#[test]
fn anima_meme_dessin_que_rust() {
    let p = program(ANIMA);
    let mut vk = Surface::new(p.view("Anima"));
    let mut rs = Surface::new(component(anima_rs::vue));
    for s in [&mut vk, &mut rs] {
        s.resize(Size::new(860.0, 720.0), 1.0);
        at(s, 0.0);
    }
    same(&vk, &rs);
    // Chaque bouton, puis la carte ; on compare pendant et après l'animation.
    let mut t = 0.0;
    let boutons = rs.rects_of("Button").len();
    let mut cibles: Vec<Point> = (0..boutons).map(|i| rs.rects_of("Button")[i].center()).collect();
    cibles.push(rs.rects_of("RoundedRectangle")[0].center());
    cibles.push(cibles[cibles.len() - 1]);
    for c in cibles {
        for s in [&mut vk, &mut rs] {
            s.handle(InputEvent::PointerMoved { position: c });
            s.handle(InputEvent::PointerDown {
                position: c,
                button: PointerButton::Primary,
            });
            s.handle(InputEvent::PointerUp {
                position: c,
                button: PointerButton::Primary,
            });
            s.handle(InputEvent::PointerMoved {
                position: Point::new(1.0, 1.0),
            });
        }
        for dt in [0.0, 0.1, 0.3, 2.0] {
            for s in [&mut vk, &mut rs] {
                at(s, t + dt);
            }
            same(&vk, &rs);
        }
        t += 2.0;
    }
    let textes = vk.texts();
    for attendu in ["Froid", "Élément 4", "Lecture"] {
        assert!(textes.iter().any(|x| x == attendu), "{textes:?}");
    }
    let battements = textes
        .iter()
        .find(|x| x.starts_with("Battements"))
        .expect("le cœur");
    assert_ne!(battements, "Battements : 0");
}

#[test]
fn feux_d_artifice() {
    let source = include_str!("../examples/vk/feux.vk");
    let images = |instants: &[f64]| {
        let p = program(source);
        let mut s = Surface::new(p.view("Feux"));
        s.resize(Size::new(640.0, 200.0), 1.0);
        let mut listes = Vec::new();
        for &t in instants {
            at(&mut s, t);
            at(&mut s, t);
            listes.push((
                s.draw_list().to_string(),
                s.rects_of("Circle").len(),
                s.rects_of("Capsule").len(),
            ));
        }
        assert!(s.needs_frame(), "l'horloge avance à chaque image");
        listes
    };
    let a = images(&[0.0, 0.5, 2.0, 2.9]);
    // Au début, des fusées (traînées) ; ensuite, des gerbes de 180 étincelles.
    assert!(a.iter().any(|(_, _, fusees)| *fusees > 0));
    assert!(
        a.iter().any(|(_, cercles, _)| *cercles >= 180),
        "{:?}",
        a.iter().map(|x| x.1).collect::<Vec<_>>()
    );
    // Déterministe : mêmes instants, même dessin.
    assert_eq!(a, images(&[0.0, 0.5, 2.0, 2.9]));
    // Une image ne prend pas trop de temps (évaluation de ~1000 étincelles).
    let p = program(source);
    let mut s = Surface::new(p.view("Feux"));
    s.resize(Size::new(640.0, 200.0), 1.0);
    at(&mut s, 0.0);
    let debut = std::time::Instant::now();
    for i in 1..=10 {
        at(&mut s, 1.0 + i as f64 * 0.016);
    }
    eprintln!("feux : {:.1} ms par image", debut.elapsed().as_secs_f64() * 100.0);
}

#[test]
fn boutique_c_se_charge() {
    let p = program(include_str!("../../../examples/c/boutique.vk"));
    p.set_model(veka::lang::Val::record([(
        "articles",
        veka::lang::Val::from(vec![veka::lang::Val::record([
            ("nom", veka::lang::Val::from("Épée")),
            ("prix", veka::lang::Val::from(12)),
            ("nouveau", veka::lang::Val::from(true)),
        ])]),
    )]));
    let s = surface(p.view("Boutique"));
    assert_eq!(
        s.texts(),
        [
            "Boutique",
            "Épée",
            "NOUVEAU",
            "Acheter 12 or",
            "Dernier achat : rien, 0 or dépensés"
        ]
    );
}

#[test]
fn flou_d_arriere_plan_en_vk() {
    let p = program(
        r#"
view Verre {
    VStack(spacing: 12) {
        Text("Dépoli")
            .padding(20)
            .frostedGlass()
        Text("Barre")
            .padding(12)
            .backdropBlur(radius: 16, shape: .capsule, tint: .Blue, tintAmount: 0.3)
        Text("Coins")
            .backdropBlur(8, cornerRadius: 10)
    }
}
"#,
    );
    let vk = surface(p.view("Verre"));
    let rs = surface(VStack![spacing: 12;
        Text("Dépoli").padding(20).frosted_glass(),
        Text("Barre").padding(12).backdrop(
            BackdropBlur::default()
                .radius(16.0)
                .shape(Shape::Capsule)
                .tint(Token::Blue, 0.3)
        ),
        Text("Coins").backdrop(
            BackdropBlur::default()
                .radius(8.0)
                .shape(Shape::RoundedRectangle(CornerRadii::all(10.0)))
        ),
    ]);
    same(&vk, &rs);
    assert_eq!(rs.draw_list().to_string().matches("backdrop").count(), 3);
}
