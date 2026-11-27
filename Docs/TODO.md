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
- [x] PR I : timelines, `matched_geometry_effect`, `.on_animation_end`, interface C et exemple C, schéma,
  « réduire les animations » de l'OS.
- [ ] macOS : relire « Réduire les animations » dès qu'il change (aujourd'hui : quand la fenêtre redevient
  active).
- [ ] `matched_geometry_effect` : fondu croisé entre la source et la cible, espaces de noms ; aujourd'hui la
  vue liée doit être la racine de ce qui apparaît (sinon la transition de la racine s'ajoute).
- [ ] `.id()` dans le schéma (le C a `veka_id`, qui ne passe pas par le schéma).
- [ ] Pointeur : suivre `offset`, `scale_effect` et `rotation_effect` (aujourd'hui : zone du layout).
- [ ] Couleur du texte animée sans refaire son layout.
- [ ] Chiffres qui défilent (`content_transition`), morphing de formes.

## Étape 5 (le langage `.vk`)
- [x] PR J : parseur, imprimeur, interpréteur, `App::window_vk`, rechargement à chaud, exemples.
- [x] PR K : `@Param`, `@Slot`, `@Event` / `.on`, `import`, `model`, fonctions, maths, `random`, `@Clock`,
  timelines dans le fichier, interface C, feux d'artifice en `.vk`.
- [ ] `@Clock` : n'avancer que si la vue lit l'horloge (aujourd'hui : à chaque image).
- [ ] Plusieurs `@Slot` remplis par fermetures nommées (aujourd'hui : le premier par `{ … }`, les autres par
  `nom: Vue(…)`).
- [ ] Timelines et horloges retrouvées par nom au rechargement (aujourd'hui : relancées si leur liste change).
- [ ] Rechargement : surveiller le dossier (événements du système) plutôt que la date toutes les 250 ms.

## Plus tard
- [ ] Effets (`Backdrop`, verre).
- [ ] Langage `.vk` : tout faire sans autre langage (maths, hasard, temps…).
- [ ] Liaisons C++, C#, Unreal, Unity sur `veka.h`.
