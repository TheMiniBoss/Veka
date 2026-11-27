# Veka : étape 4, Anima

Statut : **à valider**. Fait suite à `VEKA_ETAPE3.md`. Reprend la conception d'Anima dans VekaUI (C++ phase 2 :
`FVekaAnimationSpec`, `WithAnimation`, animations implicites, transitions, timelines, *matched geometry*), pas
son code.

## 1. Objectif

Anima, c'est **tout ce qui bouge** dans une vue Veka. On ne décrit pas une animation image par image : on change
un état, et Veka anime le passage de l'ancienne vue à la nouvelle, comme SwiftUI.

- des **courbes** : linéaire, *ease in / out / in-out*, circulaires, Bézier cubique (comme `cubic-bezier` en CSS),
  ressorts physiques ; plus délai, vitesse, répétition et aller-retour ;
- des **animations implicites** : `with_animation(spec, || etat.set(..))` ou `.animation(spec, valeur)`. Tout ce
  qui change alors s'anime : tailles et positions (le layout), couleurs (en Oklab, avec Colora), opacité,
  décalage, échelle, rotation ;
- des **transitions** : comment une vue entre et sort de l'arbre (fondu, échelle, glissement, poussée…) ;
- de nouveaux **modifiers visuels** animables : `.opacity()`, `.offset()`, `.scale_effect()`,
  `.rotation_effect()`. Ils ne refont pas le layout, ils redessinent seulement ;
- des **timelines** : des pistes à images clés et des événements nommés, pour les animations écrites à l'avance
  (PR I).

**Critères d'acceptation**

1. Un exemple `anima` (Rust et C) avec trois parties :
   - une carte qui s'agrandit avec un ressort ;
   - une liste dont les éléments entrent et sortent avec des transitions ;
   - un bouton qui change de couleur, et une galerie des courbes.
2. Les animations sont fluides à 60 / 120 Hz sur le PC. Une animation interrompue repart de là où elle en est,
   sans saut, et un ressort garde sa vitesse.
3. **0 % CPU au repos** : Veka ne demande des images que pendant une animation, puis s'arrête.
4. Le temps est donné par l'hôte. Les tests rejouent une animation image par image, au temps près, sans
   horloge réelle. Ils comparent des instantanés à 0 %, 50 % et 100 %.
5. Le cœur reste sans appel système (`check-core`).

## 2. Ce que l'utilisateur écrit

### 2.1 Rust

```rust
use veka::prelude::*;

fn vue() -> impl View {
    let ouvert = State::new(false);
    let elements = State::new(vec![1, 2, 3]);

    VStack![spacing: 12;
        RoundedRectangle(16)
            .fill(if ouvert.get() { Token::Accent } else { Token::Gray })
            .frame(if ouvert.get() { 280 } else { 120 }, 80)
            .on_tap(move || with_animation(Animation::bouncy(), || ouvert.update(|o| *o = !*o))),

        // Tout le sous-arbre s'anime quand `ouvert` change, même sans with_animation.
        Text("Détails").opacity(if ouvert.get() { 1.0 } else { 0.0 })
            .animation(Animation::ease_in_out(0.25), ouvert.get()),

        ForEach(elements.get(), |n| {
            Text(format!("Élément {n}")).id(n).transition(Transition::push(Edge::Bottom))
        }),

        Button("Ajouter", move || with_animation(Animation::snappy(), || elements.update(|v| v.push(v.len() + 1)))),
    ]
}
```

- `Animation::linear(0.3)`, `ease_in(..)`, `ease_out(..)`, `ease_in_out(..)`, `timing_curve(x1, y1, x2, y2, d)`.
- Les ressorts : `spring(reponse, amortissement)`, `interpolating_spring(masse, raideur, amortissement)` et les
  préréglages `smooth()`, `snappy()` et `bouncy()`.
- On les combine : `.delay(0.1)`, `.speed(2.0)`, `.repeat(3, aller_retour)`, `.repeat_forever(aller_retour)`.
- `Transition::opacity()`, `scale(0.8)`, `slide()`, `move_edge(Edge::Top)`, `push(Edge::Bottom)`,
  `offset(x, y)`, `identity()`, `.combined(autre)`, `Transition::asymmetric(entree, sortie)`.
- `.on_animation_end(|| ..)` : appelé quand les animations lancées par un changement sont toutes finies.

### 2.2 C

```c
static void basculer(VekaContext *cx, void *user) {
    Carte *c = user;
    veka_with_animation(cx, veka_animation_bouncy(), basculer_sans_animation, c);
}

VekaElement *texte = veka_animation(veka_opacity(veka_text("Détails"), ouvert ? 1 : 0),
                                    veka_animation_ease_in_out(0.25), ouvert);   /* valeur sur 64 bits */
VekaElement *ligne = veka_transition(veka_text("Élément"), veka_transition_push(VEKA_EDGE_BOTTOM));
```

`VekaAnimation` est une petite structure passée par valeur, sans allocation. Comme toujours, tout passe aussi par
le schéma : `animation(spec:, value:)`, `transition(insertion:, removal:)`, `opacity(value:)`… prêts pour le `.vk`.

## 3. Le modèle

### 3.1 `Animation` (la spec)

```rust
pub struct Animation {
    pub curve: Curve,            // Linear, EaseIn, EaseOut, EaseInOut, CircEaseIn/Out/InOut,
                                 // Bezier(x1, y1, x2, y2), Spring { masse, raideur, amortissement, vitesse }
    pub duration: f32,           // secondes (ignorée par un ressort, qui s'arrête de lui-même)
    pub delay: f32,
    pub speed: f32,
    pub repeat: Repeat,          // Once, Count(n), Forever
    pub autoreverse: bool,
}
```

Les valeurs par défaut sont celles de VekaUI et de SwiftUI : 0,35 s en *ease in-out*. Le ressort par défaut est
`spring(0.55, 0.825)`. Le ressort est **analytique** : la position et la vitesse sont exactes à tout instant, sans
dérive selon la fréquence d'images.

### 3.2 Ce qui s'anime

Un primitive dit ce qui, chez lui, est animable avec `fn animatable(&self) -> AnimatableData`, une petite liste
de nombres et de couleurs, comme `animatableData` dans SwiftUI. Il dit aussi comment se reconstruire à partir de
valeurs intermédiaires (`fn with_animatable(..)`). Tout le reste change d'un coup.

- **Nombres** : interpolés tels quels (rayons des coins, épaisseur du trait, opacité, décalage, échelle, angle).
- **Couleurs** : interpolées en **Oklab**, en prémultiplié. Un jeton est résolu avant d'être animé, ce qui permet
  aussi d'animer le passage du clair au sombre.
- **Dégradés** : animés arrêt par arrêt s'ils ont la même forme (même type, même nombre d'arrêts). Sinon, ils
  changent d'un coup.
- **Layout** : chaque nœud dont le rectangle change s'anime de son rectangle affiché vers le nouveau. Ses
  enfants font de même, chacun pour soi, comme dans SwiftUI.
- **Ce qui ne s'anime pas** (pour l'instant) : le texte lui-même, qui change d'un coup. Il est seulement déplacé
  et redimensionné avec son nœud. Les chiffres qui défilent (`content_transition`) viennent plus tard.

### 3.3 Transactions

- `with_animation(spec, f)` : chaque état modifié pendant `f` garde la spec. Les vues ré-évaluées à cause de ces
  états, et le layout qui suit, s'animent avec elle.
- `.animation(spec, valeur)` : si `valeur` a changé depuis la dernière évaluation, le sous-arbre s'anime avec
  `spec`, même hors de `with_animation`. Cette spec l'emporte sur celle de la transaction.
  `.animation(None, valeur)` coupe l'animation.
- Sans transaction, tout change d'un coup, comme aujourd'hui.

### 3.4 Interruptions

Quand une valeur reçoit une nouvelle cible au milieu d'une animation :

- **ressort** : il repart de la position **et de la vitesse** affichées. Le mouvement reste continu, sans
  à-coup ;
- **courbe à durée** : elle repart de la valeur affichée vers la nouvelle cible, avec la nouvelle spec.

### 3.5 Transitions

Une transition décrit l'état « absent » d'une vue : transparente, réduite, hors du cadre… Une vue qui entre
pendant une transaction part de cet état et y retourne quand elle sort. Une vue qui sort reste affichée jusqu'à
la fin de sa sortie, mais ne prend plus de place et ne reçoit plus le pointeur. Sa place se referme en même
temps, grâce au layout animé.

La transition par défaut est le **fondu**. Le flou attendra les effets, et sera rendu par un fondu en attendant,
comme dans VekaUI.

## 4. Comment ça marche

### 4.1 Le temps et l'ordonnanceur

- L'hôte donne l'heure : `Surface::frame_at(now)`. veka-app y met l'heure de la plateforme, les tests une heure
  inventée. `frame()` reste disponible et garde l'heure précédente.
- Il y a **un ordonnanceur par surface** : une liste plate des animations en cours. Il n'y a pas de minuterie
  par vue.
- `needs_frame()` est vrai tant qu'une animation tourne. veka-app redessine à chaque synchronisation verticale
  (présentation `Fifo`), puis s'arrête quand la liste est vide. On garde ainsi 0 % CPU au repos.
- Une animation qui ne change que des valeurs visuelles (opacité, décalage, échelle, rotation, couleurs)
  **redessine sans refaire le layout**. Seuls les rectangles animés coûtent un placement, qui reste en cache.

### 4.2 Valeurs cibles et valeurs affichées

Chaque nœud animé garde deux versions de son primitive :

- **la cible**, c'est-à-dire le dernier élément reçu ;
- **l'affichée**, celle que l'on dessine.

À chaque image, l'ordonnanceur avance les animations, reconstruit l'affiché à partir des valeurs intermédiaires
et marque le nœud à redessiner. Le hit-test et l'accessibilité utilisent l'affiché, comme on le voit à l'écran.

### 4.3 Ce qui change dans les crates

| Crate | Changement |
|---|---|
| `veka-anima` (nouveau, cœur) | `Animation`, courbes, Bézier, ressort analytique et temps d'arrêt, `Animator` (avance une valeur, garde la vitesse), `Transition`, timelines (PR I) |
| `veka-ui` | transactions dans `State`, `.animation()`, `.transition()`, `.opacity()`, `.offset()`, `.scale_effect()`, `.rotation_effect()`, `animatable` sur les primitives, nœuds sortants gardés jusqu'à la fin, `frame_at`, schéma |
| `veka-graphics` | rien de neuf : `PushTransform` et `PushLayer` existent déjà |
| `veka-app` | heure de la plateforme à chaque image ; redessin continu seulement pendant une animation |
| `veka-ffi` | `VekaAnimation`, `veka_with_animation`, `veka_animation`, `VekaTransition`, `veka_transition`, modifiers visuels |

## 5. Plan

- **PR H** :
  - `veka-anima` ;
  - les transactions et `.animation()` ;
  - le layout animé ;
  - les couleurs et les modifiers visuels ;
  - les transitions ;
  - l'exemple Rust, les tests image par image et les essais sur le PC.
- **PR I** :
  - les timelines (pistes, événements, lecture, pause, retour) ;
  - `matched_geometry_effect` (une vue qui « vole » d'un endroit à l'autre) ;
  - l'interface C et l'exemple C ;
  - le schéma ;
  - le réglage « réduire les animations » de l'OS ;
  - les essais sur le PC.

## 6. Choix à valider

1. **Modèle** : *recommandé* = **implicite**, façon SwiftUI. On change un état dans `with_animation` ou sous
   `.animation(spec, valeur)`, et tout ce qui en découle s'anime. C'était le modèle de VekaUI. Autre
   possibilité : des animations explicites seulement (`Animated<f32>` que la vue lit), plus simples mais à
   brancher à la main partout.
2. **Ce qui est animable** : *recommandé* = chaque primitive déclare ses valeurs animables, nombres et couleurs
   (§3.2). Le Rust n'a pas la réflexion d'Unreal, et une déclaration explicite reste rapide et vérifiable. Le
   texte change d'un coup. Autre possibilité : n'animer que le layout et les nouveaux modifiers visuels.
3. **Couleurs** : *recommandé* = interpolées en **Oklab**, comme les dégradés de Colora. Autre possibilité :
   sRGB, qui donne des milieux plus ternes.
4. **Interruption** : *recommandé* = un ressort garde sa vitesse, une courbe à durée repart de la valeur
   affichée (§3.4). Autre possibilité : les animations s'additionnent (SwiftUI, *additive*), ce qui est plus
   fluide pour les courbes mais plus lourd.
5. **Spec par défaut** de `with_animation(Animation::default())` : *recommandé* = **ressort `smooth`**, ce que
   fait SwiftUI depuis iOS 17. Autre possibilité : *ease in-out* de 0,35 s, comme VekaUI.
6. **Réduire les animations** (réglage d'accessibilité de Windows et de macOS) : *recommandé* = respecté par
   défaut. Les déplacements et changements de taille deviennent des fondus courts, et une vue peut l'ignorer
   avec `.ignore_reduce_motion()`. Ce choix arrive en PR I. Autre possibilité : laisser l'app décider.
7. **Timelines** : *recommandé* = en Rust et en C dans la PR I. Elles passeront en fichier avec l'étape `.vk`,
   et dans l'éditeur plus tard. Autre possibilité : les remettre à l'étape `.vk`.
8. **Ce qui attend** : *recommandé* = le **flou** des transitions attend les effets (rendu par un fondu en
   attendant). Les **chiffres qui défilent** et le **morphing de formes** (`VekaMorph`) viennent après la
   PR I.
