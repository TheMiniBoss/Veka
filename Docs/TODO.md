# TODO

## Pour finir l’étape 1
- [ ] Essayer la plateforme macOS sur le Mac mini M2 (fenêtre, Retina, clics, 0 % CPU au repos) et l’exemple C avec clang.
- [ ] Captures Windows et macOS dans `Docs/Screens/`.
- [ ] Menu d’application minimal sur macOS (Quitter, ⌘Q).
- [ ] Ajouter `OFL.txt` dans `fonts/LTFlodeNeue/` (licence de la police).
- [ ] Choisir la licence de Veka.

## Juste après l’étape 1
- [x] Fenêtres `Chrome::Hidden` et `Chrome::Custom`, avec Snap Layouts sur Windows 11 (étape 2, essayé sur le PC).
- [ ] Étape 2 sur le Mac : `Hidden`, `Custom`, transparence, plein écran.
- [ ] Fenêtre transparente : laisser passer les clics là où Veka ne dessine rien (aujourd’hui la marge transparente
  capte la souris).
- [ ] Clavier et focus, saisie de texte (IME).
- [ ] Un seul GPU partagé entre les fenêtres.

## Étape 3 (Colora)
- [x] PR G : `VekaPaint`, dégradés et thèmes en C ; peintures dans le schéma ; clair / sombre et accent de macOS.
- [ ] Colora sur le Mac : mode clair / sombre en direct, accent, exemples Rust et C.
- [ ] Constantes `VEKA_UNIT_*` de `veka.h` : littéraux composés du C99, à revoir pour les liaisons C++.
- [ ] Hiérarchie (`secondary` 60 %…) : le mélange alpha se fait en lumière linéaire, donc un noir à 60 % sur fond clair paraît bien plus pâle que chez Apple (mélange en sRGB) ; à corriger (mélange en sRGB pour le texte, ou alphas compensés).
- [ ] Texte en dégradé (aujourd'hui : couleur moyenne du dégradé).
- [ ] Modes de fusion (avec les effets) ; images comme remplissage (avec la vue `Image`).

## Étape 4 (Anima)
- [ ] PR I : timelines, `matched_geometry_effect`, `.on_animation_end`, interface C et exemple C, schéma,
  « réduire les animations » de l'OS.
- [ ] Pointeur : suivre `offset`, `scale_effect` et `rotation_effect` (aujourd'hui : zone du layout).
- [ ] Couleur du texte animée sans refaire son layout.
- [ ] Chiffres qui défilent (`content_transition`), morphing de formes.

## Plus tard
- [ ] Effets (`Backdrop`, verre).
- [ ] Langage `.vk` : tout faire sans autre langage (maths, hasard, temps…).
- [ ] Liaisons C++, C#, Unreal, Unity sur `veka.h`.
