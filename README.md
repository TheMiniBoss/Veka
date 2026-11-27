# Veka

Bibliothèque d’interface utilisateur en Rust pour Windows et macOS, utilisable depuis Rust, C, C++, C#, Unreal et Unity.

État : **étape 1** (une fenêtre avec du texte, des formes, des piles et un bouton, en Rust et en C).
Conception : [`Docs/VEKA_ETAPE1.md`](Docs/VEKA_ETAPE1.md). Suite : [`Docs/TODO.md`](Docs/TODO.md).

## En Rust

```rust
use veka::prelude::*;

fn compteur() -> impl View {
    let mut clics = State::new(0);

    VStack![spacing: 16;
        Text("Bonjour Veka").font(Font::Title),
        HStack![spacing: 12;
            RoundedRectangle(12).fill(Color::Blue).frame(80, 48),
            Capsule().fill(Color::Orange).frame(96, 48),
            Circle().fill(Color::Green).frame(48, 48),
        ],
        Button(format!("Clics : {clics}"), move || clics += 1),
    ]
    .padding(24)
}

fn main() {
    App::new().window("Compteur", compteur).run().unwrap();
}
```

```sh
cargo run --example compteur              # la fenêtre
cargo run --example compteur_sans_fenetre # sans fenêtre : simule 3 clics
```

## En C

L’en-tête est [`crates/veka-ffi/include/veka.h`](crates/veka-ffi/include/veka.h), l’exemple
[`examples/c/compteur.c`](examples/c/compteur.c). CMake construit la bibliothèque (`cargo build -p veka-ffi`)
puis l’exemple :

```sh
cmake -S examples/c -B build/c
cmake --build build/c
build/c/compteur                 # Windows : build\c\Debug\compteur.exe
```

## Prérequis

- Rust : la version est fixée par `rust-toolchain.toml` (rustup l’installe tout seul).
- Windows : outils C++ de Visual Studio. macOS : Xcode Command Line Tools.
- Pour l’exemple C : CMake 3.20 ou plus.

## Organisation

| Crate | Rôle |
|---|---|
| `veka` | façade : `use veka::prelude::*` |
| `veka-foundation`, `veka-graphics`, `veka-text`, `veka-ui` | le cœur, sans aucun appel au système (compile pour WebAssembly) |
| `veka-render`, `veka-render-wgpu` | rendu GPU (DirectX 12, Metal) |
| `veka-platform`, `veka-platform-windows`, `veka-platform-macos` | fenêtres natives (Win32, AppKit) |
| `veka-app` | l’hôte : plateforme → vue → rendu |
| `veka-ffi` | l’interface C (`veka.dll`, `libveka.dylib`, `veka.h`) |

Tâches du dépôt : `cargo xtask check-core` (cœur sans OS) et `cargo xtask header` (régénère `veka.h`).
