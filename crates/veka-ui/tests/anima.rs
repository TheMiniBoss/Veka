//! Anima, image par image : layout animé, couleurs, opacité, transitions, interruptions.

use std::cell::Cell;

use veka_foundation::Instant;
use veka_graphics::DrawCommand;
use veka_ui::prelude::*;

fn at(s: &mut Surface, seconds: f64) {
    s.frame_at(Instant::from_nanos((seconds * 1e9) as u64));
}

fn width(s: &Surface) -> f32 {
    s.rects_of("Rectangle")[0].width()
}

thread_local! {
    static LARGEUR: Cell<Option<State<f32>>> = const { Cell::new(None) };
    static ROUGE: Cell<Option<State<bool>>> = const { Cell::new(None) };
    static LISTE: Cell<Option<State<u32>>> = const { Cell::new(None) };
}

fn largeur() -> State<f32> {
    LARGEUR.with(|c| c.get()).unwrap()
}

fn vue_largeur() -> impl View {
    let w = State::new(100.0f32);
    LARGEUR.with(|c| c.set(Some(w)));
    Rectangle().frame(w.get(), 40)
}

fn surface(view: Element) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(400.0, 300.0), 1.0);
    at(&mut s, 10.0);
    s
}

#[test]
fn layout_anime() {
    let mut s = surface(component(vue_largeur));
    assert_eq!(width(&s), 100.0);
    with_animation(Animation::linear(1.0), || largeur().set(200.0));
    at(&mut s, 10.0);
    assert_eq!(width(&s), 100.0, "départ");
    assert!(s.is_animating());
    at(&mut s, 10.5);
    assert!((width(&s) - 150.0).abs() < 0.01, "{}", width(&s));
    // Centré dans la fenêtre : la position s'anime aussi.
    assert!((s.rects_of("Rectangle")[0].x() - 125.0).abs() < 0.01);
    at(&mut s, 11.0);
    assert_eq!(width(&s), 200.0);
    assert!(!s.is_animating());
    assert!(!s.needs_frame(), "0 % CPU au repos");
}

#[test]
fn sans_transaction_rien_ne_bouge() {
    let mut s = surface(component(vue_largeur));
    largeur().set(300.0);
    at(&mut s, 10.0);
    assert_eq!(width(&s), 300.0);
    assert!(!s.is_animating());
}

#[test]
fn ressort_interrompu_sans_a_coup() {
    let mut s = surface(component(vue_largeur));
    with_animation(Animation::snappy(), || largeur().set(300.0));
    at(&mut s, 10.0);
    at(&mut s, 10.1);
    let w1 = width(&s);
    assert!(w1 > 100.0 && w1 < 300.0);
    // Nouvelle cible en plein vol : le ressort garde sa vitesse, la largeur continue de grandir un
    // instant au lieu de repartir d'un coup.
    with_animation(Animation::snappy(), || largeur().set(100.0));
    at(&mut s, 10.1);
    assert!((width(&s) - w1).abs() < 0.01, "pas de saut");
    at(&mut s, 10.11);
    assert!(width(&s) > w1, "{} ≤ {w1}", width(&s));
    at(&mut s, 14.0);
    assert_eq!(width(&s), 100.0);
    assert!(!s.is_animating());
}

fn fills(s: &Surface) -> Vec<Paint> {
    s.draw_list()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillShape { paint, .. } => Some(paint.clone()),
            _ => None,
        })
        .collect()
}

fn vue_couleur() -> impl View {
    let rouge = State::new(true);
    ROUGE.with(|c| c.set(Some(rouge)));
    Rectangle()
        .fill(if rouge.get() {
            Color::hex(0xFF0000)
        } else {
            Color::hex(0x0000FF)
        })
        .frame(10, 10)
        // Animé chaque fois que la couleur change, sans with_animation.
        .animation(Animation::linear(1.0), rouge.get())
}

#[test]
fn couleur_en_oklab_avec_animation_valeur() {
    let mut s = surface(component(vue_couleur));
    ROUGE.with(|c| c.get()).unwrap().set(false);
    at(&mut s, 10.0);
    let c = fills(&s)[1].solid().unwrap();
    assert!(
        (c.r - 1.0).abs() < 1e-4 && c.g.abs() < 1e-4 && c.b.abs() < 1e-4,
        "départ : {c:?}"
    );
    at(&mut s, 10.5);
    let c = fills(&s)[1].solid().unwrap();
    // Le milieu en Oklab (≈ #8C53A2) est un violet bien plus clair que le milieu sRGB (#800080).
    assert!(
        (c.r - 0.55).abs() < 0.02 && (c.g - 0.33).abs() < 0.02 && (c.b - 0.64).abs() < 0.02,
        "{c:?}"
    );
    at(&mut s, 11.0);
    assert_eq!(fills(&s)[1], Paint::Solid(Color::hex(0x0000FF)));
}

#[test]
fn animation_coupee() {
    fn vue() -> impl View {
        let rouge = State::new(true);
        ROUGE.with(|c| c.set(Some(rouge)));
        Rectangle()
            .fill(if rouge.get() {
                Color::hex(0xFF0000)
            } else {
                Color::hex(0x0000FF)
            })
            .animation(None, rouge.get())
    }
    let mut s = surface(component(vue));
    with_animation(Animation::linear(1.0), || {
        ROUGE.with(|c| c.get()).unwrap().set(false)
    });
    at(&mut s, 10.0);
    assert_eq!(fills(&s)[1], Paint::Solid(Color::hex(0x0000FF)));
    assert!(!s.is_animating());
}

fn opacites(s: &Surface) -> Vec<f32> {
    s.draw_list()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::PushLayer { opacity } => Some(*opacity),
            _ => None,
        })
        .collect()
}

#[test]
fn opacite_animee() {
    fn vue() -> impl View {
        let visible = State::new(false);
        ROUGE.with(|c| c.set(Some(visible)));
        Text("Bonjour").opacity(if visible.get() { 1.0 } else { 0.0 })
    }
    let mut s = surface(component(vue));
    with_animation(Animation::linear(1.0), || {
        ROUGE.with(|c| c.get()).unwrap().set(true)
    });
    at(&mut s, 10.0);
    at(&mut s, 10.25);
    let o = opacites(&s);
    assert_eq!(o.len(), 1);
    assert!((o[0] - 0.25).abs() < 0.01, "{o:?}");
    at(&mut s, 11.0);
    assert!(opacites(&s).is_empty());
}

fn vue_liste() -> impl View {
    let n = State::new(2u32);
    LISTE.with(|c| c.set(Some(n)));
    let lignes = (1..=n.get())
        .map(|i| {
            Text(format!("Élément {i}"))
                .id(i as i64)
                .transition(Transition::push(Edge::Bottom))
        })
        .collect();
    veka_ui::views::stack(veka_ui::Axis::Vertical, Some(8.0), lignes)
}

fn glyph_runs(s: &Surface) -> usize {
    s.draw_list()
        .commands
        .iter()
        .filter(|c| matches!(c, DrawCommand::Glyphs(_)))
        .count()
}

#[test]
fn transitions_d_entree_et_de_sortie() {
    let mut s = surface(component(vue_liste));
    let n = || LISTE.with(|c| c.get()).unwrap();
    assert_eq!(s.texts(), ["Élément 1", "Élément 2"]);

    // Entrée : la nouvelle ligne arrive en fondu, d'en bas.
    with_animation(Animation::linear(1.0), || n().set(3));
    at(&mut s, 10.0);
    assert_eq!(s.texts().len(), 3);
    at(&mut s, 10.5);
    let o = opacites(&s);
    assert_eq!(o.len(), 1);
    assert!((o[0] - 0.5).abs() < 0.01, "{o:?}");
    assert!(
        s.draw_list()
            .commands
            .iter()
            .any(|c| matches!(c, DrawCommand::PushTransform(t) if t.ty > 1.0))
    );
    at(&mut s, 11.0);
    assert!(opacites(&s).is_empty());

    // Sortie : la ligne n'est plus dans l'arbre logique mais reste dessinée le temps de sortir.
    with_animation(Animation::linear(1.0), || n().set(1));
    at(&mut s, 12.0);
    at(&mut s, 12.5);
    assert_eq!(s.texts(), ["Élément 1"]);
    assert_eq!(
        glyph_runs(&s),
        3,
        "les deux lignes qui sortent sont encore dessinées"
    );
    at(&mut s, 13.0);
    assert_eq!(glyph_runs(&s), 1);
    assert!(!s.is_animating());

    // Sans animation : la sortie est immédiate.
    n().set(0);
    at(&mut s, 14.0);
    assert_eq!(glyph_runs(&s), 0);
}

#[test]
fn delai_et_repetition() {
    let mut s = surface(component(vue_largeur));
    with_animation(Animation::linear(1.0).delay(0.5).repeat(2, true), || {
        largeur().set(200.0)
    });
    at(&mut s, 10.0);
    at(&mut s, 10.4);
    assert_eq!(width(&s), 100.0, "pendant le délai");
    at(&mut s, 11.5);
    assert!((width(&s) - 200.0).abs() < 0.01);
    at(&mut s, 12.0);
    assert!((width(&s) - 150.0).abs() < 0.01, "retour");
    at(&mut s, 12.6);
    assert_eq!(width(&s), 200.0, "fin : la cible");
}

thread_local! {
    static TL: Cell<Option<TimelineState>> = const { Cell::new(None) };
    static EVENEMENTS: Cell<u32> = const { Cell::new(0) };
}

fn vue_timeline() -> impl View {
    let t = TimelineState::new(
        Timeline::new()
            .track(
                "largeur",
                [
                    Keyframe::new(0.0, 100.0).curve(Curve::Linear),
                    Keyframe::new(1.0, 200.0),
                ],
            )
            .event(0.5, "milieu"),
    );
    t.on_event(|nom| {
        assert_eq!(nom, "milieu");
        EVENEMENTS.with(|e| e.set(e.get() + 1));
    });
    TL.with(|c| c.set(Some(t)));
    Rectangle().frame(t.value("largeur"), 40)
}

#[test]
fn timeline_lecture_evenement_retour() {
    let mut s = surface(component(vue_timeline));
    let t = TL.with(|c| c.get()).unwrap();
    assert_eq!(width(&s), 100.0);
    assert!(!s.needs_frame(), "en pause : rien à faire");
    t.play();
    at(&mut s, 10.0);
    assert!(s.is_animating());
    at(&mut s, 10.5);
    assert!((width(&s) - 150.0).abs() < 0.01, "{}", width(&s));
    assert_eq!(EVENEMENTS.with(|e| e.get()), 1);
    at(&mut s, 11.2);
    assert_eq!(width(&s), 200.0);
    assert!(!t.is_playing());
    assert!(!s.is_animating());
    assert!(!s.needs_frame(), "0 % CPU au bout");

    // À l'envers depuis la fin : l'événement est franchi une seconde fois.
    t.reverse();
    t.play();
    at(&mut s, 12.0);
    at(&mut s, 12.75);
    assert!((width(&s) - 125.0).abs() < 0.01, "{}", width(&s));
    assert_eq!(EVENEMENTS.with(|e| e.get()), 2);

    // Pause puis déplacement à la main.
    t.pause();
    t.seek(0.1);
    at(&mut s, 13.0);
    assert!((width(&s) - 110.0).abs() < 0.01);
    assert!(!s.is_animating());
}

thread_local! {
    static COTE: Cell<Option<State<bool>>> = const { Cell::new(None) };
    static FINS: Cell<u32> = const { Cell::new(0) };
}

fn vue_liee() -> impl View {
    let droite = State::new(false);
    COTE.with(|c| c.set(Some(droite)));
    let pastille = |taille: f32, id: i64| {
        Circle()
            .frame(taille, taille)
            .matched_geometry_effect("pastille")
            .id(id)
    };
    let enfants = if droite.get() {
        vec![Rectangle().frame(100, 20).id(0), pastille(40.0, 2)]
    } else {
        vec![pastille(20.0, 1), Rectangle().frame(100, 20).id(0)]
    };
    veka_ui::views::stack(veka_ui::Axis::Horizontal, Some(0.0), enfants)
        .on_animation_end(|| FINS.with(|f| f.set(f.get() + 1)))
}

fn transforms(s: &Surface) -> Vec<veka_foundation::Transform2D> {
    s.draw_list()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::PushTransform(t) => Some(*t),
            _ => None,
        })
        .collect()
}

#[test]
fn geometrie_liee_et_fin_d_animation() {
    let mut s = surface(component(vue_liee));
    let avant = s.rects_of("Circle")[0];
    assert_eq!(avant.width(), 20.0);
    with_animation(Animation::linear(1.0), || {
        COTE.with(|c| c.get()).unwrap().set(true)
    });
    at(&mut s, 10.0);
    let apres = s.rects_of("Circle")[0];
    assert_eq!(apres.width(), 40.0);
    assert_eq!(
        s.rects_of("Circle").len(),
        1,
        "la source est retirée tout de suite"
    );
    at(&mut s, 10.5);
    assert_eq!(s.rects_of("Circle").len(), 1);
    let t = transforms(&s);
    assert_eq!(t.len(), 1, "{t:?}");
    // Le milieu : le centre à mi-chemin, 30 points de large.
    let c = t[0].apply(apres.center());
    let attendu = veka_foundation::Point::new(
        (avant.center().x + apres.center().x) / 2.0,
        (avant.center().y + apres.center().y) / 2.0,
    );
    assert!(
        (c.x - attendu.x).abs() < 0.01 && (c.y - attendu.y).abs() < 0.01,
        "{c:?} ≠ {attendu:?}"
    );
    assert!((t[0].a - 0.75).abs() < 0.01, "{}", t[0].a);
    assert_eq!(FINS.with(|f| f.get()), 0);
    at(&mut s, 11.0);
    assert!(transforms(&s).is_empty());
    assert!(!s.is_animating());
    assert_eq!(FINS.with(|f| f.get()), 1, "on_animation_end appelé une fois");
    at(&mut s, 12.0);
    assert_eq!(FINS.with(|f| f.get()), 1);
}

#[test]
fn reduire_les_animations() {
    let mut s = surface(component(vue_largeur));
    s.set_reduce_motion(true);
    at(&mut s, 10.0);
    with_animation(Animation::linear(1.0), || largeur().set(200.0));
    at(&mut s, 10.0);
    assert_eq!(width(&s), 200.0, "le layout ne s'anime pas");
    assert!(!s.is_animating());

    // Les transitions ne gardent que le fondu.
    let mut s = surface(component(vue_liste));
    s.set_reduce_motion(true);
    at(&mut s, 10.0);
    with_animation(Animation::linear(1.0), || LISTE.with(|c| c.get()).unwrap().set(3));
    at(&mut s, 10.0);
    at(&mut s, 10.5);
    assert_eq!(opacites(&s).len(), 1, "le fondu reste");
    assert!(transforms(&s).is_empty(), "pas de glissement");

    // `.ignore_reduce_motion()` : animé quand même.
    fn vue() -> impl View {
        component(vue_largeur).ignore_reduce_motion()
    }
    let mut s = surface(component(vue));
    s.set_reduce_motion(true);
    at(&mut s, 10.0);
    with_animation(Animation::linear(1.0), || largeur().set(200.0));
    at(&mut s, 10.0);
    at(&mut s, 10.5);
    assert!((width(&s) - 150.0).abs() < 0.01, "{}", width(&s));
}

#[test]
fn anima_dans_le_schema() {
    use veka_ui::schema::{Params, Schema, Value};
    let schema = Schema::builtin();
    let rust = Circle()
        .frame(20, 20)
        .opacity(0.5)
        .offset(3.0, 4.0)
        .scale_effect(1.5)
        .rotation_effect(45.0)
        .animation(Animation::snappy(), 1)
        .transition(Transition::scale(0.5))
        .matched_geometry_effect("a")
        .ignore_reduce_motion()
        .on_animation_end(|| {});
    for name in rust.primitive_names() {
        assert!(schema.get(name).is_some(), "`{name}` n'est pas dans le schéma");
    }
    let b = |name: &str, params: Params, child: Element| schema.build(name, params, vec![child]).unwrap();
    let mut e = Circle().frame(20, 20);
    e = b("opacity", Params::new().with("amount", Value::Float(0.5)), e);
    e = b(
        "offset",
        Params::new().with("x", Value::Int(3)).with("y", Value::Int(4)),
        e,
    );
    e = b("scaleEffect", Params::new().with("scale", Value::Float(1.5)), e);
    e = b("rotationEffect", Params::new().with("degrees", Value::Int(45)), e);
    e = b(
        "animation",
        Params::new()
            .with("spec", Value::Animation(Animation::snappy()))
            .with("value", Value::Int(1)),
        e,
    );
    e = b(
        "transition",
        Params::new().with("transition", Value::Transition(Transition::scale(0.5))),
        e,
    );
    e = b(
        "matchedGeometryEffect",
        Params::new().with("id", Value::String("a".into())),
        e,
    );
    e = b("ignoreReduceMotion", Params::new(), e);
    let donnees = b(
        "onAnimationEnd",
        Params::new().with("action", Value::Action(veka_ui::Action::new(|| {}))),
        e,
    );
    let mut a = Surface::new(rust);
    let mut d = Surface::new(donnees);
    for s in [&mut a, &mut d] {
        s.resize(Size::new(100.0, 100.0), 1.0);
        s.frame();
    }
    assert_eq!(a.draw_list().to_string(), d.draw_list().to_string());
    assert_eq!(Value::Int(1).key(), Value::Float(1.0).key());
}

#[test]
fn timeline_avant_la_premiere_heure() {
    // L'hôte dessine (ou reçoit un événement) avant de donner l'heure, puis donne une heure très
    // grande (le temps depuis le démarrage du PC) : la timeline part de là, sans rattraper.
    thread_local! {
        static BATTEMENTS: Cell<u32> = const { Cell::new(0) };
    }
    fn vue() -> impl View {
        let t = TimelineState::playing(
            Timeline::new()
                .track("x", [Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)])
                .event(0.5, "b")
                .looping(true),
        );
        t.on_event(|_| BATTEMENTS.with(|b| b.set(b.get() + 1)));
        Rectangle().frame(10, 10)
    }
    let mut s = Surface::new(component(vue));
    s.resize(Size::new(100.0, 100.0), 1.0);
    s.frame();
    s.frame();
    at(&mut s, 1_000_000.0);
    at(&mut s, 1_000_000.6);
    assert_eq!(BATTEMENTS.with(|b| b.get()), 1);
    // Un long saut (veille) : au plus un tour rejoué.
    at(&mut s, 1_000_500.0);
    assert!(BATTEMENTS.with(|b| b.get()) <= 3);
}
