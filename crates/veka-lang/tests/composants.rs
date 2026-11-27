//! PR K : composants, données de l'hôte, fonctions, maths, horloge, imports.

use std::cell::RefCell;
use std::rc::Rc;

use veka_lang::{Program, Val, parse, print};
use veka_ui::prelude::*;
use veka_ui::schema::Schema;
use veka_ui::{InputEvent, PointerButton};

fn surface(view: Element) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(600.0, 400.0), 1.0);
    s.frame();
    s
}

fn click_button(s: &mut Surface, i: usize) {
    let p = s.rects_of("Button")[i].center();
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

fn ok(src: &str) -> Program {
    let p = Program::new(src);
    let e: Vec<String> = p.errors().iter().map(ToString::to_string).collect();
    assert!(e.is_empty(), "{e:?}");
    p
}

const BOUTIQUE: &str = r#"
view Article {
    @Param var titre: String = "Article"
    @Param var prix: Int = 10
    @Slot badge
    @Event acheter(Int)

    HStack(spacing: 10) {
        Text(titre)
        Slot(badge)
        Button("Acheter \(prix) or") {
            emit acheter(prix)
        }
    }
}

view Boutique {
    @State var dernier: String = ""
    @State var total = 0

    VStack(spacing: 8) {
        ForEach(model.articles) { a in
            Article(titre: a.nom, prix: a.prix) {
                if a.nouveau {
                    Text("NOUVEAU")
                }
            }
            .on(acheter) { prix in
                dernier = a.nom
                total += prix
            }
            .id(a.id)
        }
        Article()
        Text("Dernier achat : \(dernier), total \(total)")
    }
}
"#;

fn articles() -> Val {
    let a = |id: i64, nom: &str, prix: i64, nouveau: bool| {
        Val::record([
            ("id", Val::from(id)),
            ("nom", Val::from(nom)),
            ("prix", Val::from(prix)),
            ("nouveau", Val::from(nouveau)),
        ])
    };
    Val::record([(
        "articles",
        Val::from(vec![a(1, "Épée", 12, false), a(2, "Bouclier", 8, true)]),
    )])
}

#[test]
fn parametres_slots_et_evenements() {
    let p = ok(BOUTIQUE);
    p.set_model(articles());
    let recu = Rc::new(RefCell::new(Vec::new()));
    let r = recu.clone();
    p.on_event(move |vue, ev, v| r.borrow_mut().push(format!("{vue}.{ev}({v})")));
    let mut s = surface(p.view("Boutique"));
    assert_eq!(
        s.texts(),
        [
            "Épée",
            "Acheter 12 or",
            "Bouclier",
            "NOUVEAU",
            "Acheter 8 or",
            "Article",
            "Acheter 10 or",
            "Dernier achat : , total 0"
        ]
    );
    click_button(&mut s, 1);
    click_button(&mut s, 0);
    assert_eq!(s.texts().last().unwrap(), "Dernier achat : Épée, total 20");
    // L'article sans `.on` : l'événement va à l'hôte.
    click_button(&mut s, 2);
    assert_eq!(*recu.borrow(), ["Article.acheter(10)"]);
    // L'hôte change ses données : la vue suit, les états restent.
    p.set_model(Val::record([("articles", Val::from(Vec::<Val>::new()))]));
    s.frame();
    assert_eq!(
        s.texts(),
        ["Article", "Acheter 10 or", "Dernier achat : Épée, total 20"]
    );
}

#[test]
fn erreurs_de_composant() {
    let p =
        Program::new("view A {\n    @Param var n: Int\n    Text(\"\\(n)\")\n}\nview B {\n    A(m: 1)\n}\n");
    let s = surface(p.view("B"));
    assert!(
        s.texts()[0].contains("`A` n'a pas de paramètre `m`"),
        "{:?}",
        s.texts()
    );
    let p = Program::new(
        "view A {\n    @Param var n: Int\n    Text(\"\\(n)\")\n}\nview B {\n    A(n: \"x\")\n}\n",
    );
    let s = surface(p.view("B"));
    assert!(s.texts()[0].contains("`Int` attendu"), "{:?}", s.texts());
    let p = Program::new(
        "view A {\n    @Event ok\n    Text(\"a\")\n}\nview B {\n    A()\n        .on(ko) {\n        }\n}\n",
    );
    let s = surface(p.view("B"));
    assert!(s.texts()[0].contains("vouliez-vous dire `ok`"), "{:?}", s.texts());
}

#[test]
fn fonctions_maths_enregistrements() {
    let src = r#"
func carre(x: Float) -> Float {
    x * x
}

func hypot(a: Float, b: Float) -> Float {
    let s = carre(a) + carre(b)
    sqrt(s)
}

view A {
    @State var point = { x: 3, y: 4 }
    @State var liste = [10, 20, 30]

    VStack {
        Text(format("%.1f", hypot(point.x, point.y)))
        Text("\(liste[1]) \(len(liste)) \(clamp(15, 0, 10)) \(round(pi * 100))")
        Text(format("%d%% %s", 42, "ok"))
        Text("\(random(7) == random(7)) \(random(7) != random(8)) \(random(1, 5, 6) >= 5)")
        Text("\(lerp(0, 10, 0.25)) \(floor(-1.5)) \(atan2(1, 1) * 4 == pi)")
    }
}
"#;
    let p = ok(src);
    let s = surface(p.view("A"));
    assert_eq!(
        s.texts(),
        ["5.0", "20 3 10 314", "42% ok", "true true true", "2.5 -2 true"]
    );
}

#[test]
fn recursion_sans_fin_arretee() {
    let p = Program::new("func f(x: Int) -> Int {\n    f(x + 1)\n}\nview A {\n    Text(\"\\(f(0))\")\n}\n");
    let s = surface(p.view("A"));
    assert!(s.texts()[0].contains("récursion"), "{:?}", s.texts());
}

#[test]
fn horloge() {
    let p = ok("view A {\n    @Clock var t\n    Text(format(\"%.1f\", t))\n}\n");
    let mut s = Surface::new(p.view("A"));
    s.resize(Size::new(200.0, 100.0), 1.0);
    let at = |s: &mut Surface, t: f64| {
        s.frame_at(veka_foundation::Instant::from_nanos((t * 1e9) as u64));
    };
    at(&mut s, 10.0);
    at(&mut s, 10.0);
    assert_eq!(s.texts(), ["0.0"]);
    at(&mut s, 11.5);
    at(&mut s, 11.5);
    assert_eq!(s.texts(), ["1.5"]);
}

#[test]
fn timeline_controlee_par_une_action() {
    let src = r#"
timeline Va(length: 1) {
    track x { 0: 0 ; 1: 100 .linear }
}

view A {
    @Timeline var v = Va()

    VStack {
        Button("go") {
            v.seek(0.5)
        }
        Text("\(round(v.x)) \(v.isPlaying)")
    }
}
"#;
    let p = ok(src);
    let mut s = surface(p.view("A"));
    assert_eq!(s.texts()[1], "0 false");
    click_button(&mut s, 0);
    assert_eq!(s.texts()[1], "50 false");
}

#[test]
fn import_et_rechargement() {
    let fichiers = Rc::new(RefCell::new(std::collections::HashMap::from([(
        "boutons.vk".to_string(),
        "view Gros {\n    @Param var t: String = \"?\"\n    Text(t)\n        .font(.largeTitle)\n}\n"
            .to_string(),
    )])));
    let f = fichiers.clone();
    let p = Program::with_loader(
        "import \"boutons.vk\"\nview A {\n    Gros(t: \"salut\")\n}\n",
        Schema::builtin(),
        move |path| f.borrow().get(path).cloned().ok_or_else(|| "absent".to_string()),
    );
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    assert_eq!(p.imports(), ["boutons.vk"]);
    let mut s = surface(p.view("A"));
    assert_eq!(s.texts(), ["salut"]);
    fichiers.borrow_mut().insert(
        "boutons.vk".into(),
        "view Gros {\n    @Param var t: String = \"?\"\n    Text(\"[\\(t)]\")\n}\n".into(),
    );
    p.reload("import \"boutons.vk\"\nview A {\n    Gros(t: \"salut\")\n}\n")
        .unwrap();
    s.frame();
    assert_eq!(s.texts(), ["[salut]"]);
    let p = Program::new("import \"x.vk\"\nview A {\n    Text(\"a\")\n}\n");
    assert!(p.errors()[0].message.contains("ne lit pas de fichiers"));
}

#[test]
fn aller_retour_des_nouvelles_formes() {
    let src = r#"
import "a.vk"

func f(x: Int, y) -> Int {
    let z = x + 1
    z * 2
}

timeline T(loop: true, length: 0.9) {
    track échelle { 0: 1 .easeOut ; 0.15: 1.35 ; 0.5: 1 }
    event 0.15 battement
}

view V {
    @Param var titre: String = "x"
    @Param var n: Int
    @Slot badge
    @Event acheter(Int)
    @Event fini
    @State var r = { a: 1, b: [1, 2][0] }
    @Timeline var p = T(autoplay: true)
    @Clock var t

    Button("x") {
        emit acheter(n)
        emit fini
    }
}
"#;
    let (m, e) = parse(src);
    assert!(e.is_empty(), "{e:?}");
    let text = print(&m);
    let (m2, e) = parse(&text);
    assert!(e.is_empty(), "{e:?}\n{text}");
    assert_eq!(m, m2, "{text}");
    assert_eq!(print(&m2), text);
}
