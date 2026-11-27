//! Le compteur de `examples/c/compteur.c`, écrit avec les mêmes fonctions, depuis Rust.

use std::ffi::{CStr, c_void};

use veka_ui::{InputEvent, PointerButton, Size, Surface, component};

use super::*;

struct Compteur {
    clics: *mut VekaState,
}

unsafe extern "C" fn incrementer(cx: *mut VekaContext, user: *mut c_void) {
    let c = unsafe { &mut *(user as *mut Compteur) };
    veka_state_set_int(cx, c.clics, veka_state_get_int(cx, c.clics) + 1);
}

unsafe extern "C" fn setup(cx: *mut VekaContext, user: *mut c_void) {
    let c = unsafe { &mut *(user as *mut Compteur) };
    c.clics = veka_state_new_int(cx, 0);
}

unsafe extern "C" fn body(cx: *mut VekaContext, user: *mut c_void) -> *mut VekaElement {
    let c = unsafe { &mut *(user as *mut Compteur) };
    let libelle = std::ffi::CString::new(format!("Clics : {}", veka_state_get_int(cx, c.clics))).unwrap();
    unsafe {
        let formes = [
            veka_frame(
                veka_fill(veka_rounded_rect(12.0), veka_rgb(0.25, 0.5, 1.0)),
                80.0,
                48.0,
            ),
            veka_frame(veka_fill(veka_capsule(), veka_rgb(1.0, 0.6, 0.2)), 96.0, 48.0),
            veka_frame(veka_fill(veka_circle(), veka_rgb(0.3, 0.8, 0.4)), 48.0, 48.0),
        ];
        let lignes = [
            veka_font(veka_text(c"Bonjour Veka".as_ptr()), VekaFont::Title),
            veka_hstack(12.0, formes.as_ptr(), formes.len()),
            veka_button(libelle.as_ptr(), Some(incrementer), user),
        ];
        veka_padding(veka_vstack(16.0, lignes.as_ptr(), lignes.len()), 24.0)
    }
}

#[test]
fn compteur_en_c() {
    let mut c = Compteur {
        clics: std::ptr::null_mut(),
    };
    let user = &mut c as *mut Compteur as *mut c_void;
    let mut surface = Surface::new(component(app::window_view(Some(setup), Some(body), user)));
    surface.resize(Size::new(480.0, 320.0), 1.0);
    surface.frame();
    assert!(
        surface.texts().contains(&"Clics : 0".to_string()),
        "{:?}",
        surface.texts()
    );
    assert_eq!(surface.rects_of("RoundedRectangle").len(), 1);
    assert_eq!(surface.rects_of("Circle").len(), 1);

    let bouton = surface.rects_of("Button")[0].center();
    for _ in 0..3 {
        surface.handle(InputEvent::PointerDown {
            position: bouton,
            button: PointerButton::Primary,
        });
        surface.handle(InputEvent::PointerUp {
            position: bouton,
            button: PointerButton::Primary,
        });
    }
    surface.frame();
    assert!(
        surface.texts().contains(&"Clics : 3".to_string()),
        "{:?}",
        surface.texts()
    );
}

#[test]
fn erreurs_sans_panique() {
    unsafe {
        // Paramètre inconnu : NULL et un message.
        let params = [VekaParam {
            name: c"nimporte".as_ptr(),
            value: veka_number(1.0),
        }];
        let e = veka_element(c"VStack".as_ptr(), params.as_ptr(), 1, std::ptr::null(), 0);
        assert!(e.is_null());
        let msg = CStr::from_ptr(veka_last_error()).to_str().unwrap();
        assert!(msg.contains("nimporte"), "{msg}");

        // Composant inconnu.
        assert!(veka_element(c"Truc".as_ptr(), std::ptr::null(), 0, std::ptr::null(), 0).is_null());

        // Un état hors d'une vue : NULL, pas de panique.
        veka_clear_error();
        assert!(veka_state_new_int(std::ptr::null_mut(), 0).is_null());
        assert!(!veka_last_error().is_null());

        // Un modifier sur NULL.
        assert!(veka_padding(std::ptr::null_mut(), 4.0).is_null());

        // Chemin données complet : VStack(spacing: 8) { Text("a") }.padding(4)
        let enfants = [veka_text(c"a".as_ptr())];
        let params = [VekaParam {
            name: c"spacing".as_ptr(),
            value: veka_number(8.0),
        }];
        let pile = veka_element(c"VStack".as_ptr(), params.as_ptr(), 1, enfants.as_ptr(), 1);
        assert!(!pile.is_null());
        let params = [VekaParam {
            name: c"all".as_ptr(),
            value: veka_number(4.0),
        }];
        let e = veka_modifier(pile, c"padding".as_ptr(), params.as_ptr(), 1);
        assert!(!e.is_null());
        veka_element_free(e);
    }
}

#[test]
fn fenetre_personnalisee() {
    unsafe {
        let lignes = [
            veka_title_bar(c"Veka".as_ptr()),
            veka_text(c"Contenu".as_ptr()),
            veka_spacer(),
        ];
        let vue = veka_vstack(0.0, lignes.as_ptr(), lignes.len());
        assert!(!vue.is_null());
        let mut s = Surface::new(Box::from_raw(vue).element);
        s.resize(Size::new(480.0, 320.0), 1.0);
        s.frame();
        assert_eq!(s.regions().len(), 4, "barre + 3 boutons");

        let fermer = veka_window_control(veka_rectangle(), VekaWindowControl::Close);
        let barre = veka_window_drag_area(veka_hstack(0.0, [fermer].as_ptr(), 1));
        assert!(!barre.is_null());
        veka_element_free(barre);
        veka_element_free(veka_window_button(VekaWindowControl::Maximize));

        let mut desc = veka_window_desc_default(c"Veka".as_ptr());
        desc.chrome = VekaChrome::Custom;
        let app = veka_app_new();
        assert_eq!(
            veka_app_add_window_ex(app, &desc, None, Some(body), std::ptr::null_mut()),
            0
        );
        veka_app_free(app);
    }
}

#[test]
fn colora_en_c() {
    use veka_colora::{ColorScheme, Paint};
    use veka_graphics::DrawCommand;

    let peintures = |e: *mut VekaElement, scheme: ColorScheme| -> Vec<Paint> {
        assert!(!e.is_null(), "{:?}", unsafe { CStr::from_ptr(veka_last_error()) });
        let mut s = Surface::new(unsafe { Box::from_raw(e) }.element);
        s.set_system_appearance(scheme, None);
        s.resize(Size::new(200.0, 200.0), 1.0);
        s.frame()
            .commands
            .iter()
            .filter_map(|c| match c {
                DrawCommand::FillShape { paint, .. } => Some(paint.clone()),
                _ => None,
            })
            .collect()
    };
    unsafe {
        // Un thème : Tint orange en clair, et un jeton maison qui vaut un dégradé.
        let theme = veka_theme_new(c"Marque".as_ptr(), std::ptr::null());
        assert_eq!(
            veka_theme_set_color(
                theme,
                c"Tint".as_ptr(),
                veka_solid(veka_hex(0xFF6A00)),
                veka_solid(veka_hex(0xFF8A33)),
            ),
            0
        );
        let couleurs = [veka_hex(0x000000), veka_hex(0xFFFFFF)];
        let degrade = veka_linear_gradient(couleurs.as_ptr(), 2, VEKA_UNIT_TOP, VEKA_UNIT_BOTTOM);
        assert_eq!(
            veka_theme_set_color(theme, c"Marque.Fond".as_ptr(), degrade, std::ptr::null_mut()),
            0
        );

        let forme = |jeton: &CStr| {
            veka_frame(
                veka_fill_paint(
                    veka_rectangle(),
                    veka_token(jeton.as_ptr(), VekaHierarchy::Primary),
                ),
                10.0,
                10.0,
            )
        };
        let vue = veka_theme(forme(c"Tint"), theme);
        let p = peintures(vue, ColorScheme::Light);
        assert_eq!(p[1], Paint::Solid(veka_foundation::Color::hex(0xFF6A00)));
        let vue = veka_color_scheme(veka_theme(forme(c"Tint"), theme), VekaColorScheme::Dark);
        let p = peintures(vue, ColorScheme::Light);
        assert_eq!(p[1], Paint::Solid(veka_foundation::Color::hex(0xFF8A33)));
        let p = peintures(veka_theme(forme(c"Marque.Fond"), theme), ColorScheme::Dark);
        assert!(matches!(p[1], Paint::Linear(_)), "{p:?}");

        // Arrêts en jetons, espace d'interpolation, réglage.
        let arrets = [
            VekaStop {
                color: veka_rgb(0.0, 0.0, 0.0),
                token: c"Blue".as_ptr(),
                hierarchy: VekaHierarchy::Primary,
                location: 0.0,
            },
            VekaStop {
                color: veka_rgb(1.0, 1.0, 1.0),
                token: std::ptr::null(),
                hierarchy: VekaHierarchy::Primary,
                location: 1.0,
            },
        ];
        let g = veka_angular_gradient(couleurs.as_ptr(), 2, VEKA_UNIT_CENTER, 0.0, 360.0);
        let g = veka_paint_space(veka_paint_stops(g, arrets.as_ptr(), 2), VekaColorSpace::OkLch);
        let p = peintures(veka_fill_paint(veka_circle(), g), ColorScheme::Light);
        match &p[1] {
            Paint::Angular(a) => {
                assert!(p[1].is_resolved());
                assert_eq!(a.gradient.space, veka_colora::ColorSpace::OkLch);
            }
            autre => panic!("{autre:?}"),
        }
        let p = peintures(
            veka_grayscale(veka_fill(veka_rectangle(), veka_hex(0xFF0000)), 1.0),
            ColorScheme::Light,
        );
        let c = p[1].solid().unwrap();
        assert!((c.r - c.g).abs() < 1e-3, "{c:?}");

        // Le chemin données : fill(paint:) avec une peinture prêtée.
        let maille = [veka_hex(0xFF0000); 4];
        let mesh = veka_mesh_gradient(2, 2, std::ptr::null(), maille.as_ptr());
        let params = [VekaParam {
            name: c"paint".as_ptr(),
            value: veka_paint_value(mesh),
        }];
        let e = veka_modifier(veka_rectangle(), c"fill".as_ptr(), params.as_ptr(), 1);
        assert!(matches!(peintures(e, ColorScheme::Light)[1], Paint::Mesh(_)));
        veka_paint_free(mesh);

        // Erreurs : jeton NULL, maillage trop petit, arrêts sur une couleur unie.
        assert!(veka_token(std::ptr::null(), VekaHierarchy::Primary).is_null());
        assert!(veka_mesh_gradient(1, 3, std::ptr::null(), maille.as_ptr()).is_null());
        assert!(veka_paint_stops(veka_solid(veka_hex(0)), arrets.as_ptr(), 2).is_null());
        assert!(veka_fill_paint(veka_rectangle(), std::ptr::null_mut()).is_null());

        veka_theme_free(theme);
    }
}

struct Anima {
    ouvert: *mut VekaState,
    tl: *mut VekaState,
    modele: *mut VekaTimeline,
    evenements: u32,
}

unsafe extern "C" fn ouvrir(cx: *mut VekaContext, user: *mut c_void) {
    let a = unsafe { &mut *(user as *mut Anima) };
    veka_state_set_bool(cx, a.ouvert, true);
}

unsafe extern "C" fn compter(_cx: *mut VekaContext, name: *const std::ffi::c_char, user: *mut c_void) {
    let a = unsafe { &mut *(user as *mut Anima) };
    assert_eq!(unsafe { CStr::from_ptr(name) }, c"pic");
    a.evenements += 1;
}

unsafe extern "C" fn anima_setup(cx: *mut VekaContext, user: *mut c_void) {
    let a = unsafe { &mut *(user as *mut Anima) };
    a.ouvert = veka_state_new_bool(cx, false);
    a.tl = unsafe { veka_state_new_timeline(cx, a.modele, true) };
}

unsafe extern "C" fn anima_body(cx: *mut VekaContext, user: *mut c_void) -> *mut VekaElement {
    let a = unsafe { &mut *(user as *mut Anima) };
    veka_timeline_on_event(cx, a.tl, Some(compter), user);
    let largeur = if veka_state_get_bool(cx, a.ouvert) {
        200.0
    } else {
        100.0
    };
    unsafe {
        let lignes = [
            veka_frame(veka_rectangle(), largeur, 40.0),
            veka_opacity(
                veka_frame(veka_circle(), 10.0, 10.0),
                veka_timeline_value(cx, a.tl, c"o".as_ptr()),
            ),
        ];
        veka_vstack(0.0, lignes.as_ptr(), lignes.len())
    }
}

#[test]
fn anima_en_c() {
    use veka_foundation::Instant;
    use veka_graphics::DrawCommand;

    let at = |s: &mut Surface, t: f64| {
        s.frame_at(Instant::from_nanos((t * 1e9) as u64));
    };
    let opacites = |s: &Surface| -> Vec<f32> {
        s.draw_list()
            .commands
            .iter()
            .filter_map(|c| match c {
                DrawCommand::PushLayer { opacity } => Some(*opacity),
                _ => None,
            })
            .collect()
    };
    unsafe {
        let modele = veka_timeline_new();
        veka_timeline_key(modele, c"o".as_ptr(), 0.0, 0.0, VekaCurve::Linear);
        veka_timeline_key(modele, c"o".as_ptr(), 1.0, 1.0, VekaCurve::Linear);
        veka_timeline_event(modele, 0.5, c"pic".as_ptr());
        let mut a = Anima {
            ouvert: std::ptr::null_mut(),
            tl: std::ptr::null_mut(),
            modele,
            evenements: 0,
        };
        let user = &mut a as *mut Anima as *mut c_void;
        let mut s = Surface::new(component(app::window_view(
            Some(anima_setup),
            Some(anima_body),
            user,
        )));
        s.resize(Size::new(400.0, 300.0), 1.0);
        at(&mut s, 10.0);
        assert!(veka_timeline_is_playing(context(), a.tl));
        veka_with_animation(context(), veka_animation_linear(1.0), Some(ouvrir), user);
        at(&mut s, 10.0);
        at(&mut s, 10.5);
        let w = s.rects_of("Rectangle")[0].width();
        assert!((w - 150.0).abs() < 0.01, "{w}");
        let o = opacites(&s);
        assert!(o.len() == 1 && (o[0] - 0.5).abs() < 0.01, "{o:?}");
        assert_eq!(a.evenements, 1);
        at(&mut s, 11.5);
        assert!(!veka_timeline_is_playing(context(), a.tl));
        assert!(!s.is_animating());
        veka_timeline_free(modele);
    }
}

#[test]
fn anima_c_comme_rust() {
    use veka_ui::prelude::*;
    let rust = Circle()
        .frame(20, 20)
        .opacity(0.5)
        .offset(3.0, 4.0)
        .scale_effect(1.5)
        .rotation_effect(45.0)
        .animation(Animation::snappy().delay(0.2), 1)
        .transition(Transition::asymmetric(
            Transition::push(Edge::Top).combined(Transition::scale(0.5)),
            Transition::opacity(),
        ))
        .matched_geometry_effect("a")
        .ignore_reduce_motion();
    unsafe {
        let mut e = veka_frame(veka_circle(), 20.0, 20.0);
        e = veka_opacity(e, 0.5);
        e = veka_offset(e, 3.0, 4.0);
        e = veka_scale_effect(e, 1.5);
        e = veka_rotation_effect(e, 45.0);
        let spec = veka_animation_delay(veka_animation_snappy(), 0.2);
        e = veka_animation(e, &spec, 1);
        let t = veka_transition_asymmetric(
            veka_transition_combined(veka_transition_push(VekaEdge::Top), veka_transition_scale(0.5)),
            veka_transition_opacity(),
        );
        e = veka_transition(e, t);
        e = veka_matched_geometry_effect(e, c"a".as_ptr());
        e = veka_ignore_reduce_motion(e);
        assert!(!e.is_null(), "{:?}", CStr::from_ptr(veka_last_error()));
        let c = Box::from_raw(e).element;
        let mut a = Surface::new(rust);
        let mut b = Surface::new(c);
        for s in [&mut a, &mut b] {
            s.resize(Size::new(100.0, 100.0), 1.0);
            s.frame();
        }
        assert_eq!(a.draw_list().to_string(), b.draw_list().to_string());
        assert_eq!(Animation::from(spec), Animation::snappy().delay(0.2));

        // Aller-retour des specs.
        for a in [
            Animation::linear(1.0),
            Animation::ease_in(0.3).repeat(3, true),
            Animation::circ_ease_in_out(2.0).repeat_forever(false).speed(2.0),
            Animation::timing_curve(0.3, 1.6, 0.6, 1.0, 1.0),
            Animation::bouncy(),
        ] {
            assert_eq!(Animation::from(VekaAnimation::from(a)), a);
        }
        // Erreurs.
        assert!(veka_transition(veka_rectangle(), std::ptr::null_mut()).is_null());
        assert!(veka_transition_combined(veka_transition_opacity(), std::ptr::null_mut()).is_null());
        assert!(veka_state_new_timeline(std::ptr::null_mut(), std::ptr::null(), false).is_null());
        veka_with_animation(
            std::ptr::null_mut(),
            veka_animation_smooth(),
            None,
            std::ptr::null_mut(),
        );
        assert!(!veka_last_error().is_null());
    }
}

// --- Le langage .vk depuis le C -------------------------------------------------------------------

const BOUTIQUE_VK: &std::ffi::CStr = c"view Boutique {
    @Event acheter(String)
    VStack {
        ForEach(model.articles) { a in
            Button(\"\\(a.nom) : \\(a.prix) or\") {
                emit acheter(a.nom)
            }
        }
    }
}
";

unsafe extern "C" fn recu(
    view: *const c_char,
    event: *const c_char,
    value: *const VekaData,
    user: *mut c_void,
) {
    let out = unsafe { &mut *(user as *mut Vec<String>) };
    let mut buf = [0 as c_char; 64];
    unsafe { veka_data_text(value, buf.as_mut_ptr(), buf.len()) };
    let s = |p: *const c_char| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    out.push(format!("{}.{}({})", s(view), s(event), s(buf.as_ptr())));
}

#[test]
fn vk_depuis_le_c() {
    unsafe {
        let vk = veka_vk_load(BOUTIQUE_VK.as_ptr());
        assert!(!vk.is_null());
        let model = veka_data_record();
        let articles = veka_data_array();
        for (nom, prix) in [(c"Épée", 12), (c"Arc", 9)] {
            let a = veka_data_record();
            assert_eq!(
                veka_data_set(a, c"nom".as_ptr(), veka_data_string(nom.as_ptr())),
                0
            );
            assert_eq!(veka_data_set(a, c"prix".as_ptr(), veka_data_int(prix)), 0);
            assert_eq!(veka_data_push(articles, a), 0);
        }
        assert_eq!(veka_data_len(articles), 2);
        assert_eq!(
            veka_data_as_int(veka_data_get(veka_data_at(articles, 1), c"prix".as_ptr())),
            9
        );
        assert_eq!(veka_data_kind(veka_data_at(articles, 0)), VekaDataKind::Record);
        assert_eq!(veka_data_set(model, c"articles".as_ptr(), articles), 0);
        assert_eq!(veka_vk_set_model(vk, model), 0);
        let mut events: Vec<String> = Vec::new();
        assert_eq!(
            veka_vk_on_event(vk, Some(recu), &mut events as *mut Vec<String> as *mut c_void),
            0
        );
        let view = take(veka_vk_view(vk, c"Boutique".as_ptr())).unwrap();
        let mut s = Surface::new(view);
        s.resize(Size::new(300.0, 200.0), 1.0);
        s.frame();
        assert_eq!(s.texts(), ["Épée : 12 or", "Arc : 9 or"]);
        let p = s.rects_of("Button")[1].center();
        s.handle(InputEvent::PointerDown {
            position: p,
            button: PointerButton::Primary,
        });
        s.handle(InputEvent::PointerUp {
            position: p,
            button: PointerButton::Primary,
        });
        assert_eq!(events, ["Boutique.acheter(Arc)"]);
        // Une erreur : NULL et un message avec sa position.
        assert!(veka_vk_load(c"view A {\n    Txt(\"x\")\n}\n".as_ptr()).is_null());
        let e = CStr::from_ptr(veka_last_error()).to_string_lossy().into_owned();
        assert!(e.starts_with("2:5") && e.contains("`Text`"), "{e}");
        assert!(veka_vk_view(vk, c"Absente".as_ptr()).is_null());
        drop(s);
        veka_vk_free(vk);
    }
}

#[test]
fn flou_d_arriere_plan_en_c() {
    use veka_ui::prelude::*;
    let dessin = |e: Element| {
        let mut s = Surface::new(e);
        s.resize(Size::new(200.0, 100.0), 1.0);
        s.frame().to_string()
    };
    unsafe {
        let c = veka_frosted_glass(veka_text(c"Verre".as_ptr()));
        assert!(!c.is_null());
        assert_eq!(
            dessin(Box::from_raw(c).element),
            dessin(Text("Verre").frosted_glass())
        );

        let mut b = veka_backdrop_default();
        b.radius = 12.0;
        b.shape = VekaBackdropShape::Capsule;
        let c = veka_backdrop(veka_text(c"Barre".as_ptr()), &b, std::ptr::null_mut());
        let rs = Text("Barre").backdrop(BackdropBlur::default().radius(12.0).shape(Shape::Capsule));
        let d = dessin(Box::from_raw(c).element);
        assert!(d.contains("backdrop Capsule"), "{d}");
        assert_eq!(d, dessin(rs));

        let app = veka_app_new();
        assert_eq!(veka_app_set_backdrop_quality(app, VekaBackdropQuality::Low), 0);
        veka_app_free(app);
        assert!(veka_backdrop(veka_text(c"x".as_ptr()), std::ptr::null(), std::ptr::null_mut()).is_null());
    }
}
