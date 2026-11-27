# Veka — Étape 1 : document de conception

> **Statut : proposition, en attente de validation.** Aucun code n'est écrit avant ton accord.
> Les choix à valider sont regroupés au §12 ; chacun a une recommandation.

| | |
|---|---|
| Dépôt | `TheMiniBoss/Veka` (Rust) |
| Référence | `TheMiniBoss/Veka-Unreal`, branche `C++` : `Docs/VEKAUI_PHASE0.md`, `Docs/TODO.md` (**lecture seule**) |
| Cibles | Windows 10/11 (DirectX 12), macOS 13+ (Metal) |
| Toolchain | Rust stable (édition 2024), `Cargo.lock` versionné |

---

## 1. But de l'étape 1

Une fenêtre sur Windows et macOS qui affiche :

- du **texte** ;
- des **formes** (rectangle arrondi, capsule, cercle) ;
- une **pile verticale** et une **pile horizontale** ;
- un **bouton qui compte les clics** (le libellé se met à jour).

Le **même exemple** est écrit deux fois : en Rust, et dans un **petit programme C** qui passe par l'interface C.

### Critères d'acceptation

1. `cargo run --example compteur` ouvre la fenêtre sur Windows et sur macOS ; chaque clic incrémente le compteur.
2. `examples/c/compteur.c`, compilé contre `veka.h` et la bibliothèque Veka, affiche la même fenêtre et se comporte pareil.
3. Le cœur (`veka-foundation`, `veka-graphics`, `veka-text`, `veka-ui`) **ne dépend d'aucune API système** :
   il compile et ses tests passent sur Linux, sans fenêtre ni GPU.
4. Au repos (pas de souris qui bouge, rien qui change) : **aucune image redessinée, 0 % CPU**.
5. La liste de dessin contient déjà la commande de lecture de l'arrière-plan (pour le futur Liquid Glass),
   et le renderer wgpu dessine dans une texture intermédiaire qui permet de la lire.
6. CI GitHub Actions verte sur Windows et macOS (build, tests, compilation de l'exemple C).

### Hors étape 1

Colora (thèmes, dégradés), Anima (animations), `.vk`, symboles, clavier et focus, saisie de texte, défilement,
plusieurs fenêtres, effets (flou, verre), renderers Unreal et Unity. L'architecture les prévoit (§10), on ne les
construit pas.

---

## 2. Ce qu'on reprend de VekaUI (Unreal)

VekaUI C++ est allé jusqu'à la Phase 4 (Liquid Glass, Hub, `.vk`). On garde **la conception**, pas le code :

| VekaUI (Unreal) | Veka (Rust) |
|---|---|
| `FVekaElement` : description légère, modifiers chaînés | `Element` (même rôle) |
| `FVekaNode` : arbre retenu qui porte l'état et le layout | `Node` (même rôle) |
| Reconciler : diff par type + identité, `.Id()` explicite | idem |
| `FVekaState<T>`, `FVekaEnvironment` | `State<T>`, `Environment` |
| `Stack` unique (`Axis = H / V / Depth`) + alias `HStack` / `VStack` / `ZStack` | idem |
| Protocole de layout `SizeThatFits` / `Place` (`FVekaLayoutChild`) | idem (trait `Layout`) |
| Formes SDF, hit-test exact (pas de clic dans les coins arrondis) | idem, SDF partagés entre rendu et hit-test |
| « Le framework utilise sa propre API d'extension » | idem : `Stack`, `Text`, `Button` sont écrits avec l'API publique |
| Ré-évaluation différée à la fin de la frame (coalescing) | idem |
| Scheduler central, pas de tick par widget ; 0 coût au repos | idem |
| Backdrop : **une** capture + une pyramide de flou (dual Kawase) partagée par couche | prévu dans la liste de dessin dès maintenant (§7.3) |

Ce qui change : **Slate disparaît**. Veka fait lui-même layout, dessin (liste de dessin), hit-test et
distribution des événements. C'est ce qui permet d'être portable vers n'importe quel hôte.

Leçon tirée de `TODO.md` (verre trop sombre dans la galerie du Hub, « un hôte ne prend qu'une capture par image ») :
la liste de dessin découpe le rendu en **segments** ; un effet qui lit l'arrière-plan lit ce qui a été dessiné
**avant lui** dans son segment, et une nouvelle capture est prise si du contenu a été dessiné depuis la précédente.

---

## 3. Architecture

```
            Rust (API Veka)                    C / C++ / C# / moteurs
                  │                                       │
                  │                           ┌───────────▼───────────┐
                  │                           │  veka-ffi  (veka.h)   │
                  │                           └───────────┬───────────┘
                  ▼                                       ▼
  ┌──────────────────────────────────────────────────────────────────┐
  │ veka-ui   View, Element, Node, State, layout, contrôles, fenêtre │  ← CŒUR
  │ veka-text   polices, mise en forme, mise en page du texte        │    (aucun appel
  │ veka-graphics   formes, chemins, Paint, liste de dessin          │     système)
  │ veka-foundation   géométrie, couleurs, ids, temps                │
  └───────────────┬──────────────────────────────┬───────────────────┘
                  │ DrawList (données pures)     │ événements / services (traits)
                  ▼                              ▼
     ┌────────────────────────┐       ┌────────────────────────────┐
     │ Renderer (trait)       │       │ Platform (trait)           │
     │  └ veka-render-wgpu    │       │  └ veka-platform-winit     │
     │  └ (Unreal, Unity…)    │       │  └ (hôte Unreal, AzOS…)    │
     └────────────────────────┘       └────────────────────────────┘
                  ▲                              ▲
                  └──────────── veka-app ────────┘   (assemble Platform + UI + Renderer)
```

Trois règles :

1. **Le cœur ne voit jamais l'OS.** Tout ce qui touche au système (fenêtres, entrées, horloge, presse-papier,
   polices du système plus tard) passe par le trait `Platform`. Pour AzOS, on écrit une autre implémentation
   de `Platform` ; le cœur ne change pas. C'est vérifié mécaniquement : les crates du cœur n'ont **aucune**
   dépendance vers `winit`, `windows`, `objc2` ou `wgpu` (contrôle en CI, §11).
2. **Le cœur ne dessine pas.** Il produit une `DrawList` (données, `Send`), un renderer la dessine. Un renderer
   Unreal ou Unity lira la même liste.
3. **L'hôte peut posséder la fenêtre.** Dans Unreal ou Unity, c'est le moteur qui a la fenêtre, la boucle et le GPU.
   Veka doit donc aussi pouvoir être **piloté de l'extérieur** : l'hôte pousse les événements, demande une
   `DrawList`, la dessine lui-même (§8.2). `veka-app` n'est qu'un hôte parmi d'autres.

---

## 4. Crates et modules

Un **workspace Cargo**, une crate par module (proposition, choix n° 1). La crate `veka` regroupe tout, à la
manière des frameworks Apple : `veka::foundation`, `veka::graphics`, `veka::ui`…

```
Veka/
├─ Cargo.toml                    workspace
├─ crates/
│  ├─ veka/                      façade : réexporte les modules, `veka::prelude`
│  ├─ veka-foundation/           Point, Size, Rect, Insets, Transform2D, Color (sRGB linéaire), Id, Instant
│  ├─ veka-graphics/             Shape (SDF), Path (plus tard), Paint, DrawList
│  ├─ veka-text/                 Font, FontCollection, TextStyle, TextLayout (mise en forme + lignes)
│  ├─ veka-ui/                   View, Element, Node, State, Environment, Layout, Stack, Text, Shape, Button
│  ├─ veka-platform/             traits Platform / Window / événements (aucune implémentation)
│  ├─ veka-platform-winit/       implémentation Windows + macOS
│  ├─ veka-render/               trait Renderer (aucune implémentation)
│  ├─ veka-render-wgpu/          renderer wgpu (DX12 / Metal)
│  ├─ veka-app/                  boucle d'application : Platform + UI + Renderer
│  └─ veka-ffi/                  interface C (cdylib + staticlib), génère include/veka.h
├─ examples/
│  ├─ compteur.rs                l'exemple en Rust
│  └─ c/compteur.c (+ CMakeLists.txt)
├─ include/veka.h                généré par cbindgen, versionné
└─ Docs/
```

Colora, Anima et Lang **n'ont pas de crate** à l'étape 1 (pas de crates vides). Ils arriveront comme
`veka-colora`, `veka-anima`, `veka-lang`. Le module `Graphics` garde un `Color` simple en attendant Colora.

Dépendances entre crates (une flèche = « dépend de ») :

```
veka-ui → veka-text → veka-graphics → veka-foundation
veka-render-wgpu → veka-render → veka-graphics
veka-platform-winit → veka-platform → veka-foundation
veka-app → veka-ui, veka-render, veka-platform
veka-ffi → veka-app
```

---

## 5. Modèle déclaratif (veka-ui)

### 5.1 L'exemple en Rust (cible)

```rust
use veka::prelude::*;

struct Compteur;

impl View for Compteur {
    fn body(&self, cx: &mut BuildContext) -> Element {
        let clics = cx.state(|| 0u32);

        VStack::new().spacing(16).children((
            Text::new("Bonjour Veka").size(28.0),
            HStack::new().spacing(12).children((
                Shape::rounded_rect(12.0).fill(Color::rgb(0.25, 0.5, 1.0)).frame(80.0, 48.0),
                Shape::capsule().fill(Color::rgb(1.0, 0.6, 0.2)).frame(96.0, 48.0),
                Shape::circle().fill(Color::rgb(0.3, 0.8, 0.4)).frame(48.0, 48.0),
            )),
            Button::new(format!("Clics : {}", clics.get(cx)), move |cx| {
                clics.update(cx, |n| *n += 1);
            }),
        ))
        .padding(24)
    }
}

fn main() -> veka::Result<()> {
    App::new().window(Window::new("Veka — Compteur").size(480, 320), Compteur).run()
}
```

### 5.2 Les pièces

| Type | Rôle |
|---|---|
| `trait View` | une vue composée : `fn body(&self, cx) -> Element`. Ré-évaluée quand son état change. |
| `Element` | description légère et clonable : un *primitive* (`Text`, `Shape`, `Stack`…) ou une `View`, + modifiers + enfants + id optionnel. |
| `Node` | arbre **retenu** (arena, ids générationnels) : garde l'état, le layout calculé, le dernier `Element`. |
| Reconciler | compare le nouvel `Element` à l'ancien `Node` par **type + position** (ou `.id()` explicite) ; ne touche que ce qui change. |
| `State<T>` | **poignée** `Copy` vers une valeur stockée dans le `Node` de la vue. `get(cx)`, `set(cx, v)`, `update(cx, f)`. Un `set` marque la vue « à ré-évaluer » ; la ré-évaluation a lieu une fois, avant la prochaine image. |
| `Environment` | valeurs héritées par les enfants (police, taille, couleur du texte, échelle d'affichage…), surchargées par modifier. |
| Modifiers | `.padding()`, `.frame()`, `.background()`, `.foreground()`, `.on_tap()`… Chaque modifier est une valeur d'un trait public `Modifier` (extensible, comme `FVekaModifier`). |

`State<T>` est une poignée plutôt qu'un `Rc<RefCell<T>>` : pas de cycles, pas d'emprunts qui paniquent, et l'état
reste dans l'arbre (il survit aux ré-évaluations et disparaît avec le nœud). Les closures d'action reçoivent un
`ActionContext` qui donne accès à l'état.

### 5.3 Layout

Protocole de VekaUI, sans changement :

- `size_that_fits(proposal) -> Size` : le parent propose une taille (largeur / hauteur chacune `Option<f32>`),
  l'enfant répond ce qu'il veut ;
- `place(rect)` : le parent donne le rectangle final.

Implémenté par un trait public `Layout` ; `Stack` l'utilise comme n'importe qui.

`Stack` (étape 1) : `axis` (Horizontal / Vertical / Depth), `spacing`, `alignment` (début / centre / fin sur l'axe
croisé), distribution « packed ». `Spacer` remplit l'espace restant sur l'axe du Stack parent. Unités : **points
logiques** (`f32`), multipliés par l'échelle d'affichage au moment du rendu (Retina, 125 % / 150 % Windows).

### 5.4 Primitives et contrôles de l'étape 1

| Élément | Détail |
|---|---|
| `Text` | une chaîne, taille, graisse, couleur ; retour à la ligne si la largeur proposée est limitée |
| `Shape` | `rect`, `rounded_rect(r)` (rayons par coin), `capsule`, `circle`, `ellipse` ; `fill(Color)`, `stroke(Color, largeur)` |
| `Stack` / `HStack` / `VStack` / `ZStack` | §5.3 |
| `Spacer` | §5.3 |
| `Button` | libellé (texte) + action ; états **survolé** et **pressé** visibles ; l'action part au relâchement **dans** la forme (comme macOS / Windows) |

Le look du bouton est un **style par défaut** (capsule bleue, texte blanc, assombri au survol / à l'appui), déjà
séparé du comportement pour accueillir les Styles de composants de VekaUI plus tard.

### 5.5 Événements et hit-test

La plateforme envoie des événements bruts (pointeur déplacé, appuyé, relâché, sorti, redimensionnement, échelle).
`veka-ui` les distribue : hit-test du dernier dessiné vers le premier, **avec la forme exacte** (SDF), puis
remontée vers les parents (bubbling). Un appui **capture** le pointeur jusqu'au relâchement.

### 5.6 Threading

L'arbre de vues vit sur **un seul thread** (le thread principal à l'étape 1, imposé par macOS pour les fenêtres).
Ce qui en sort est fait pour le multithread : la `DrawList` est `Send` et le renderer peut la dessiner sur un
autre thread ; la mise en page du texte est pure et parallélisable plus tard. Pas de thread de layout dédié
(même conclusion que VekaUI §13).

---

## 6. Texte (veka-text)

- Mise en forme (*shaping*) et mise en page avec **Parley** (crate Linebender : shaping, bidi, retour à
  la ligne) — choix n° 3.
- Résultat : un `TextLayout` (lignes, glyphes positionnés, métriques). `veka-ui` s'en sert pour mesurer et pour
  produire des commandes `Glyphs` dans la liste de dessin.
- La **rastérisation** des glyphes n'est **pas** dans le cœur : la liste de dessin donne (police, glyphe, taille,
  position) et chaque renderer rastérise comme il veut (wgpu : `swash` + atlas ; Unreal : ses propres outils).
- **Police** à l'étape 1 : une police libre **embarquée** (Inter, licence OFL) — choix n° 4. Résultat identique
  sur Windows, macOS et dans les tests Linux. LT Flode Neue la remplacera quand la licence sera vérifiée ; les
  polices du système passeront par un service de `Platform` (pas d'accès direct aux dossiers de l'OS dans le cœur).

---

## 7. Liste de dessin (veka-graphics)

### 7.1 Forme

```rust
pub struct DrawList {
    pub commands: Vec<DrawCommand>,
    pub fonts: FontTable,         // polices référencées par les commandes Glyphs
}

pub enum DrawCommand {
    /// Forme analytique : rect, rounded rect (4 rayons), capsule, cercle, ellipse.
    FillShape   { shape: ShapeSdf, rect: Rect, paint: Paint },
    StrokeShape { shape: ShapeSdf, rect: Rect, paint: Paint, width: f32 },
    /// Glyphes déjà placés par veka-text.
    Glyphs      { font: FontId, size: f32, glyphs: Range<u32>, paint: Paint },
    PushClip    { shape: ShapeSdf, rect: Rect },     PopClip,
    PushTransform(Transform2D),                       PopTransform,
    PushLayer   { opacity: f32 },                     PopLayer,
    /// Réservée pour l'étape « verre » : lit ce qui est déjà dessiné derrière `rect`.
    Backdrop    { shape: ShapeSdf, rect: Rect, effect: BackdropEffect },
}
```

- `Paint` = `Solid(Color)` à l'étape 1 ; les dégradés de Colora (linear, radial, angular, mesh) s'y ajouteront.
- `ShapeSdf` porte la **description analytique** de la forme. Le même code SDF sert au hit-test (`veka-ui`), au
  rendu (shader wgpu) et plus tard à la réfraction du verre (normale du SDF, comme VekaUI 4b).
- Coordonnées en points logiques ; l'échelle d'affichage est donnée au renderer avec la liste.

### 7.2 Tri et lots

La liste est dans l'ordre de peinture. Le renderer regroupe les commandes consécutives compatibles en **lots
instanciés** (toutes les formes d'un segment en un seul draw, tous les glyphes d'un atlas en un autre).

### 7.3 Arrière-plan pour Liquid Glass (prévu dès maintenant)

- Une commande `Backdrop` coupe la liste en **segments**. Le renderer dessine le segment, rend son résultat
  lisible (copie de la zone utile), puis l'effet le lit.
- Tous les `Backdrop` d'un même segment **partagent une capture** et une pyramide de flou (leçon de VekaUI 4a).
- Le renderer wgpu dessine toujours dans une **texture intermédiaire** puis la copie vers la fenêtre : c'est ce
  qui rend la lecture possible. Coût mesuré à l'étape 1 (une copie plein écran) ; si c'est trop, on dessine
  directement dans la fenêtre tant qu'il n'y a aucun `Backdrop`.
- Dans un moteur, l'arrière-plan contient aussi la scène 3D : le trait `Renderer` accepte une **texture
  d'arrière-plan externe** fournie par l'hôte.
- À l'étape 1, `BackdropEffect` n'a qu'une variante `None` (tests seulement) ; le flou et le verre arriveront
  avec leurs shaders.

---

## 8. Platform et Renderer

### 8.1 Trait `Platform` (veka-platform)

```rust
pub trait Platform {
    fn run(self, app: Box<dyn PlatformApp>) -> Result<()>;   // possède la boucle (cas app native)
}
pub trait PlatformApp {
    fn event(&mut self, ctx: &mut dyn PlatformContext, event: PlatformEvent);
}
pub trait PlatformContext {
    fn create_window(&mut self, desc: WindowDesc) -> Result<WindowId>;
    fn window(&self, id: WindowId) -> &dyn PlatformWindow;  // taille, échelle, titre, request_redraw, handles
    fn now(&self) -> Instant;
    fn exit(&mut self);
}
pub enum PlatformEvent {
    Resumed, WindowResized { .. }, ScaleChanged { .. }, CloseRequested { .. }, RedrawRequested { .. },
    PointerMoved { .. }, PointerButton { .. }, PointerLeft { .. },
}
```

`PlatformWindow` expose `raw-window-handle` pour que le renderer crée sa surface sans connaître l'OS.

Implémentation de l'étape 1 : **`veka-platform-winit`** (winit gère Win32 et AppKit) — choix n° 2. Elle tient en
quelques centaines de lignes, et on pourra la remplacer par des backends natifs (Win32 / AppKit écrits à la
main) le jour où winit nous limite (barre de titre transparente, vibrancy macOS, IME…), sans toucher au cœur.

### 8.2 Hôte piloté de l'extérieur

Pour Unreal / Unity (pas à l'étape 1, mais l'API le permet) : `veka-ui` expose une `Surface` sans boucle :

```rust
let mut surface = Surface::new(root_view, environment);
surface.resize(size, scale);
surface.handle(InputEvent::PointerMoved { .. });
if surface.needs_frame() { let list: &DrawList = surface.frame(now); /* l'hôte dessine */ }
```

`veka-app` est simplement l'hôte « fenêtre native » : il branche `Platform` → `Surface` → `Renderer`.

### 8.3 Trait `Renderer` (veka-render) et renderer wgpu

```rust
pub trait Renderer {
    fn render(&mut self, list: &DrawList, target: RenderTarget<'_>, scale: f32) -> Result<()>;
}
```

`veka-render-wgpu` :

- backend **DirectX 12** sur Windows, **Metal** sur macOS (Vulkan en repli possible sur Windows) ;
- formes : quads instanciés, **SDF analytique dans le shader WGSL**, antialiasing par la dérivée de distance
  (net à toute échelle, coins arrondis parfaits, aucun maillage) — choix n° 5 ;
- texte : glyphes rastérisés par `swash` dans un **atlas** (niveaux de gris à l'étape 1), quads instanciés ;
- redessin **à la demande** uniquement (`request_redraw` quand l'arbre a changé) → 0 % CPU au repos.

---

## 9. Interface C (veka-ffi)

### 9.1 Principes

- Bibliothèque `veka.dll` / `libveka.dylib` (+ statique), en-tête **`veka.h` généré par cbindgen** et versionné.
- Types opaques (`VekaApp`, `VekaElement`, `VekaContext`, `VekaState`), fonctions `veka_*`, codes d'erreur.
- **Aucune panique ne traverse la frontière** (`catch_unwind` partout, erreur renvoyée + `veka_last_error()`).
- Règle de propriété simple : un `VekaElement*` passé à un conteneur ou à un modifier est **consommé** ;
  le C ne libère que ce qu'il n'a donné à personne.
- Même modèle que Rust (une vue = une fonction `body` ré-appelée quand l'état change) : un programme C et un
  programme Rust se ressemblent, et C++ / C# / Unreal / Unity hériteront du même modèle.

Ce choix est le n° 6.

### 9.2 L'exemple en C (cible)

```c
#include <stdio.h>
#include "veka.h"

typedef struct { VekaState *clics; } Compteur;

static void incrementer(VekaContext *cx, void *user) {
    Compteur *c = user;
    veka_state_set_int(cx, c->clics, veka_state_get_int(cx, c->clics) + 1);
}

static void setup(VekaContext *cx, void *user) {
    Compteur *c = user;
    c->clics = veka_state_new_int(cx, 0);
}

static VekaElement *body(VekaContext *cx, void *user) {
    Compteur *c = user;
    char libelle[64];
    snprintf(libelle, sizeof libelle, "Clics : %d", veka_state_get_int(cx, c->clics));

    VekaElement *formes[] = {
        veka_frame(veka_fill(veka_rounded_rect(12), veka_rgb(0.25f, 0.5f, 1.0f)), 80, 48),
        veka_frame(veka_fill(veka_capsule(),        veka_rgb(1.0f, 0.6f, 0.2f)), 96, 48),
        veka_frame(veka_fill(veka_circle(),         veka_rgb(0.3f, 0.8f, 0.4f)), 48, 48),
    };
    VekaElement *lignes[] = {
        veka_text_sized("Bonjour Veka", 28),
        veka_hstack(12, formes, 3),
        veka_button(libelle, incrementer, c),
    };
    return veka_padding(veka_vstack(16, lignes, 3), 24);
}

int main(void) {
    Compteur c = {0};
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Veka — Compteur", 480, 320, setup, body, &c);
    return veka_app_run(app);   /* bloque jusqu'à la fermeture de la fenêtre */
}
```

Compilation : `cmake` sur les deux OS (MSVC sous Windows, clang sous macOS), lié à la lib produite par
`cargo build -p veka-ffi`. La CI compile et lance l'exemple en mode « une image puis quitter »
(`VEKA_EXIT_AFTER_FIRST_FRAME=1`) pour vérifier que tout s'enchaîne.

---

## 10. Ce que l'architecture prépare (sans le construire)

| Plus tard | Où ça se branche |
|---|---|
| Colora (tokens, thèmes, dégradés, OkLCH) | `Paint` (nouvelles variantes) + `Environment` (thème) ; crate `veka-colora` |
| Anima (ressorts, implicite, transitions) | valeurs « présentées » dans `Node`, scheduler qui demande des images seulement s'il y a une animation |
| `.vk` | `veka-lang` produit des `Element` comme le code Rust (même modèle intermédiaire que VekaUI) |
| Liquid Glass, flou | `DrawCommand::Backdrop` + shaders du renderer ; SDF déjà partagés |
| Symboles, chemins, traits riches | `Path` dans `veka-graphics`, tessellation dans le renderer |
| Unreal, Unity | `Surface` (§8.2) + un renderer par moteur qui lit la `DrawList` ; les shaders VekaUI servent de base |
| AzOS | une autre implémentation de `Platform` |
| C++ / C# confortables | surcouches au-dessus de `veka.h` (classe C++ header-only, P/Invoke C#) |

---

## 11. Tests et vérification

| Niveau | Quoi | Où |
|---|---|---|
| Unitaires | géométrie, SDF (dedans / dehors / bord), layout des Stacks, reconciler (état conservé, nœuds réutilisés), hit-test, bouton (survol / appui / clic hors forme) | Linux (ce conteneur) + CI |
| Instantanés de liste de dessin | l'exemple compteur → `DrawList` sérialisée, comparée à une référence | Linux + CI, sans GPU |
| Pureté du cœur | `cargo tree` : aucune dépendance système dans les crates du cœur | CI |
| Rendu | image de l'exemple rendue hors écran par wgpu, comparée avec tolérance | CI Windows / macOS (adaptateur logiciel si pas de GPU) |
| FFI | build CMake de `compteur.c` + lancement « une image » | CI Windows / macOS |
| À l'œil | fenêtre réelle, clics, Retina / 150 % | ton PC (et ton Mac si tu en as un) |

---

## 12. Choix à valider

| # | Question | Recommandation | Alternative |
|---|---|---|---|
| 1 | Organisation du code | **Workspace multi-crates** + façade `veka` (la séparation cœur / OS est garantie par les dépendances) | une seule crate avec des modules (plus simple, mais rien n'empêche le cœur d'appeler l'OS) |
| 2 | Couche Platform de l'étape 1 | **winit** derrière le trait `Platform` (Windows + macOS tout de suite) | Win32 + AppKit écrits à la main dès maintenant (plus de contrôle, 3 à 4 fois plus de travail) |
| 3 | Texte | **Parley** (Linebender) pour shaping + mise en page, **swash** pour la rastérisation | cosmic-text (très utilisé aussi, API moins orientée texte riche) |
| 4 | Police par défaut de l'étape 1 | **Inter embarquée (OFL)**, rendu identique partout ; LT Flode Neue après vérif de licence | polices du système (Segoe UI / SF Pro) via `Platform` |
| 5 | Rendu des formes | **SDF analytique maison** dans wgpu (même SDF que le hit-test et le futur verre, shaders entièrement à nous) | Vello (rendu 2D complet, mais modèle et shaders imposés, plus dur à porter vers Unreal / Unity) |
| 6 | Modèle de l'API C | **Vue avec fonction `body` ré-appelée + état géré par Veka** (même modèle que Rust) | arbre impératif (créer / modifier / supprimer des widgets à la main) |

Questions ouvertes (ne bloquent pas l'étape 1) :

- **Machines de test** : ton PC Windows pour l'essai à l'œil ; as-tu un Mac, ou la CI macOS suffit-elle pour l'instant ?
- **Licence de Veka** (MIT / Apache-2.0, ou propriétaire) : à décider avant toute diffusion ; ça ne change pas le code.

---

## 13. Découpage proposé (le plan détaillé suit la validation)

1. Workspace, CI Windows / macOS, contrôle de pureté du cœur.
2. `veka-foundation` + `veka-graphics` (géométrie, SDF, `DrawList`) avec tests.
3. `veka-text` (Parley, police embarquée, mesure).
4. `veka-ui` : `Element`, `Node`, reconciler, `State`, layout `Stack`, `Text`, `Shape` ; instantanés de liste de dessin.
5. `veka-platform` + `veka-platform-winit`, `veka-render` + `veka-render-wgpu`, `veka-app` : la fenêtre s'affiche.
6. Événements, hit-test, `Button` : le compteur fonctionne en Rust.
7. `veka-ffi` + `veka.h` + `examples/c/compteur.c` : le compteur fonctionne en C.
8. Vérification sur Windows et macOS, captures, `Docs/CHANGELOG.md`.
