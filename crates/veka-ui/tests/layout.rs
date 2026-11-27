use veka_ui::prelude::*;

fn layout(view: Element, w: f32, h: f32) -> Surface {
    let mut s = Surface::new(view);
    s.resize(Size::new(w, h), 1.0);
    s.frame();
    s
}

#[test]
fn vstack_empile_et_centre() {
    let s = layout(
        VStack![spacing: 10; Rectangle().frame(40, 20), Rectangle().frame(60, 30)],
        200.0,
        200.0,
    );
    let r = s.rects_of("Rectangle");
    assert_eq!(r[0], Rect::new(80.0, 70.0, 40.0, 20.0));
    assert_eq!(r[1], Rect::new(70.0, 100.0, 60.0, 30.0));
}

#[test]
fn hstack_alignement_par_defaut_au_centre() {
    let s = layout(
        HStack![Rectangle().frame(10, 10), Rectangle().frame(10, 30)],
        100.0,
        100.0,
    );
    let r = s.rects_of("Rectangle");
    assert_eq!(r[0].y() + 5.0, r[1].y() + 15.0);
    assert_eq!(r[1].x() - r[0].max_x(), 8.0, "espacement par défaut");
}

#[test]
fn spacer_pousse_aux_extremites() {
    let s = layout(
        HStack![spacing: 0; Rectangle().frame(10, 10), Spacer(), Rectangle().frame(10, 10)],
        100.0,
        50.0,
    );
    let r = s.rects_of("Rectangle");
    assert_eq!(r[0].x(), 0.0);
    assert_eq!(r[1].max_x(), 100.0);
    // Le Spacer ne grandit pas la pile sur l'axe croisé.
    assert_eq!(s.rects_of("HStack")[0].height(), 10.0);
}

#[test]
fn forme_flexible_prend_le_reste() {
    let s = layout(
        VStack![spacing: 0; Rectangle().frame_height(20), Circle()],
        100.0,
        100.0,
    );
    let c = s.rects_of("Circle")[0];
    assert_eq!(c.height(), 80.0);
    assert_eq!(c.width(), 100.0);
}

#[test]
fn padding_et_frame() {
    let s = layout(
        Rectangle().frame(10, 10).padding(5).background(Color::Red),
        100.0,
        100.0,
    );
    assert_eq!(s.rects_of("background")[0], Rect::new(40.0, 40.0, 20.0, 20.0));
    assert_eq!(s.rects_of("Rectangle")[0], Rect::new(45.0, 45.0, 10.0, 10.0));
}

#[test]
fn texte_revient_a_la_ligne() {
    let long = "Un texte assez long pour ne pas tenir sur une seule ligne dans une petite fenêtre";
    let large = layout(Text(long), 2000.0, 400.0).rects_of("Text")[0];
    let etroit = layout(Text(long), 200.0, 400.0).rects_of("Text")[0];
    assert!(etroit.width() <= 200.0);
    assert!(etroit.height() > large.height() * 2.0);
}

#[test]
fn zstack_superpose() {
    let s = layout(
        ZStack![Rectangle().frame(50, 50), Circle().frame(20, 20)],
        100.0,
        100.0,
    );
    assert_eq!(
        s.rects_of("Circle")[0].center(),
        s.rects_of("Rectangle")[0].center()
    );
}

#[test]
fn spacer_laisse_la_place_au_texte() {
    // Le Spacer passe après le texte : le texte tient sur une ligne s'il y a la place.
    let titre = "Veka — fenêtre transparente";
    let seul = layout(Text(titre), 2000.0, 400.0).rects_of("Text")[0];
    let s = layout(
        HStack![Text(titre), Spacer(), Rectangle().frame(46, 32)],
        seul.width() + 46.0 + 40.0,
        400.0,
    );
    let t = s.rects_of("Text")[0];
    assert_eq!(t.height(), seul.height());
    assert_eq!(s.rects_of("Rectangle")[0].max_x(), seul.width() + 86.0);
}
