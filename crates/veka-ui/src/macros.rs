//! Les piles : `VStack![…]`, `HStack![…]`, `ZStack![…]`, avec `spacing: n;` en option.
//!
//! ```
//! # use veka_ui::prelude::*;
//! let e = VStack![spacing: 12;
//!     Text("Un"),
//!     HStack![Text("Deux"), Spacer(), Text("Trois")],
//! ];
//! ```

#[doc(hidden)]
#[macro_export]
macro_rules! __veka_stack {
    ($axis:ident; alignment: $a:expr, spacing: $s:expr; $($child:expr),* $(,)?) => {
        $crate::views::stack_aligned(
            $crate::Axis::$axis,
            $a,
            ::core::option::Option::Some(($s) as f32),
            ::std::vec![$($crate::View::into_element($child)),*],
        )
    };
    ($axis:ident; alignment: $a:expr; $($child:expr),* $(,)?) => {
        $crate::views::stack_aligned(
            $crate::Axis::$axis,
            $a,
            ::core::option::Option::None,
            ::std::vec![$($crate::View::into_element($child)),*],
        )
    };
    ($axis:ident; spacing: $s:expr; $($child:expr),* $(,)?) => {
        $crate::views::stack(
            $crate::Axis::$axis,
            ::core::option::Option::Some(($s) as f32),
            ::std::vec![$($crate::View::into_element($child)),*],
        )
    };
    ($axis:ident; $($child:expr),* $(,)?) => {
        $crate::views::stack(
            $crate::Axis::$axis,
            ::core::option::Option::None,
            ::std::vec![$($crate::View::into_element($child)),*],
        )
    };
}

/// Pile verticale.
#[macro_export]
macro_rules! VStack {
    ($($t:tt)*) => { $crate::__veka_stack!(Vertical; $($t)*) };
}

/// Pile horizontale.
#[macro_export]
macro_rules! HStack {
    ($($t:tt)*) => { $crate::__veka_stack!(Horizontal; $($t)*) };
}

/// Enfants superposés.
#[macro_export]
macro_rules! ZStack {
    ($($t:tt)*) => { $crate::__veka_stack!(Depth; $($t)*) };
}
