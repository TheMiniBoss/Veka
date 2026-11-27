//! Le langage : erreurs avec positions, rechargement qui garde les états, actions.

use veka_lang::{Program, parse, parse_expr, print, print_expr};
use veka_ui::prelude::*;
use veka_ui::{InputEvent, PointerButton};

fn surface(view: Element) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(400.0, 300.0), 1.0);
    s.frame();
    s
}

fn click_first_button(s: &mut Surface) {
    let p = s.rects_of("Button")[0].center();
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

fn messages(p: &Program) -> Vec<String> {
    p.errors().iter().map(ToString::to_string).collect()
}

#[test]
fn expressions_aller_retour() {
    for src in [
        "1 + 2 * 3",
        "(1 + 2) * 3",
        "a && b || !c",
        "x ? 1 : -2.5",
        "\"Clics : \\(n + 1)\"",
        "0..<10",
        "#FF6A0080",
        ".easeInOut(0.25).delay(1.0)",
        "LinearGradient([.Blue, .Purple], from: .topLeading)",
    ] {
        let e = parse_expr(src).unwrap_or_else(|d| panic!("{src} : {d}"));
        let text = print_expr(&e);
        assert_eq!(parse_expr(&text).unwrap(), e, "{src} → {text}");
    }
}

#[test]
fn erreur_de_syntaxe_avec_position() {
    let (_, errors) = parse("view A {\n    Text(\"x\"\n}\n");
    assert!(!errors.is_empty());
    assert_eq!(errors[0].span.line, 3, "{errors:?}");
}

#[test]
fn vue_et_modifier_inconnus_avec_suggestion() {
    let p = Program::new("view A {\n    Txt(\"x\")\n        .paddin(4)\n}\n");
    let m = messages(&p);
    assert!(
        m.iter().any(|x| x.starts_with("2:5") && x.contains("`Text`")),
        "{m:?}"
    );
    let p = Program::new("view A {\n    Text(\"x\")\n        .paddin(4)\n}\n");
    let m = messages(&p);
    assert!(m.iter().any(|x| x.contains("`padding`")), "{m:?}");
    let p = Program::new("view A {\n    VStack(spcing: 4) {\n    }\n}\n");
    let m = messages(&p);
    assert!(m.iter().any(|x| x.contains("`spacing`")), "{m:?}");
}

#[test]
fn erreur_a_l_execution_dans_la_vue() {
    let p = Program::new("view A {\n    Text(inconnu)\n}\n");
    let s = surface(p.view("A"));
    let t = s.texts();
    assert!(t[0].starts_with("2:10") && t[0].contains("inconnu"), "{t:?}");
}

#[test]
fn type_d_etat_verifie() {
    let p = Program::new("view A {\n    @State var n: Int = \"x\"\n    Text(\"\\(n)\")\n}\n");
    let s = surface(p.view("A"));
    assert!(s.texts()[0].contains("`Int` attendu"), "{:?}", s.texts());
}

#[test]
fn rechargement_garde_les_etats() {
    let v1 = "view A {\n    @State var n = 0\n    Button(\"n = \\(n)\") {\n        n += 1\n    }\n}\n";
    let p = Program::new(v1);
    let mut s = surface(p.view("A"));
    click_first_button(&mut s);
    click_first_button(&mut s);
    assert_eq!(s.texts(), ["n = 2"]);
    // Nouveau texte, nouvel état : `n` garde sa valeur, `m` prend la sienne.
    let v2 = "view A {\n    @State var n = 0\n    @State var m = 10\n    Button(\"n + m = \\(n + m)\") {\n        n += 1\n    }\n}\n";
    p.reload(v2).unwrap();
    s.frame();
    assert_eq!(s.texts(), ["n + m = 12"]);
    // Un fichier faux : l'ancien reste.
    let e = p.reload("view A {\n    Text(\n}\n").unwrap_err();
    assert!(!e.is_empty());
    s.frame();
    assert_eq!(s.texts(), ["n + m = 12"]);
}

#[test]
fn if_foreach_et_actions_sur_tableau() {
    let src = r#"
view A {
    @State var items = [1, 2]
    @State var vide = false

    VStack {
        Button("Ajouter") {
            items.append(items.count + 1)
            vide = items.isEmpty
        }
        if items.count > 2 {
            Text("beaucoup")
        } else {
            Text("peu")
        }
        ForEach(items) { x, i in
            Text("\(i) : \(x * 10)")
        }
        ForEach(0..<2) { k in
            Text("k\(k)")
        }
    }
}
"#;
    let p = Program::new(src);
    assert!(p.errors().is_empty(), "{:?}", messages(&p));
    let mut s = surface(p.view("A"));
    assert_eq!(s.texts(), ["Ajouter", "peu", "0 : 10", "1 : 20", "k0", "k1"]);
    click_first_button(&mut s);
    assert_eq!(
        s.texts(),
        ["Ajouter", "beaucoup", "0 : 10", "1 : 20", "2 : 30", "k0", "k1"]
    );
}

#[test]
fn erreur_dans_une_action() {
    let p = Program::new("view A {\n    Button(\"x\") {\n        n = 1\n    }\n}\n");
    let mut s = surface(p.view("A"));
    click_first_button(&mut s);
    let e = Program::take_runtime_error().expect("une erreur");
    assert_eq!(e.span.line, 3, "{e}");
}

#[test]
fn deux_vues_ont_chacune_leurs_etats() {
    let src = r#"
view A {
    HStack {
        B
        C
    }
}
view B {
    @State var n = 1
    Text("b\(n)")
}
view C {
    @State var n = "c"
    Text(n)
}
"#;
    let p = Program::new(src);
    let s = surface(p.view("A"));
    assert_eq!(s.texts(), ["b1", "c"]);
}

#[test]
fn impression_canonique() {
    let (m, e) = parse("view A{Text(\"x\").padding(4).font(.title)}");
    assert!(e.is_empty(), "{e:?}");
    assert_eq!(
        print(&m),
        "view A {\n    Text(\"x\")\n        .padding(4)\n        .font(.title)\n}\n"
    );
}
