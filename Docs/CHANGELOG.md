# Changelog

Le plus récent en premier.

## Étape 9 — clavier, focus et saisie de texte

### Clavier et focus (PR T)
- Touches et caractères tapés sur Windows (`WM_KEYDOWN`, `WM_CHAR`) et macOS (`keyDown:`), avec les
  modificateurs ; `Modifiers::COMMAND` vaut ⌘ sur le Mac et Ctrl ailleurs.
- Focus : `.focusable()`, un clic le donne, Tab et Maj+Tab le passent, anneau à la couleur d'accent.
  `.focused(état)` et `.focused_equals(état, valeur)` le relient à un état, dans les deux sens.
- `.on_key_press(touche, …)` : la touche remonte de la vue qui a le focus jusqu'à la racine.
  `.keyboard_shortcut(touche, modificateurs)` déclenche un bouton de n'importe où.
- `.vk` : `$nom` passe un `@State` à une vue qui l'écrit ; `.focusable()`, `.focused($focus, equals: …)`,
  `.onKeyPress(.return) { … }`, `.keyboardShortcut("r", modifiers: .command)`, `.keyboardShortcut(.cancelAction)`.
- Exemple `clavier` en Rust et en `.vk` (même dessin, testé après les mêmes touches).

## Correction des modes de fusion (PR S)
- Comme SwiftUI, un `.blend_mode` se mélange avec tout ce que l'interface a dessiné derrière la vue, mais plus
  avec le fond de la fenêtre, qui ne fait pas partie du contenu. Avant, un cercle en `multiply` devenait
  presque noir sur un fond de fenêtre sombre. Le contenu passe dans un calque transparent seulement s'il
  contient un mode de fusion.

## Petits restes (PR R)
- Fenêtre transparente : les clics passent à travers la marge où Veka ne dessine rien, vers la fenêtre de
  dessous (`DrawList::painted_bounds`, `PlatformWindow::set_hit_area`).
- `.preferred_color_scheme(Some(ColorScheme::Dark))` (`.preferredColorScheme(.dark)`) : l'app impose son mode,
  la barre de titre suit. `colora` s'en sert pour son bouton clair / sombre.
- Hiérarchie : les jetons translucides (`secondary`…) ont le contraste d'un mélange en sRGB, comme chez Apple.
- Verre liquide : le bord sombre est décalé vers l'intérieur et se voit ; `liquid_distortion` avance de nouveau
  en continu (la courbe linéaire est portée par la première clé de la timeline).

## Étape 8 — modes de fusion

### Interface C (PR Q)
- `VekaBlendMode` (21 modes, `VEKA_BLEND_MODE_MULTIPLY`…), `veka_blend_mode` et `veka_compositing_group`.
  Le C++ les prend tels quels dans `veka.h`.
- Exemple C `fusion` (même grille que l'exemple Rust), construit et lancé par la CI sur Windows.

### Calques et modes (PR P)
- `.blend_mode(BlendMode::Multiply)` (`.blendMode(.multiply)` en `.vk`) : la vue est dessinée dans un calque à
  part, puis mélangée d'un bloc avec ce qui est derrière. Les 21 modes de SwiftUI (formules W3C), mélangés en
  sRGB comme SwiftUI et Figma ; normal et découpes (`sourceAtop`, `destinationOver`, `destinationOut`) restent
  en lumière linéaire.
- `.compositing_group()` (`.compositingGroup()`) : le même calque, sans mélange. Une opacité appliquée ensuite
  rend le groupe transparent d'un bloc, et les modes ou les verres à l'intérieur ne lisent que le groupe.
- Liste de dessin : `PushGroup { blend, opacity }` / `PopGroup`. Formules de référence côté processeur :
  `veka_graphics::composite`, comparées au GPU par les tests.
- Renderer : calques à la taille de la fenêtre (un par niveau d'imbrication, 8 au plus), zone composée limitée
  à ce que le groupe a dessiné. Un groupe normal, opaque et sans rien à isoler ne coûte rien.
- Exemple `fusion` en Rust et en `.vk` (même dessin, testé) : les 21 modes et l'opacité d'un groupe.

## Essais sur le Mac mini

- macOS : les animations (Anima, timelines, `@Clock`, feux d'artifice) s'arrêtaient après la première image,
  car AppKit oublie un `setNeedsDisplay` demandé pendant `drawRect:`. Les images sont maintenant cadencées par
  le `CADisplayLink` de la vue (macOS 14+, mis en pause quand rien ne bouge : toujours 0 % de CPU au repos).
- macOS : menu d'application (Masquer ⌘H, Masquer les autres ⌥⌘H, Quitter ⌘Q ; Fenêtre : Réduire ⌘M, Zoom,
  Plein écran ⌃⌘F, Fermer ⌘W). Quitter (menu, Dock) demande la fermeture de chaque fenêtre, comme sur Windows.
- macOS : « Réduire les animations » et la couleur d'accent sont relus dès qu'ils changent dans les Réglages.
- Barre de titre au style Mac : le feu vert passe en plein écran (convention de macOS) et chaque feu montre son
  symbole au survol (×, −, flèches). Le menu porte le nom du paquet `.app` quand il y en a un.
- Exemple `fenetre_transparente` : la carte prend le fond de fenêtre du thème (le blanc fixe rendait le texte
  illisible en mode sombre).

## Étape 7 — un `.vk` rapide

### Interpréteur avec caches (PR O)
- Chaque nœud de l'arbre garde ce que l'interpréteur apprend à sa première évaluation : valeur d'une
  expression constante (`2 * 3`, `.white`, `[.clear, .white]`), place d'une variable locale, fonction ou
  fonction de base appelée, entrée du schéma d'une vue ou d'un modifier. L'arbre est refait au rechargement :
  les caches repartent vides.
- Fonctions pures mémorisées par site d'appel (au plus 4 096 résultats, arguments de base seulement) ; une
  fonction qui lit `model`, elle ou celles qu'elle appelle, ne l'est pas.
- Moins d'allocations : arguments sur la pile, une seule liste pour un `ForEach`, noms du schéma sans copie
  (`Params::with_static`, `Schema::build_entry`), enfants parcourus sans copie dans le layout, Anima, la
  réconciliation et l'environnement. Les états et paramètres utilisent un hachage rapide.
- Feux d'artifice : −31 % d'instructions par image (release).

### Gains simples (PR N)
- Le cœur de Veka (`veka-foundation`, `veka-graphics`, `veka-colora`, `veka-anima`, `veka-text`, `veka-ui`,
  `veka-lang`) est compilé en `opt-level = 1` même en debug : les feux d'artifice passent de 31 à 6,5 ms par
  image (720 × 480, hors GPU). Une application qui dépend de Veka peut copier ces lignes de `Cargo.toml`.
- Interface : sans animation demandée ni en cours, plus de comparaison des valeurs animables après le layout ; le
  relevé des zones de la fenêtre n'alloue plus.
- Interpréteur : un appel de fonction ne crée plus de contexte neuf, les noms des `let`, paramètres et
  fermetures sont partagés (`Rc<str>` dans l'AST). Feux en release : de 6,3 à 5,3 ms par image.
- Mesure : `cargo test -p veka --release --test mesure -- --ignored --nocapture` (feux, anima, verre), affichée
  par la CI sans seuil ; `VEKA_STATS=1` affiche chaque seconde les images par seconde et le temps d'une image
  (interface, puis rendu et présentation).

## Étape 6 — Liquid Glass

### Verre liquide (PR M)
- `.glass()` et `.glass_with(Glass::regular() | Glass::clear())`, avec `.clarity(0…1)`, `.tint(…)`,
  `.interactive(true)`, `.shape(…)`, réfraction, bord, séparation des couleurs, reflet et ombre : l'aspect de
  Liquid Glass 27 (bord assombri à l'intérieur, reflet vif tourné vers la lumière, lumière interne plus claire en
  haut, ombre douce). Un verre interactif est éclairé par le pointeur ; l'image n'est redessinée que si le
  pointeur bouge au-dessus d'un tel verre.
- `.glass_group(douceur)` : un seul verre pour tous les `glass` du sous-arbre, dont les formes fusionnent comme
  des gouttes (minimum lisse des SDF, jusqu'à 8 formes) et suivent leurs transformations (`offset`, animations).
- `.liquid_distortion(force, taille, vitesse, forme)` : le fond ondule dans la forme (eau, chaleur).
- Fonds du système : `Window::backdrop(SystemBackdrop::Mica | MicaAlt | Acrylic)` (Mica, Mica Alt et Acrylic sur
  Windows 11 ; fond de fenêtre, barre latérale et HUD sur macOS). Veka se dessine par-dessus, sur un fond
  transparent ; un verre posé sur ce fond garde sa lumière sans noircir.
- `.vk` : `.glass(clarity:, interactive:, shape:, cornerRadius:, …)`, `.glassGroup(smoothness:)`,
  `.liquidDistortion(…)` ; C : `VekaGlass`, `veka_glass_regular`, `veka_glass_clear`, `veka_glass`,
  `veka_glass_group`, `veka_liquid_distortion`, et `VekaWindowDesc.backdrop`.
- Exemple `verre` complet en Rust, en `.vk` (`verre.vk`, même dessin, testé) et en C (`examples/c/verre.c`) ;
  `cargo run --example verre -- mica` le montre sur le matériau du système.
- « Réduire la transparence » et la qualité `Off` : chaque verre (et chaque forme d'un groupe) devient une
  teinte unie.
- `BackdropEffect::Glass` est en boîte (`Box<GlassEffect>`) pour ne pas alourdir les autres commandes.

### Flou d'arrière-plan (PR L)
- `.backdrop_blur(rayon)`, `.frosted_glass()` et `.backdrop(BackdropBlur { … })` : ce qui est dessiné derrière
  la vue est flouté, saturé, teinté (jeton Colora) et granuleux, découpé à sa forme (rectangle, rectangle arrondi,
  capsule, cercle, ellipse). Tout est animable. Dans le `.vk` : `.backdropBlur(16, shape: .capsule)`,
  `.frostedGlass()` ; en C : `veka_backdrop_blur`, `veka_frosted_glass`, `veka_backdrop`.
- Renderer wgpu : une capture copie seulement la zone utile, puis une pyramide (réduction 2×2 et flou gaussien
  par niveau) donne n'importe quel rayon en mélangeant deux niveaux. Les effets partagent une capture tant que
  rien n'a été dessiné sur leur zone depuis ; un verre posé sur un autre le voit. Sans effet, rien ne change
  (pas de copie ni de passe en plus).
- Repli : `App::backdrop_quality(BackdropQuality::Off | Low | High)` (`veka_app_set_backdrop_quality`), et
  « Réduire la transparence » de macOS / effets de transparence désactivés sur Windows : teinte unie et opaque.
- Exemple `verre` (`cargo run --example verre`).

## Étape 5 — le langage `.vk`

### Composants, données de l'hôte, logique, interface C (PR K)
- Composants : `@Param var nom: Type = défaut`, `@Slot nom` et `Slot(nom)`, `@Event nom(Type)`, `emit nom(valeur)`
  et `.on(nom) { valeur in … }` ; une vue du fichier s'utilise comme un composant (`Article(nom: a.nom) { … }`).
- Données de l'hôte : `model` dans le fichier, `Program::set_model(Val)` (les vues affichées suivent),
  `Program::on_event` reçoit les `emit` que personne n'écoute ; `Val` (nombres, textes, tableaux,
  enregistrements) est public.
- Expressions : enregistrements `{ x: 1, y: 2 }`, indices `a[i]`, `func` pures au niveau du fichier, maths
  (`sin`, `cos`, `atan2`, `sqrt`, `pow`, `clamp`, `lerp`, `floor`, `round`, `pi`…), `random(graine)` et
  `random(graine, min, max)` déterministes, `format("%.1f", x)`, `len`.
- Temps : `@Clock var t` ; `timeline Nom(loop:, length:) { track … ; event … }`, `@Timeline var p = Nom(autoplay:)`,
  `p.piste`, `p.play()` / `pause` / `seek` / `reverse` / `toggle`, `.onTimelineEvent(p, événement) { … }`.
- `import "fichier.vk"` (le cœur reçoit un lecteur de l'hôte) ; veka-app relit aussi les imports à chaud.
- Interface C : `veka_vk_load`, `veka_vk_load_file`, `veka_vk_reload`, `veka_vk_view`, `veka_vk_set_model`,
  `veka_vk_on_event`, `veka_app_add_window_vk`, et `VekaData` (`veka_data_*`) pour les données. Exemple
  `examples/c/boutique.c` + `boutique.vk`.
- `anima.vk` gagne sa pulsation (même dessin que `anima.rs`) ; `feux.vk` : des feux d'artifice écrits seulement
  en `.vk` (`cargo run --example vk -- feux`).
- Correction : `rgb(…)` et les fonctions de base pouvaient paniquer (dégradé évalué trop tôt).

### Parseur, interpréteur, rechargement à chaud (PR J)
- Nouveau crate `veka-lang` (cœur, compile pour WebAssembly) : lexer, parseur avec reprise sur erreur, AST,
  imprimeur canonique (relire le texte imprimé redonne le même arbre), analyse au chargement (vues, modifiers et
  paramètres inconnus, avec « vouliez-vous dire »).
- Interpréteur : `view`, `@State var nom: Type = valeur` (retrouvés par nom : le rechargement les garde),
  expressions (arithmétique, comparaisons, ternaire, plages, chaînes `\(…)`, couleurs `#RRGGBB[AA]`),
  `if` / `else`, `let`, `ForEach(tableau ou plage) { x, i in … }`, actions (`=`, `+=`…, `toggle`, `append`,
  `insert(at:)`, `remove(at:)`, `removeLast`…), `withAnimation(.snappy) { … }`, les cinq dégradés, animations et
  transitions, jetons Colora, `theme Nom: Parent { Jeton: clair / sombre }` dans le fichier.
- `Program::new(source)`, `.view("Nom")`, `.reload(source)` (un fichier faux laisse l'ancien en place),
  `.errors()` ; une vue fausse affiche son erreur (`ligne:colonne : message`).
- veka-app : `App::window_vk(chemin, "Vue")`, rechargement à chaud (actif en debug, `App::hot_reload(false)`),
  bandeau rouge des erreurs ; plateformes : `PlatformEvent::Wake` et `PlatformContext::waker()` (Windows :
  message de fil, macOS : file principale de libdispatch).
- veka-ui : `component_tagged` (deux vues `.vk` n'ont pas les mêmes états), `colorScheme(.system)`.
- Exemples `examples/vk/compteur.vk`, `colora.vk`, `anima.vk` (`cargo run --example vk -- colora`) : même liste
  de dessin que les exemples Rust, testé image par image.

## Étape 4 — Anima

### Timelines, géométrie liée, interface C (PR I)
- Timelines : `Timeline` (pistes à images clés `Keyframe`, événements nommés, durée, boucle, vitesse) et
  `TimelineState` dans une vue (`playing`, `play`, `pause`, `stop`, `seek`, `reverse`, `value`, `on_event`).
  La surface les fait avancer à chaque image ; au repos, plus aucune image.
- `.matched_geometry_effect(id)` : une vue qui apparaît part de la place et de la taille de celle, de même
  `id`, qui disparaît dans la même animation.
- `.on_animation_end(|| ..)` : appelé quand les animations d'un sous-arbre sont finies.
- « Réduire les animations » de Windows (effets d'animation) et de macOS (Accessibilité) : le layout et les
  déplacements ne s'animent plus, les transitions gardent leur fondu ; `.ignore_reduce_motion()` pour s'en
  passer ; `Surface::set_reduce_motion`.
- Schéma : `opacity`, `offset`, `scaleEffect`, `rotationEffect`, `animation`, `transition`,
  `matchedGeometryEffect`, `ignoreReduceMotion`, `onAnimationEnd` ; valeurs `Animation` et `Transition`.
- Interface C : `VekaAnimation` (par valeur) et ses constructeurs, `veka_with_animation`, `veka_animation`,
  `VekaTransition` et `veka_transition`, `veka_opacity` / `offset` / `scale_effect` / `rotation_effect`,
  `veka_matched_geometry_effect`, `veka_on_animation_end`, `veka_ignore_reduce_motion`, `veka_id`, les timelines
  (`VekaTimeline`, `veka_state_new_timeline`, `veka_timeline_*`). Exemple `examples/c/anima.c`.
- L'exemple `anima` gagne une pastille à géométrie liée et une pulsation en timeline.
- Schéma : `background` prend une forme (`shape` : rectangle, capsule, cercle, ellipse).
- Une image d'avance au lieu de deux dans la chaîne d'affichage : un clic se voit plus tôt.

### Animations implicites et transitions (PR H)
- Nouveau crate `veka-anima` (cœur) : `Animation` (linéaire, *ease*, circulaires, Bézier cubique, ressorts
  analytiques, préréglages `smooth` / `snappy` / `bouncy`, délai, vitesse, répétition, aller-retour),
  `Animator` (un ressort interrompu garde sa vitesse), `Transition` (fondu, échelle, glissement, poussée,
  décalage, combinées, asymétriques).
- `with_animation(spec, || ..)` et `.animation(spec, valeur)` : layout, couleurs (en Oklab), dégradés, rayons,
  réglages de couleur, opacité, décalage, échelle et rotation s'animent ; les vues entrent et sortent avec
  `.transition()`.
- Nouveaux modifiers `.opacity()`, `.offset()`, `.scale_effect()`, `.rotation_effect()`.
- `Surface::frame_at(heure)` : le temps vient de l'hôte ; veka-app redessine seulement pendant une animation.
- `.id()` passe aux modifiers qui l'enveloppent (une ligne identifiée garde son identité avec `.transition()`).
- Exemple `anima`, tests image par image.

## Étape 3 — Colora

### Interface C, schéma et macOS (PR G)
- Schéma : `fill`, `stroke`, `foregroundColor`, `tint` et `background` prennent un paramètre `paint` (couleur,
  jeton ou dégradé) ; nouveaux modifiers `theme`, `colorScheme`, `brightness`, `contrast`, `saturation`,
  `hueRotation`, `grayscale`.
- C : `VekaPaint` (`veka_solid`, `veka_token`, les cinq dégradés, `veka_paint_stops` pour des arrêts en jetons,
  `veka_paint_space`), `veka_fill_paint` et compagnie, `VekaTheme` (`veka_theme_new`, `veka_theme_set_color`,
  `veka_theme_set_metric`), `veka_theme`, `veka_color_scheme` et les réglages. Exemple `examples/c/colora.c`.
- macOS : mode clair / sombre (`effectiveAppearance`, suivi en direct) et couleur d'accent du système.
- Exemple `colora` : légendes plus lisibles.

### Couleurs, dégradés, thèmes (PR F)
- Nouveau crate `veka-colora` : espaces sRGB, linéaire, **Oklab**, OkLCH ; les 26 jetons d'Alpha 13 et les jetons
  de contrôle (`ButtonAccent → ThemeAccent → Tint`) ; hiérarchie (60 %, 30 %, 18 %) ; `Theme` avec parent ;
  réglages (luminosité, contraste, saturation, teinte, niveaux de gris).
- `Paint` partout où l'on donne une couleur : couleur, jeton ou dégradé linéaire, radial, elliptique, angulaire,
  maillé. Les jetons sont résolus avant le rendu.
- Renderer : dégradés précalculés dans un atlas de rampes (demi-flottants), vérifiés pixel par pixel hors écran.
- Windows : mode clair / sombre et couleur d'accent du système, suivis en direct ; barre de titre du système
  sombre en mode sombre.
- Piles : chaque enfant reçoit d'abord sa taille idéale quand elle tient (plus de bouton qui passe à la ligne).
- Exemple `colora`.

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
