//! Les états : des valeurs gardées par Veka pour une vue, comme `State<T>` en Rust.
//!
//! Les `veka_state_new_*` se font dans la fonction `setup` d'une fenêtre (ou dans `body`), toujours
//! dans le même ordre : Veka retrouve les états par ordre d'appel. `setup` est rappelée avant chaque
//! `body`, et renvoie alors les mêmes états.

use veka_ui::State;

use crate::{VekaContext, guard};

/// Un état. Opaque : c'est une poignée, pas un pointeur à libérer.
pub struct VekaState {
    _private: u8,
}

fn handle<T>(state: State<T>) -> *mut VekaState {
    state.to_bits() as usize as *mut VekaState
}

fn state<T>(s: *mut VekaState) -> Result<State<T>, String> {
    if s.is_null() {
        return Err("état NULL".into());
    }
    Ok(State::from_bits(s as usize as u64))
}

/// Crée un état entier (la première fois) ou retrouve le même (les fois suivantes).
#[unsafe(no_mangle)]
pub extern "C" fn veka_state_new_int(_cx: *mut VekaContext, initial: i64) -> *mut VekaState {
    guard(std::ptr::null_mut(), || Ok(handle(State::new(initial))))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_get_int(_cx: *mut VekaContext, s: *mut VekaState) -> i64 {
    guard(0, || Ok(state::<i64>(s)?.get()))
}

/// Change la valeur : la vue sera reconstruite avant la prochaine image.
#[unsafe(no_mangle)]
pub extern "C" fn veka_state_set_int(_cx: *mut VekaContext, s: *mut VekaState, value: i64) {
    guard((), || {
        state::<i64>(s)?.set(value);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_new_number(_cx: *mut VekaContext, initial: f64) -> *mut VekaState {
    guard(std::ptr::null_mut(), || Ok(handle(State::new(initial))))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_get_number(_cx: *mut VekaContext, s: *mut VekaState) -> f64 {
    guard(0.0, || Ok(state::<f64>(s)?.get()))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_set_number(_cx: *mut VekaContext, s: *mut VekaState, value: f64) {
    guard((), || {
        state::<f64>(s)?.set(value);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_new_bool(_cx: *mut VekaContext, initial: bool) -> *mut VekaState {
    guard(std::ptr::null_mut(), || Ok(handle(State::new(initial))))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_get_bool(_cx: *mut VekaContext, s: *mut VekaState) -> bool {
    guard(false, || Ok(state::<bool>(s)?.get()))
}

#[unsafe(no_mangle)]
pub extern "C" fn veka_state_set_bool(_cx: *mut VekaContext, s: *mut VekaState, value: bool) {
    guard((), || {
        state::<bool>(s)?.set(value);
        Ok(())
    })
}
