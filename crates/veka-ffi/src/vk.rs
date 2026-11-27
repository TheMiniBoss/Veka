//! Le langage `.vk` depuis le C : charger un fichier, en construire les vues, lui donner des
//! données (`VekaData`) et recevoir ses événements.

use std::ffi::{CString, c_char, c_void};
use std::path::PathBuf;
use std::rc::Rc;

use veka_lang::{Program, Val};
use veka_ui::schema::Schema;

use crate::app::{VekaApp, VekaWindowDesc, window_desc};
use crate::{VekaElement, boxed, guard, set_error, string};

/// Un fichier `.vk` chargé. Opaque ; à libérer avec `veka_vk_free`.
pub struct VekaVk {
    program: Program,
}

/// Une donnée pour (ou de) un fichier `.vk` : nombre, texte, booléen, tableau, enregistrement.
/// Opaque. Celles que crée le C lui appartiennent jusqu'à ce qu'il les donne (`veka_data_push`,
/// `veka_data_set`, `veka_vk_set_model` les consomment).
///
/// (Côté Rust, un pointeur `VekaData` est un pointeur vers une `Val`.)
pub struct VekaData {
    _private: [u8; 0],
}

/// La nature d'une `VekaData`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VekaDataKind {
    None,
    Bool,
    Int,
    Float,
    String,
    Array,
    Record,
    /// Couleur, symbole, peinture… : lisible seulement en texte.
    Other,
}

fn data(value: Val) -> *mut VekaData {
    Box::into_raw(Box::new(value)).cast()
}

fn lend(v: &Val) -> *const VekaData {
    (v as *const Val).cast()
}

unsafe fn take_data(d: *mut VekaData) -> Result<Val, String> {
    if d.is_null() {
        return Err("donnée NULL".into());
    }
    // SAFETY : obtenue de `data` et pas encore consommée (contrat de l'API).
    Ok(*unsafe { Box::from_raw(d.cast::<Val>()) })
}

unsafe fn borrow_data<'a>(d: *const VekaData) -> Option<&'a Val> {
    // SAFETY : `d` est NULL ou une donnée valide (contrat de l'API).
    unsafe { d.cast::<Val>().as_ref() }
}

unsafe fn borrow_data_mut<'a>(d: *mut VekaData) -> Option<&'a mut Val> {
    // SAFETY : `d` est NULL ou une donnée du C, pas empruntée ailleurs (contrat de l'API).
    unsafe { d.cast::<Val>().as_mut() }
}

fn errors_text(p: &Program) -> Option<String> {
    let e: Vec<String> = p
        .errors()
        .iter()
        .filter(|d| d.is_error())
        .map(ToString::to_string)
        .collect();
    (!e.is_empty()).then(|| e.join("\n"))
}

/// Charge un fichier `.vk` depuis son texte. NULL si le texte a des erreurs (`veka_last_error`
/// les donne, une par ligne, avec leur position).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_load(source: *const c_char) -> *mut VekaVk {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let source = unsafe { string(source, "texte .vk") }?;
        let program = Program::new(source);
        match errors_text(&program) {
            Some(e) => Err(e),
            None => Ok(Box::into_raw(Box::new(VekaVk { program }))),
        }
    })
}

/// Charge un fichier `.vk` depuis le disque ; ses `import` sont relatifs à son dossier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_load_file(path: *const c_char) -> *mut VekaVk {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let path = PathBuf::from(unsafe { string(path, "chemin") }?);
        let source = std::fs::read_to_string(&path).map_err(|e| format!("{} : {e}", path.display()))?;
        let dir = path.parent().map(PathBuf::from).unwrap_or_default();
        let program = Program::with_loader(&source, Schema::builtin(), move |p| {
            std::fs::read_to_string(dir.join(p)).map_err(|e| e.to_string())
        });
        match errors_text(&program) {
            Some(e) => Err(e),
            None => Ok(Box::into_raw(Box::new(VekaVk { program }))),
        }
    })
}

/// Remplace le texte du fichier ; les états des vues affichées gardent leurs valeurs. Renvoie 0,
/// ou -1 si le nouveau texte a des erreurs (l'ancien reste alors en place).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_reload(vk: *mut VekaVk, source: *const c_char) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API.
        let vk = unsafe { vk.as_ref() }.ok_or("fichier .vk NULL")?;
        let source = unsafe { string(source, "texte .vk") }?;
        vk.program
            .reload(source)
            .map(|()| 0)
            .map_err(|e| e.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))
    })
}

/// La vue `name` du fichier, comme un élément (à donner à une pile ou à renvoyer de `body`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_view(vk: *mut VekaVk, name: *const c_char) -> *mut VekaElement {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        let vk = unsafe { vk.as_ref() }.ok_or("fichier .vk NULL")?;
        let name = unsafe { string(name, "nom de vue") }?;
        if vk.program.module().view(name).is_none() {
            return Err(format!("vue `{name}` absente du fichier"));
        }
        Ok(boxed(vk.program.view(name)))
    })
}

/// Donne ses données au fichier (`model` dans le `.vk`) ; consomme `model`. Les vues affichées
/// sont ré-évaluées.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_set_model(vk: *mut VekaVk, model: *mut VekaData) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API.
        let model = unsafe { take_data(model) }?;
        let vk = unsafe { vk.as_ref() }.ok_or("fichier .vk NULL")?;
        vk.program.set_model(model);
        Ok(0)
    })
}

/// Reçoit un `emit` qu'aucune vue du fichier n'écoute. `value` n'est valable que pendant l'appel
/// (NULL pour un événement sans valeur).
pub type VekaVkEventFn = Option<
    unsafe extern "C" fn(
        view: *const c_char,
        event: *const c_char,
        value: *const VekaData,
        user: *mut c_void,
    ),
>;

/// Installe le rappel d'événements du fichier (remplace le précédent).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_on_event(vk: *mut VekaVk, f: VekaVkEventFn, user: *mut c_void) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API.
        let vk = unsafe { vk.as_ref() }.ok_or("fichier .vk NULL")?;
        let f = f.ok_or("rappel NULL")?;
        let user = user as usize;
        vk.program.on_event(move |view, event, value| {
            let (view, event) = (cstring(view), cstring(event));
            let value = if matches!(value, Val::Unit) {
                std::ptr::null()
            } else {
                lend(value)
            };
            // SAFETY : fonction et `user` fournis par le C ; chaînes et valeur valides pendant l'appel.
            unsafe { f(view.as_ptr(), event.as_ptr(), value, user as *mut c_void) };
        });
        Ok(0)
    })
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', " ")).unwrap_or_default()
}

/// Libère un fichier `.vk` (les vues déjà construites restent valables).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_vk_free(vk: *mut VekaVk) {
    if !vk.is_null() {
        // SAFETY : contrat de l'API.
        drop(unsafe { Box::from_raw(vk) });
    }
}

/// Ajoute une fenêtre qui affiche la vue `view` du fichier `path`, relu à chaud en debug. Rend le
/// fichier chargé (pour `veka_vk_set_model` et `veka_vk_on_event`), à libérer avec
/// `veka_vk_free` ; NULL en cas d'erreur.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_app_add_window_vk(
    app: *mut VekaApp,
    desc: *const VekaWindowDesc,
    path: *const c_char,
    view: *const c_char,
) -> *mut VekaVk {
    guard(std::ptr::null_mut(), || {
        if app.is_null() || desc.is_null() {
            return Err("application ou réglages NULL".into());
        }
        // SAFETY : contrat de l'API.
        let (app, d) = unsafe { (&mut *app, *desc) };
        let desc = unsafe { window_desc(&d) }?;
        let path = unsafe { string(path, "chemin") }?;
        let view = unsafe { string(view, "nom de vue") }?;
        let program = app.app.window_vk_program(desc, path, view);
        Ok(Box::into_raw(Box::new(VekaVk { program })))
    })
}

// --- Données ----------------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn veka_data_int(v: i64) -> *mut VekaData {
    data(Val::Int(v))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_data_float(v: f64) -> *mut VekaData {
    data(Val::Float(v))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_data_bool(v: bool) -> *mut VekaData {
    data(Val::Bool(v))
}

/// Un texte (copié). NULL si `s` n'est pas une chaîne UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_string(s: *const c_char) -> *mut VekaData {
    guard(std::ptr::null_mut(), || {
        // SAFETY : contrat de l'API.
        Ok(data(Val::from(unsafe { string(s, "texte") }?)))
    })
}

/// Un tableau vide (`veka_data_push` le remplit).
#[unsafe(no_mangle)]
pub extern "C" fn veka_data_array() -> *mut VekaData {
    data(Val::from(Vec::new()))
}

/// Un enregistrement vide (`veka_data_set` le remplit).
#[unsafe(no_mangle)]
pub extern "C" fn veka_data_record() -> *mut VekaData {
    data(Val::record(Vec::<(&str, Val)>::new()))
}

/// Ajoute `item` à la fin du tableau `array` ; consomme `item`. Renvoie 0, ou -1.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_push(array: *mut VekaData, item: *mut VekaData) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API.
        let item = unsafe { take_data(item) }?;
        let array = unsafe { borrow_data_mut(array) }.ok_or("tableau NULL")?;
        match array {
            Val::Array(a) => {
                Rc::make_mut(a).push(item);
                Ok(0)
            }
            v => Err(format!(
                "veka_data_push : un tableau attendu, trouvé {}",
                v.type_name()
            )),
        }
    })
}

/// Donne au champ `key` de l'enregistrement `record` la valeur `value` ; consomme `value`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_set(
    record: *mut VekaData,
    key: *const c_char,
    value: *mut VekaData,
) -> i32 {
    guard(-1, || {
        // SAFETY : contrat de l'API.
        let value = unsafe { take_data(value) }?;
        let key = unsafe { string(key, "champ") }?;
        let record = unsafe { borrow_data_mut(record) }.ok_or("enregistrement NULL")?;
        if !matches!(record, Val::Record(_)) {
            return Err(format!(
                "veka_data_set : un enregistrement attendu, trouvé {}",
                record.type_name()
            ));
        }
        *record = record.with(key, value);
        Ok(0)
    })
}

/// Libère une donnée que le C n'a donnée à personne.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_free(d: *mut VekaData) {
    // SAFETY : contrat de l'API.
    drop(unsafe { take_data(d) });
}

/// La nature de `d` (`VEKA_DATA_KIND_NONE` pour NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_kind(d: *const VekaData) -> VekaDataKind {
    // SAFETY : contrat de l'API.
    match unsafe { borrow_data(d) } {
        None | Some(Val::Unit) => VekaDataKind::None,
        Some(Val::Bool(_)) => VekaDataKind::Bool,
        Some(Val::Int(_)) => VekaDataKind::Int,
        Some(Val::Float(_)) => VekaDataKind::Float,
        Some(Val::Str(_)) => VekaDataKind::String,
        Some(Val::Array(_) | Val::Range(..)) => VekaDataKind::Array,
        Some(Val::Record(_)) => VekaDataKind::Record,
        Some(_) => VekaDataKind::Other,
    }
}

/// Un nombre (entier ou non) ; 0 sinon.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_as_float(d: *const VekaData) -> f64 {
    // SAFETY : contrat de l'API.
    unsafe { borrow_data(d) }.and_then(Val::as_f64).unwrap_or(0.0)
}

/// Un entier (un nombre est arrondi vers zéro) ; 0 sinon.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_as_int(d: *const VekaData) -> i64 {
    // SAFETY : contrat de l'API.
    match unsafe { borrow_data(d) } {
        Some(Val::Int(i)) => *i,
        Some(v) => v.as_f64().map_or(0, |f| f as i64),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_as_bool(d: *const VekaData) -> bool {
    // SAFETY : contrat de l'API.
    unsafe { borrow_data(d) }.and_then(Val::as_bool).unwrap_or(false)
}

/// La donnée en texte, copiée dans `buffer` (`size` octets, zéro final compris). Renvoie la
/// longueur complète en octets (sans le zéro), même si elle ne tient pas.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_text(d: *const VekaData, buffer: *mut c_char, size: usize) -> usize {
    // SAFETY : contrat de l'API.
    let text = unsafe { borrow_data(d) }
        .map(ToString::to_string)
        .unwrap_or_default();
    let bytes = text.as_bytes();
    if !buffer.is_null() && size > 0 {
        let n = bytes.len().min(size - 1);
        // SAFETY : `buffer` a `size` octets (contrat de l'API).
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), buffer, n);
            *buffer.add(n) = 0;
        }
    }
    bytes.len()
}

/// Le nombre d'éléments d'un tableau (ou de champs d'un enregistrement) ; 0 sinon.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_len(d: *const VekaData) -> usize {
    // SAFETY : contrat de l'API.
    match unsafe { borrow_data(d) } {
        Some(Val::Array(a)) => a.len(),
        Some(Val::Record(r)) => r.len(),
        Some(v @ Val::Range(..)) => v.items().map_or(0, |i| i.len()),
        _ => 0,
    }
}

/// L'élément `i` d'un tableau, emprunté (valable tant que `d` l'est) ; NULL hors du tableau.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_at(d: *const VekaData, i: usize) -> *const VekaData {
    // SAFETY : contrat de l'API.
    match unsafe { borrow_data(d) } {
        Some(Val::Array(a)) => a.get(i).map_or(std::ptr::null(), lend),
        _ => std::ptr::null(),
    }
}

/// Le champ `key` d'un enregistrement, emprunté (valable tant que `d` l'est) ; NULL s'il manque.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn veka_data_get(d: *const VekaData, key: *const c_char) -> *const VekaData {
    // SAFETY : contrat de l'API.
    let Ok(key) = (unsafe { string(key, "champ") }) else {
        set_error("veka_data_get : champ invalide");
        return std::ptr::null();
    };
    match unsafe { borrow_data(d) } {
        Some(Val::Record(r)) => r
            .iter()
            .find(|(k, _)| &**k == key)
            .map_or(std::ptr::null(), |(_, v)| lend(v)),
        _ => std::ptr::null(),
    }
}
