//! Colora dans les vues : jetons, thèmes, mode clair / sombre, réglages, dégradés.

use veka_graphics::DrawCommand;
use veka_ui::prelude::*;

fn fills(view: Element, scheme: ColorScheme) -> Vec<Paint> {
    let mut s = Surface::new(view);
    s.set_system_appearance(scheme, None);
    s.resize(Size::new(200.0, 200.0), 1.0);
    s.frame()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::FillShape { paint, .. } => Some(paint.clone()),
            DrawCommand::Glyphs(run) => Some(run.paint.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn clair_et_sombre() {
    let vue = || Rectangle().frame(10, 10).into_element();
    let clair = fills(vue(), ColorScheme::Light);
    // Fond de la fenêtre, puis la forme (couleur de premier plan).
    assert_eq!(clair, [Paint::Solid(Color::White), Paint::Solid(Color::Black)]);
    let sombre = fills(vue(), ColorScheme::Dark);
    assert_eq!(sombre, [Paint::Solid(Color::Black), Paint::Solid(Color::White)]);
    // Forcé sur un sous-arbre.
    let force = fills(
        Rectangle().frame(10, 10).color_scheme(ColorScheme::Dark),
        ColorScheme::Light,
    );
    assert_eq!(force[1], Paint::Solid(Color::White));
}

#[test]
fn theme_et_jetons_de_controle() {
    let theme = Theme::new("Marque").color_all(Token::Tint, Color::Red);
    let p = fills(Button("Ok", || {}).theme(theme), ColorScheme::Light);
    // Le bouton prend ButtonAccent → ThemeAccent → Tint, redéfini par le thème.
    assert!(p.contains(&Paint::Solid(Color::Red)), "{p:?}");
}

#[test]
fn hierarchie() {
    let p = fills(
        Rectangle().fill(Token::Foreground.secondary()),
        ColorScheme::Light,
    );
    assert_eq!(p[1], Paint::Solid(Color::Black.with_alpha(0.6)));
}

#[test]
fn reglage_de_saturation() {
    let p = fills(Rectangle().fill(Color::Red).saturation(0.0), ColorScheme::Light);
    let c = p[1].solid().expect("uni");
    assert!((c.r - c.g).abs() < 1e-3 && (c.g - c.b).abs() < 1e-3, "{c:?}");
    // Le fond de la fenêtre (hors du sous-arbre) n'est pas touché.
    assert_eq!(p[0], Paint::Solid(Color::White));
}

#[test]
fn degrade_resolu() {
    let vue = Rectangle().fill(LinearGradient::new([
        Ink::from(Token::Accent),
        Color::White.into(),
    ]));
    let p = fills(vue, ColorScheme::Dark);
    match &p[1] {
        Paint::Linear(g) => {
            assert!(p[1].is_resolved());
            assert_eq!(g.gradient.stops[0].ink, Ink::Color(Color::hex(0x0A84FF)));
        }
        autre => panic!("{autre:?}"),
    }
}

#[test]
fn texte_au_premier_plan() {
    let p = fills(Text("a").foreground(Token::Red), ColorScheme::Light);
    assert_eq!(p.last(), Some(&Paint::Solid(Color::hex(0xFF3B30))));
}
