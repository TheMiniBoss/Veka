# Veka — Étape 1 : plan d'implémentation

> **Statut : proposition, en attente de validation.** Conception : [`VEKA_ETAPE1.md`](VEKA_ETAPE1.md) (les `§` y renvoient).
> Choix appliqués : 1 à 7 (2 = natif), 8 et 9 tels que proposés.

Légende : `[ ]` à faire · **(P)** bloquant pour la suite · 🖥 vérifié sur ton PC / Mac.

---

## Organisation

### Branches et PR

Le dépôt n'a encore aucune branche de base. Proposition :

- créer `main` à partir de la branche de conception (documents seuls) — **à ton accord**, car je ne pousse que sur
  ma branche de travail sans permission ;
- **3 PR** vers `main`, une par jalon, chacune relue et fusionnée par toi :

| PR | Lots | Jalon | Vérifiable où |
|---|---|---|---|
| A | 0 → 5 | **cœur sans fenêtre** : l'exemple compteur produit sa liste de dessin, les clics simulés changent le libellé | ce conteneur (Linux) + CI |
| B | 6 → 9 | **la fenêtre Rust** sur Windows et macOS | CI + 🖥 PC et Mac |
| C | 10 → 11 | **le même exemple en C** | CI + 🖥 PC et Mac |

### Machines

- Ce conteneur (Linux) : tout le cœur, les tests, les instantanés, le contrôle WebAssembly.
- CI GitHub Actions : `windows-latest`, `macos-latest` (Apple Silicon), `ubuntu-latest`.
- 🖥 Ton **PC Windows** et ton **Mac mini M2**, rattachés à cette session (Remote Control depuis l'app Claude
  desktop, dossier du clone de Veka) : je lance `cargo run`, je prends des captures, je corrige. À préparer avant la
  PR B : Rust (`rustup`), sur Windows les *Build Tools* de Visual Studio (C++), sur Mac les *Command Line Tools*
  de Xcode, et CMake sur les deux (pour la PR C).

### Règles de code

- Rust stable, édition 2024, `rust-toolchain.toml` figé ; `cargo fmt`, `cargo clippy -D warnings` en CI.
- `unsafe` seulement dans les crates de plateforme, de rendu et FFI, chaque bloc commenté (`// SAFETY:`).
- Dépendances principales : `windows`, `objc2` (+ `objc2-app-kit`, `objc2-foundation`, `objc2-quartz-core`),
  `wgpu`, `raw-window-handle`, `parley`, `swash`, `bytemuck`, `slotmap`, `cbindgen` (build), `insta` (tests).
  Versions stables du moment, figées dans `Cargo.lock`.

---

## PR A — Cœur sans fenêtre

### Lot 0 — Socle **(P)**
- [ ] `Cargo.toml` de workspace (§4), `rust-toolchain.toml`, `.gitignore`, `rustfmt.toml`, profils `dev` / `release`
- [ ] Crates vides avec leur `lib.rs` et une doc de module : `veka`, `veka-foundation`, `veka-graphics`,
      `veka-text`, `veka-ui`, `veka-platform`, `veka-platform-windows`, `veka-platform-macos`, `veka-render`,
      `veka-render-wgpu`, `veka-app`, `veka-ffi` (les crates de plateforme ne compilent leur code que sur leur OS)
- [ ] Polices : `fonts/LTFlodeNeue/*.otf` (copiées depuis `Veka-Unreal/Resources/Fonts`, lecture seule) + `OFL.txt`
      (**à fournir par toi** : le fichier de licence n'est pas dans Veka-Unreal) ; étape 1 : Regular, Medium, SemiBold, Bold
- [ ] CI `.github/workflows/ci.yml` : fmt, clippy, tests sur les 3 OS
- [ ] **Contrôle de pureté** `xtask check-core` : `cargo tree` des crates du cœur ne contient ni `windows`, ni
      `objc2`, ni `wgpu`, ni `winit` ; `cargo check --target wasm32-unknown-unknown` des crates du cœur
- Fini quand : CI verte sur les 3 OS avec des crates vides.

### Lot 1 — `veka-foundation`
- [ ] `Point`, `Vector`, `Size`, `Rect`, `Insets`, `Transform2D` (`f32`, `#[repr(C)]`), opérations et conversions
- [ ] `Color` (sRGB → linéaire, alpha prémultiplié), couleurs nommées de base (`Blue`, `Orange`, `Green`, `White`…)
- [ ] `Instant` / `Duration` fournis par la plateforme (pas de `std::time::Instant` dans le cœur, à cause du web)
- [ ] Tests : géométrie (union, intersection, inset, transform), couleur (aller-retour sRGB)

### Lot 2 — `veka-graphics`
- [ ] `ShapeSdf` : rect, rounded rect (4 rayons, rayons bornés à la demi-taille), capsule, cercle, ellipse ;
      `distance(point)` en Rust **et** même formule en WGSL (lot 6) ; `contains(point)`
- [ ] `Paint::Solid(Color)` (enum ouverte pour Colora)
- [ ] `DrawList`, `DrawCommand` (§7.1) dont `Backdrop { effect: BackdropEffect::None }` ; `FontTable` ; table des
      glyphes (`Vec<PositionedGlyph>` partagée, les commandes en gardent une plage)
- [ ] `DrawList::segments()` : découpe aux `Backdrop` (§7.3)
- [ ] Sortie texte lisible de la liste (pour les instantanés `insta`)
- [ ] Tests : SDF dedans / dehors / bord pour chaque forme, coins arrondis (le coin n'est pas touché), segments

### Lot 3 — `veka-text`
- [ ] `FontCollection` chargée depuis des octets (polices embarquées via `include_bytes!` dans `veka`, pas d'accès
      disque dans le cœur)
- [ ] `TextStyle` (taille, graisse, couleur), `TextLayout::new(text, style, max_width)` avec Parley : lignes,
      glyphes positionnés, métriques (ascent, descent, hauteur de ligne, largeur)
- [ ] Conversion `TextLayout` → commande `Glyphs`
- [ ] Tests : mesure stable de « Bonjour Veka » ; retour à la ligne quand `max_width` est petit ; accents

### Lot 4 — `veka-ui` : modèle, layout, primitives **(P)**
- [ ] `Element` (primitive | vue | fragment | vide), modifiers, enfants, `.id()`
- [ ] `Node` en arène (`slotmap`), runtime `Ui` : arbre, file des vues à ré-évaluer, contexte courant
      (variable locale au thread, installée pendant `body` et pendant les actions)
- [ ] Vues : fonctions `fn() -> impl View` et trait `View` ; reconciler (type + position, `.id()`) ;
      `State<T>` retrouvé par ordre d'appel, vérification de l'ordre en debug, opérateurs (`+=`…) et `Display`
- [ ] `Environment` (police, taille, couleur du texte, échelle)
- [ ] Trait public `Layout` (`size_that_fits` / `place`) ; `Stack` (axes H / V / Depth, spacing, alignment),
      `Spacer` ; modifiers `padding`, `frame`, `background`, `foreground`
- [ ] Primitives `Text`, `Shape` (`Rectangle`, `RoundedRectangle`, `Capsule`, `Circle`, `Ellipse`, `fill`, `stroke`)
- [ ] Macros `VStack![…]`, `HStack![…]`, `ZStack![…]` (avec `spacing: n;` optionnel)
- [ ] **Schéma** (§5.7) : registre des primitives et modifiers, `Value` (bool, int, float, string, color, size,
      enum), `Element::from_data(nom, params, enfants)` ; test « toute primitive est dans le schéma »
- [ ] `Surface` (§8.2) : `resize`, `frame(now) -> &DrawList`, `needs_frame()` ; ré-évaluation regroupée avant l'image
- [ ] Tests : layout des piles (tailles, espacement, alignement), état conservé après ré-évaluation, nœuds
      réutilisés (pas recréés), instantané de la liste de dessin du compteur, compteur construit en Rust **et** par
      données → même liste

### Lot 5 — `veka-ui` : événements et `Button` **(P)**
- [ ] `InputEvent` (pointeur déplacé / appuyé / relâché / sorti) → `Surface::handle`
- [ ] Hit-test du dessus vers le dessous avec la forme exacte (SDF), remontée vers les parents, capture à l'appui
- [ ] Modifiers `on_tap`, `on_hover` ; états survolé / pressé
- [ ] `Button(titre, action)` : style par défaut (capsule bleue, texte blanc, plus sombre au survol et à l'appui),
      action au relâchement **dans** la forme ; style séparé du comportement
- [ ] Exemple `examples/compteur.rs` (sans fenêtre pour l'instant : la fonction de vue est partagée avec les tests)
- [ ] Tests : 3 clics simulés → « Clics : 3 » dans la liste ; clic dans le coin arrondi = rien ; appui dedans puis
      relâché dehors = rien ; au repos `needs_frame() == false`
- **Fin de la PR A** : tout tourne dans ce conteneur, CI verte, contrôle de pureté et WebAssembly verts.

---

## PR B — La fenêtre Rust (Windows et macOS)

### Lot 6 — `veka-render` + `veka-render-wgpu` **(P)**
- [ ] Trait `Renderer` (§8.3), `RenderTarget` (surface de fenêtre ou texture), texture d'arrière-plan externe optionnelle
- [ ] Initialisation `wgpu` asynchrone (bloquée par `pollster` sur desktop) ; DX12 sur Windows, Metal sur macOS
- [ ] Dessin dans une **texture intermédiaire** puis copie vers la fenêtre (§7.3) ; mesure du coût de la copie
- [ ] Pipeline formes : quads instanciés, SDF en WGSL (mêmes formules que le lot 2), antialiasing par dérivée,
      remplissage et trait, clips (rect + SDF), transforms, calques d'opacité
- [ ] Pipeline texte : rastérisation `swash` → atlas (niveaux de gris), cache par (police, glyphe, taille × échelle)
- [ ] Parcours par segments, `Backdrop::None` sans effet
- [ ] Test de rendu hors écran : l'exemple compteur → PNG, comparé à une référence avec tolérance (CI Windows / Mac)

### Lot 7 — `veka-platform` + `veka-platform-windows` **(P)**
- [ ] Trait `Platform` / `PlatformApp` / `PlatformContext` / `PlatformWindow` / `PlatformEvent` (§8.1) ; `WindowDesc`
      avec `chrome` (seul `System` construit) et les régions de fenêtre (§8.4) déjà dans l'API
- [ ] Win32 : classe et fenêtre, `WndProc`, boucle `GetMessageW`, DPI Per-Monitor V2 (`WM_DPICHANGED`), souris
      (`TrackMouseEvent` pour la sortie), `InvalidateRect` / `WM_PAINT`, fermeture, `raw-window-handle`
- [ ] Horloge (`QueryPerformanceCounter`)
- [ ] 🖥 PC : fenêtre vide qui s'ouvre, se redimensionne, change d'écran 100 % ↔ 150 %

### Lot 8 — `veka-platform-macos` **(P)**
- [ ] AppKit : `NSApplication`, délégué, `NSWindow`, `NSView` à nous avec `CAMetalLayer`, suivi souris
      (`NSTrackingArea`), `backingScaleFactor`, redessin à la demande, fermeture = fin de l'app
- [ ] Horloge (`mach_absolute_time`)
- [ ] 🖥 Mac mini M2 : mêmes vérifications qu'au lot 7, Retina

### Lot 9 — `veka-app` + le compteur en Rust
- [ ] `App::new().window(titre, vue).run()` : choisit la plateforme de l'OS, branche Platform → `Surface` → Renderer,
      redessin seulement si `needs_frame()`
- [ ] `cargo run --example compteur`
- [ ] 🖥 PC et Mac : captures, clics, survol, 0 % CPU au repos (gestionnaire des tâches / Moniteur d'activité)
- **Fin de la PR B.**

---

## PR C — Le même exemple en C

### Lot 10 — `veka-ffi`
- [ ] `cdylib` + `staticlib` ; `veka.h` généré par `cbindgen` et versionné dans `include/` (la CI vérifie qu'il est à jour)
- [ ] Fonctions `veka_*` du §9.2 : app, fenêtre, vue (`setup` + `body`), état entier, texte, formes, `fill`, `frame`,
      `padding`, piles, bouton ; tout passe par le chemin **données** du schéma (§5.7)
- [ ] `catch_unwind` sur chaque fonction, `veka_last_error()` ; propriété « consommé par le parent »
- [ ] `VEKA_EXIT_AFTER_FIRST_FRAME=1` pour la CI
- [ ] `examples/c/compteur.c` + `CMakeLists.txt` (MSVC et clang)
- [ ] CI Windows / macOS : build CMake, lancement « une image »
- [ ] 🖥 PC et Mac : même fenêtre et même comportement que l'exemple Rust

### Lot 11 — Fin d'étape
- [ ] `Docs/CHANGELOG.md` (format de VekaUI : le plus récent en premier)
- [ ] `Docs/TODO.md` : suite (fenêtres `Chrome::Custom`, Colora, Anima, `.vk`…)
- [ ] `README.md` : construire, lancer l'exemple Rust et l'exemple C
- [ ] Captures Windows et macOS dans `Docs/Screens/`
- **Fin de la PR C : étape 1 terminée** (critères d'acceptation du §1).

---

## Risques

| Risque | Parade |
|---|---|
| Le natif macOS (objc2, délégués, `CAMetalLayer`) prend plus de temps que prévu | lot isolé ; le cœur et le rendu sont déjà testés avant ; vérification directe sur ton Mac |
| Écarts de rendu du texte entre Windows et macOS | même police embarquée, même rastériseur (`swash`) partout : pas de rendu du système |
| Copie de la texture intermédiaire trop coûteuse | mesurée au lot 6 ; repli : dessin direct tant qu'il n'y a pas de `Backdrop` |
| Pas de GPU sur les machines de CI | adaptateur logiciel de wgpu (WARP sur Windows) ; sinon test de rendu seulement sur tes machines |
| `State` par ordre d'appel mal utilisé (dans un `if`) | vérification en debug avec message clair |
