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
