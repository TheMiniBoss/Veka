//! Verre liquide : groupes qui fusionnent, lumière du pointeur, replis.

use veka_graphics::{BackdropEffect, DrawCommand};
use veka_ui::prelude::*;
use veka_ui::{InputEvent, Surface};

fn surface(view: Element) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(400.0, 200.0), 1.0);
    s.frame();
    s
}

fn verres(s: &Surface) -> Vec<veka_graphics::GlassEffect> {
    s.draw_list()
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Backdrop {
                effect: BackdropEffect::Glass(g),
                ..
            } => Some(**g),
            _ => None,
        })
        .collect()
}

fn barre(spacing: f32) -> Element {
    HStack![spacing: spacing;
        Text("A").padding(10).glass(),
        Text("B").padding(10).glass().offset(0.0, 4.0),
        Text("C").padding(10).glass(),
    ]
    .glass_group(12.0)
}

#[test]
fn un_groupe_donne_un_seul_verre() {
    let s = surface(barre(2.0));
    let v = verres(&s);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].count, 3);
    assert_eq!(v[0].smoothness, 12.0);
    assert!(!s.draw_list().commands.iter().any(|c| matches!(
        c,
        DrawCommand::Backdrop {
            effect: BackdropEffect::GlassMember,
            ..
        }
    )));
    // Le verre est dessiné avant le contenu des boutons (le texte reste au-dessus).
    let first_glass = s
        .draw_list()
        .commands
        .iter()
        .position(|c| matches!(c, DrawCommand::Backdrop { .. }))
        .unwrap();
    let first_text = s
        .draw_list()
        .commands
        .iter()
        .position(|c| matches!(c, DrawCommand::Glyphs(_)))
        .unwrap();
    assert!(first_glass < first_text);
    // Les formes suivent les boutons, y compris l'`offset` de B (repère du groupe).
    let (a, b) = (v[0].shapes[0].rect, v[0].shapes[1].rect);
    assert!(b.x() > a.max_x());
    assert!((b.y() - a.y() - 4.0).abs() < 0.01, "{a:?} {b:?}");
}

#[test]
fn sans_groupe_chaque_verre_est_seul() {
    let s = surface(
        HStack![spacing: 8;
            Text("A").padding(10).glass(),
            Text("B").padding(10).glass_with(Glass::clear().shape(Shape::Circle)),
        ]
        .into_element(),
    );
    let v = verres(&s);
    assert_eq!(v.len(), 2);
    assert!(v.iter().all(|g| g.count == 0));
    // Clair : moins teinté et moins flou que le réglage par défaut.
    assert!(v[1].tint_amount < v[0].tint_amount);
    assert!(v[1].blur < v[0].blur);
}

#[test]
fn le_pointeur_n_eveille_que_les_verres_interactifs() {
    let mut s = surface(Text("A").padding(10).glass());
    assert!(!s.handle(InputEvent::PointerMoved {
        position: Point::new(5.0, 5.0)
    }));
    let mut s = surface(
        Text("A")
            .padding(10)
            .glass_with(Glass::regular().interactive(true)),
    );
    assert!(s.handle(InputEvent::PointerMoved {
        position: Point::new(5.0, 5.0)
    }));
    s.frame();
    assert_eq!(s.draw_list().pointer, Some(Point::new(5.0, 5.0)));
    assert!(s.handle(InputEvent::PointerLeft));
    s.frame();
    assert_eq!(s.draw_list().pointer, None);
}

#[test]
fn reduire_la_transparence_donne_des_teintes_unies() {
    let mut s = Surface::new(
        VStack![spacing: 8;
            barre(2.0),
            Text("D").glass(),
            Text("E").frosted_glass(),
            Text("F").liquid_distortion(6.0, 90.0, 1.0, Shape::Circle),
        ]
        .into_element(),
    );
    s.resize(Size::new(400.0, 300.0), 1.0);
    s.set_reduce_transparency(true);
    s.frame();
    let cmds = &s.draw_list().commands;
    assert!(!cmds.iter().any(|c| matches!(c, DrawCommand::Backdrop { .. })));
    // Les trois formes du groupe, le verre seul et le dépoli : cinq teintes unies (plus le texte).
    let fills = cmds
        .iter()
        .filter(|c| matches!(c, DrawCommand::FillShape { .. }))
        .count();
    assert!(fills >= 5, "{}", s.draw_list());
}
