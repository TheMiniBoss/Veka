# Changelog

Le plus récent en premier.

## Étape 2 — fenêtres personnalisées

### Transparence, macOS et interface C (PR E)
- `Chrome::transparent()` : sur Windows, présentation par DirectComposition (`WS_EX_NOREDIRECTIONBITMAP`, alpha
  prémultiplié) ; sur macOS, fenêtre non opaque. Exemple `fenetre_transparente`.
- macOS : `Hidden` (barre transparente, feux gardés) et `Custom` (feux masqués), déplacement par
  `performWindowDragWithEvent`, double-clic selon le réglage du système, état agrandi / plein écran / actif.
  Écrit et compilé, **pas encore essayé** sur un Mac.
- C : `VekaWindowDesc`, `veka_app_add_window_ex`, `veka_window_drag_area`, `veka_window_control`,
  `veka_window_button`, `veka_title_bar` ; `examples/c/fenetre_perso.c`.

### Windows et cœur (PR D)
- `.window_drag_area()`, `.window_control(…)`, `TitleBar`, `WindowButton`, état de la fenêtre dans l'environnement,
  `WindowAction::…perform()`.
- Windows : barre et bordure supprimées, bords de redimensionnement, Snap Layouts sur le bouton agrandir, ombre et
  coins de DWM. Exemple `fenetre_perso`.

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
