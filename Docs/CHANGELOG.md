# Changelog

Le plus récent en premier.

## Étape 1 — 27/11/2026

### Interface C (PR C)
- `veka-ffi` : `veka.dll` / `libveka.dylib`, en-tête `veka.h` généré par cbindgen (`cargo xtask header`, vérifié par la CI).
- Fonctions `veka_*` : application et fenêtres, états (`int`, `number`, `bool`), texte, formes, piles, bouton,
  modifiers ; `veka_element` / `veka_modifier` donnent accès à tout le schéma par des données.
- Aucune panique ne traverse la frontière ; `veka_last_error()`.
- `examples/c/compteur.c` avec CMake : essayé sur Windows (MSVC, `/W4` sans avertissement).

### Fenêtres natives et rendu GPU (PR B)
- Renderer wgpu : formes en SDF analytique, texte rastérisé par swash dans un atlas, texture intermédiaire.
- Plateforme Windows (Win32) : DPI par écran, souris, redessin à la demande, 0 % CPU au repos.
- Plateforme macOS (AppKit) : écrite et compilée, **pas encore essayée** sur un Mac.
- `App::new().window(titre, vue).run()`.

### Cœur (PR A)
- Géométrie, couleurs, formes, liste de dessin, texte (Parley + police LT Flode Neue).
- Vues façon SwiftUI : piles, texte, formes, bouton, modifiers, `State<T>`, layout par proposition de taille.
- Schéma : chaque composant et modifier constructible par des données.
