# Veka : étape 3, Colora

Statut : **validé, en cours** (PR F). Fait suite à `VEKA_ETAPE2.md`. Reprend la conception de Colora dans VekaUI
(Alpha 13 puis C++ phases 2 à 4), pas son code.

## 1. Objectif

Colora, c'est **tout ce qui colore** une vue Veka :

- des **couleurs sémantiques** (les « jetons » : `Accent`, `Foreground`, `Separator`…) qui changent avec le thème
  et avec le mode clair / sombre ;
- des **dégradés** : linéaire, radial, elliptique, angulaire et maillé (*mesh*), interpolés dans un espace
  perceptuel (Oklab) pour éviter les milieux boueux ;
- des **thèmes** qui ne redéfinissent que ce qui change, appliqués à n'importe quelle partie de l'arbre ;
- des **réglages de couleur** sur un sous-arbre : luminosité, contraste, saturation, teinte, niveaux de gris.

**Critères d'acceptation**

1. Un exemple `colora` (Rust et C) : nuancier des jetons, les cinq dégradés, un thème maison sur une partie de la
   fenêtre, un bouton clair / sombre.
2. Le mode clair / sombre **suit l'OS en direct** sur Windows (et sur macOS quand le Mac sera accessible).
3. Les dégradés sont identiques au pixel près entre le rendu hors écran et la fenêtre ; tests d'instantanés.
4. Le cœur reste sans appel système (`check-core`) ; 0 % CPU au repos.

## 2. Ce que l'utilisateur écrit

### 2.1 Rust

```rust
use veka::prelude::*;

fn vue() -> impl View {
    VStack![spacing: 12;
        Text("Titre").foreground(Token::Foreground),
        Text("Sous-titre").foreground(Token::Foreground.secondary()),      // 60 % (hiérarchie Alpha 13)
        RoundedRectangle(16)
            .fill(LinearGradient::new([Color::Blue, Color::Purple]).from(UnitPoint::TOP_LEADING)
                                                                    .to(UnitPoint::BOTTOM_TRAILING))
            .frame(240, 120),
        Circle().fill(AngularGradient::new([Token::Red, Token::Yellow, Token::Green, Token::Blue, Token::Red])),
        Rectangle().fill(MeshGradient::grid(3, 3, couleurs)),
        Button("Valider", || {}),                                  // prend Token::ButtonBackground, Token::Tint
    ]
    .theme(mon_theme())                     // thème du sous-arbre
    .color_scheme(ColorScheme::Dark)        // forcer le sombre ici (sinon : celui de l'OS)
    .saturation(0.8)                        // réglage de couleur du sous-arbre
}

fn mon_theme() -> Theme {
    Theme::new("Marque")
        .color(Token::Tint, Color::hex(0xFF6A00), Color::hex(0xFF8A33))     // clair, sombre
        .color(Token::custom("Marque.Fond"), LinearGradient::new([..]), Color::hex(0x101010))
        .metric("CornerRadius.Card", 20.0)
}
```

- Partout où l'on donne une couleur (`fill`, `foreground`, `background`, `stroke`, `tint`), on peut donner un
  **`Paint`** : une `Color`, un `Token` ou un dégradé.
- `Token::Accent`… sont les **26 jetons d'Alpha 13** (§3.1) ; `Token::custom("…")` pour les siens.
- `Environment::color_scheme()` et `Environment::resolve(token)` pour les vues qui calculent leurs couleurs.

### 2.2 C

```c
VekaPaint degrade = veka_linear_gradient((VekaColor[]){ VEKA_BLUE, VEKA_PURPLE }, 2,
                                         VEKA_UNIT_TOP_LEADING, VEKA_UNIT_BOTTOM_TRAILING);
VekaElement *carte = veka_fill_paint(veka_rounded_rectangle(16), degrade);
VekaElement *titre = veka_foreground_paint(veka_text("Titre"), veka_token("Foreground", VEKA_PRIMARY));

VekaTheme *theme = veka_theme_new("Marque", NULL);
veka_theme_set_color(theme, "Tint", veka_solid(0xFF6A00FF), veka_solid(0xFF8A33FF));
VekaElement *zone = veka_theme(contenu, theme);
```

Et, comme toujours, tout passe aussi par le schéma : `fill(paint:)` accepte `{"linearGradient": {...}}` ou
`{"token": "Accent"}`, ce qui servira tel quel au `.vk`.

## 3. Le modèle

### 3.1 Jetons

Les jetons d'Alpha 13, chacun avec une valeur claire et une valeur sombre (palette système d'Apple, comme
VekaUI) :

- couleurs : `Accent`, `Black`, `Blue`, `Brown`, `Clear`, `Cyan`, `Gray`, `Green`, `Indigo`, `Mint`, `Orange`,
  `Pink`, `Purple`, `Red`, `Teal`, `White`, `Yellow` ;
- sens : `Tint`, `Background`, `WindowBackground`, `Foreground`, `Separator`, `Selection`, `Placeholder`, `Link`,
  `Fill`.

Plus les **jetons de contrôle** (`ButtonBackground`, `ToggleAccent`, `SliderKnob`…), qui, tant qu'aucun thème ne
les fixe, renvoient à un autre jeton (`ToggleAccent → ThemeAccent → Tint`). Les composants ne lisent que ces
jetons : un thème change l'allure d'un contrôle sans toucher au code.

**Hiérarchie** (Alpha 13) : `primary` 100 %, `secondary` 60 %, `tertiary` 30 %, `quaternary` 18 %.

Un jeton inconnu s'affiche en **magenta** avec un avertissement unique, comme VekaUI.

### 3.2 Paint

```rust
pub enum Paint {
    Solid(Color),
    Token(Token, Hierarchy),
    Linear(LinearGradient),       // stops, from, to            (UnitPoint : 0..1 dans le rectangle)
    Radial(RadialGradient),       // stops, center, start_radius, end_radius (en points)
    Elliptical(EllipticalGradient), // stops, center, rayons en fraction de la demi-taille
    Angular(AngularGradient),     // stops, center, start_angle, end_angle (degrés, sens horaire depuis +X)
    Mesh(MeshGradient),           // largeur × hauteur points, positions et couleurs, lissage bicubique
}
```

Un *stop* porte une couleur **ou un jeton**. Chaque dégradé choisit son espace d'interpolation : `Oklab`
(défaut), `OkLch` (garde la saturation, la teinte prend le plus court chemin), `Linear`, `Srgb`.

### 3.3 Thèmes et mode clair / sombre

- `Theme { name, parent, colors: jeton → (Paint clair, Paint sombre), metrics: nom → f32 }`. Un jeton absent
  vient du parent, puis des valeurs Veka. Un jeton peut valoir un dégradé.
- Dans l'environnement : `theme`, `color_scheme` et `color_matrix`. `.theme()`, `.color_scheme()` et les
  réglages les modifient pour le sous-arbre.
- Le mode clair / sombre vient de la plateforme : Windows (`AppsUseLightTheme`, message `WM_SETTINGCHANGE`),
  macOS (`effectiveAppearance`, observé). Un changement redessine la fenêtre, sans relancer l'app.

## 4. Comment ça marche

### 4.1 Les jetons sont résolus avant le rendu

Au moment de peindre, `veka-ui` remplace chaque jeton par sa valeur (thème + mode de l'environnement) et applique
la matrice de couleur. La `DrawList` ne contient donc que des couleurs et des dégradés **déjà résolus** : les
renderers (wgpu aujourd'hui, Unreal et Unity demain) n'ont rien à savoir des thèmes.

### 4.2 Dégradés dans le renderer wgpu

- Le CPU **précalcule** chaque dégradé en une rampe de 256 texels (couleurs linéaires prémultipliées, interpolées
  dans l'espace demandé) rangée dans un **atlas de rampes**. Le shader des formes calcule la position `t` selon
  le type (linéaire, radial, elliptique, angulaire) et lit la rampe : nombre de stops illimité, le shader reste
  simple, et les dégradés à l'identique sont partagés.
- Dégradé maillé : précalculé (bicubique Catmull-Rom, dans l'espace du dégradé) en une petite texture de 64 × 64
  dans le même atlas, lue avec les coordonnées de la forme.
- Les rampes sont mises en cache par contenu : 0 travail tant qu'un dégradé ne change pas.
- Le même code de calcul (`veka-colora`) sert au renderer, aux tests et plus tard à Anima, qui animera les
  couleurs en Oklab.

### 4.3 Ce qui change dans les crates

| Crate | Changement |
|---|---|
| `veka-colora` (nouveau) | espaces de couleur (sRGB, linéaire, Oklab, OkLCH), dégradés et leur échantillonnage, jetons et palette, `Theme`, `ColorMatrix` |
| `veka-graphics` | `Paint` (celui de Colora) gagne les dégradés ; dans une `DrawList` il est toujours résolu (un jeton oublié s'afficherait en magenta) |
| `veka-ui` | `Paint` accepté partout ; `.theme()`, `.color_scheme()`, `.brightness()`, `.contrast()`, `.saturation()`, `.hue_rotation()`, `.grayscale()` ; composants passés aux jetons ; schéma |
| `veka-render-wgpu` | atlas de rampes, types de remplissage dans le shader |
| `veka-platform-*` | `ColorScheme` du système et événement `ColorSchemeChanged` |
| `veka-ffi` | `VekaPaint`, dégradés, `VekaTheme`, `veka_theme`, `veka_color_scheme`, réglages |

## 5. Plan

- **PR F** : `veka-colora`, `Paint` complet, résolution des jetons, thèmes, réglages, rendu des cinq dégradés ;
  clair / sombre de Windows ; exemple Rust ; essais sur le PC.
- **PR G** : interface C et exemple C, schéma, clair / sombre de macOS ; essais sur le PC (puis le Mac).

## 6. Choix à valider

1. **Jetons** : *recommandé* = reprendre les 26 jetons d'Alpha 13 + les jetons de contrôle à renvoi (§3.1), avec
   `Token::custom` pour les siens. Autre possibilité : un jeu plus court, façon SwiftUI (`primary`,
   `secondary`, `accent`…).
2. **Espace d'interpolation par défaut** : *recommandé* = **Oklab** (dégradés propres, comme VekaUI). Autre
   possibilité : sRGB, comme la plupart des outils de design (milieux plus ternes).
3. **Rendu des dégradés** : *recommandé* = rampes précalculées dans un atlas (§4.2). Autre possibilité : stops
   envoyés au shader (limité à 8 ou 16 stops, shader plus lourd).
4. **Clair / sombre** : *recommandé* = suivre l'OS par défaut, en direct, avec `.color_scheme()` pour forcer.
   Autre possibilité : clair par défaut, l'app choisit.
5. **Couleur d'accent** : *recommandé* = le bleu Veka par défaut, et `Accent::System` pour prendre celle de l'OS
   (Windows et macOS). Autre possibilité : suivre l'OS par défaut.
6. **Ce qui attend** : *recommandé* = les **modes de fusion** (Multiply, Screen… qui lisent l'arrière-plan)
   arrivent avec les effets et le Liquid Glass ; les **images** comme remplissage arrivent avec la vue `Image`.
   Les réglages de couleur, eux, sont faits ici (ils ne coûtent rien : appliqués aux couleurs calculées).
7. **Thèmes dans des fichiers** : *recommandé* = en Rust et en C maintenant, en fichier avec l'étape `.vk` (même
   format que les vues). Autre possibilité : un format JSON dès maintenant.
