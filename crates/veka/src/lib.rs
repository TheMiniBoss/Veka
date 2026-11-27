//! # Veka
//!
//! Bibliothèque d'interface utilisateur pour Windows et macOS, utilisable depuis Rust, C, C++, C#,
//! Unreal et Unity. Organisée en modules, à la manière des frameworks d'Apple :
//!
//! | Module | Contenu |
//! |---|---|
//! | [`foundation`] | géométrie, couleurs, temps |
//! | [`graphics`] | formes, liste de dessin |
//! | [`text`] | polices, mise en page du texte |
//! | [`ui`] | vues, état, layout, contrôles |
//! | [`app`] | fenêtres natives : `App::new().window(titre, vue).run()` |
//!
//! ```
//! use veka::prelude::*;
//!
//! fn compteur() -> impl View {
//!     let mut clics = State::new(0);
//!     VStack![spacing: 16;
//!         Text("Bonjour Veka").font(Font::Title),
//!         Button(format!("Clics : {clics}"), move || clics += 1),
//!     ]
//!     .padding(24)
//! }
//! # let _ = component(compteur);
//! ```

#[cfg(feature = "app")]
pub use veka_app as app;
#[cfg(feature = "app")]
pub use veka_app::App;
pub use veka_foundation as foundation;
pub use veka_graphics as graphics;
pub use veka_text as text;
pub use veka_ui as ui;

pub use veka_ui::{HStack, VStack, ZStack};

/// `use veka::prelude::*;` : tout ce qu'il faut pour écrire des vues.
pub mod prelude {
    #[cfg(feature = "app")]
    pub use veka_app::App;
    pub use veka_ui::prelude::*;
}
