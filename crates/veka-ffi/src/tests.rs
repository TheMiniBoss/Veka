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
