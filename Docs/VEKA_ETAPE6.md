# Veka : étape 6, Liquid Glass

Statut : **validé** (les 9 choix recommandés) ; PR L faite. Fait suite à `VEKA_ETAPE5.md`. Reprend la Phase 4 de VekaUI
(spike 4.0, 4a flou d'arrière-plan, 4b verre liquide : `backdropBlur`, `frostedGlass`, `glass`, `glassGroup`,
`liquidDistortion`, shader `VekaBackdrop.usf`). On garde la conception et les formules des shaders, pas le code.
**Référence visuelle : le Liquid Glass d'iOS / macOS 27** (demande de l'utilisateur), pas celui de 26 (§2.1).

## 1. Objectif

Une vue peut lire **ce qui est déjà dessiné derrière elle**, le flouter, le teinter et le réfracter dans sa forme.
C'est la base du flou d'arrière-plan, du verre dépoli et du **verre liquide** à la manière d'Apple : bord courbe qui
tord le fond, couleurs séparées, reflet qui suit le pointeur, formes qui fusionnent comme des gouttes.

```rust
// Rust
HStack![spacing: 12;
    Button("Lecture", jouer).glass(),
    Button("Suivant", suivant).glass(),
]
.padding(8)
.glass_group(16.0)            // les deux boutons fusionnent quand ils se rapprochent

Text("Réglages").padding(16).frosted_glass()
Rectangle().backdrop_blur(24.0)
```

```swift
// .vk : mêmes noms qu'en Rust, comme le reste du schéma
HStack(spacing: 12) {
    Button("Lecture") { … }.glass()
    Button("Suivant") { … }.glass(.clear.tint(.Blue))
}
.glassGroup(16)
```

```c
/* C */
veka_glass(element, VEKA_GLASS_REGULAR);
veka_glass_group(pile, 16.0f);
```

**Critères d'acceptation**

1. Un exemple `verre` (Rust, `.vk` et C) : une barre d'outils en verre liquide au-dessus d'un fond animé coloré
   (les feux d'artifice et des dégradés), un panneau en verre dépoli, deux boutons qui fusionnent en se
   rapprochant. Le rendu ressemble au verre de VekaUI (captures comparées sur le PC).
2. **Coût GPU** : verre sur toute la fenêtre en 1080p **< 1 ms** (VekaUI : 0,26 ms), mesuré sur le PC.
3. **Au repos, 0 % CPU** : le verre ne redemande une image que si le fond change ou si le pointeur bouge au-dessus.
4. Un seul flou partagé par segment, même avec 20 éléments en verre (test sur la liste de dessin et compteur de
   passes du renderer).
5. Sans GPU qui sait lire l'arrière-plan, ou avec « Réduire la transparence » de l'OS, chaque effet devient une
   teinte unie : rien ne casse.
6. Le hit-test suit la forme fusionnée d'un `glassGroup` (pas de clic dans le vide entre deux gouttes).

## 2. Les effets

| Modifier | Rôle | Réglages principaux |
|---|---|---|
| `backdrop_blur(rayon)` | flou de ce qui est derrière, découpé à la forme | teinte, quantité de teinte, saturation |
| `frosted_glass()` | flou fort, plus de teinte, un peu de grain | teinte, flou, grain |
| `glass()` / `glass_with(GlassMaterial)` | verre liquide façon 27 : réfraction au bord (normale du SDF), aberration chromatique, bord assombri puis reflet vif éclairé par le pointeur, lumière interne, ombre douce | clarté, flou, réfraction, largeur du bord, aberration, reflet, teinte, ombre |
| `glass_group(douceur)` | un seul verre pour tous les `glass()` du sous-arbre (8 formes au plus), fusionnés par minimum lisse | douceur, matériau du groupe |
| `liquid_distortion(force, taille, vitesse)` | le fond ondule dans la forme (eau, chaleur) | force, taille des vagues, vitesse |

- **Formes** : toutes celles de `veka-graphics` (rectangle, rectangle arrondi, capsule, cercle, ellipse). La forme
  vient du `background_shape` ou du `clip_shape` de la vue, sinon d'un paramètre `in: Shape`, comme
  `glassEffect(in:)` chez Apple. Les SDF sont déjà partagés entre le rendu et le hit-test.
- **Matériaux prêts** : `Glass::regular()` (le réglage « Par défaut » de 27) et `Glass::clear()` (plus clair),
  modifiables : `.clarity(0..1)` (de teinté à clair, comme le curseur de 27), `.tint(couleur)`,
  `.interactive(bool)` (reflet qui suit le pointeur et léger gonflement à l'appui, en Anima).
- **Animable** : tous les nombres passent par Anima (rayon de flou, réfraction, douceur du groupe). Les formes d'un
  groupe qui se déplacent avec un ressort fusionnent et se séparent en continu.
- **Colora** : la teinte est une peinture (jeton, couleur ou dégradé) ; les valeurs par défaut suivent le thème
  clair ou sombre.

### 2.1 Ce que la version 27 change, et ce qu'on en prend

D'après les comparaisons publiées (MacStories, Tom's Guide ; Apple ne donne pas de chiffres) :

| iOS / macOS 26 | iOS / macOS 27 | Dans Veka |
|---|---|---|
| Fond très flou, verre qui se fond dans un fond sombre | fond mieux diffusé et **lumière propre au verre** : il se détache même sur du noir | un léger éclairage interne (dégradé du haut vers le bas, ajouté à la couleur) |
| Bord clair seulement | **bord assombri à l'intérieur** + **reflet spéculaire plus vif** : effet de couches | deux anneaux dans le shader : un sombre juste à l'intérieur du bord, puis le reflet |
| Réfraction légère | réfraction plus marquée au bord | réfraction et largeur de bord par défaut plus grandes que VekaUI |
| Deux réglages : Clair ou Teinté | **un curseur** de « Plus clair » à « Plus teinté » ; le teinté masque vraiment le fond au lieu de le flouter | un réglage continu `clarity` (0 teinté … 1 clair) sur le matériau, et une valeur par défaut pour toute l'app, comme le curseur de l'OS |
| Ombre faible | ombre portée plus présente (non confirmé) | une ombre douce sous le verre, réglable |
| Menus sombres | menus en verre avec bord sombre | le même matériau sert aux menus quand ils arriveront |

L'accessibilité ne change pas : « Réduire la transparence » et « Augmenter le contraste » restent les deux réglages
à suivre (§3.4). Les valeurs exactes se règlent à l'œil sur le PC, captures côte à côte avec un iPhone ou un Mac
sous 27 si tu en as un.

## 3. Fonctionnement

### 3.1 Liste de dessin

`DrawCommand::Backdrop { shape, rect, effect }` existe depuis l'étape 1 et coupe déjà la liste en segments.
`BackdropEffect` gagne `Blur`, `Glass` et `Distortion` (paramètres en pixels, teinte déjà résolue par Colora).
Un `glassGroup` émet **une** commande `Backdrop` avec ses formes (jusqu'à 8) au lieu d'une par enfant.

### 3.2 Renderer wgpu

Le renderer dessine déjà dans une texture intermédiaire. Pour chaque segment :

1. dessiner les commandes du segment dans la texture intermédiaire ;
2. copier **l'union des rectangles** des `Backdrop` du segment (plus une marge pour le flou et la réfraction) dans
   une texture de capture ;
3. construire **une** pyramide de flou **dual Kawase** (moitié, quart… jusqu'au plus grand rayon demandé) ;
4. composer chaque effet dans la texture intermédiaire avec son shader : `blur` (teinte, saturation, grain) ou
   `glass` (champ SDF des formes, normale par différences, réfraction et aberration, reflet). Deux niveaux de
   pyramide mélangés donnent n'importe quel rayon, même animé ;
5. passer au segment suivant (ce qui est dessiné après peut à son tour être vu par un autre verre).

Sans aucun `Backdrop` dans l'image, rien ne change : pas de copie ni de passe en plus. Les shaders WGSL reprennent
les formules de `VekaBackdrop.usf`.

### 3.3 Interaction et repos

- La position du pointeur arrive aux effets `interactive` comme une valeur d'environnement : un mouvement au-dessus
  d'un verre redemande une image, ailleurs non.
- Un verre au-dessus d'une animation est redessiné avec elle, sans travail en plus côté CPU.
- `liquid_distortion` demande des images tant qu'elle est visible et que sa vitesse n'est pas 0 (comme `@Clock`).

### 3.4 Repli et accessibilité

- **Qualité** : `Quality::{Off, Low, Medium, High}` sur l'app (niveaux de pyramide, prises de l'aberration).
  `Off` : teinte unie.
- **« Réduire la transparence »** (macOS) et **« Effets de transparence » désactivés** (Windows) : teinte unie et
  opaque, relue quand le réglage change.

### 3.5 Fonds du système (fenêtre)

Un verre de Veka ne voit que ce que Veka dessine : il ne peut pas lire le bureau derrière la fenêtre. Pour une
fenêtre « en verre » sur le bureau, on utilise le matériau du système, prévu depuis l'étape 2 :
`WindowDesc::backdrop(SystemBackdrop::{None, Mica, MicaAlt, Acrylic})` sur Windows 11 (DWM) et `Sidebar`,
`HudWindow`… sur macOS (`NSVisualEffectView`). Le contenu de Veka se dessine par-dessus, avec du verre de Veka
où il le faut.

## 4. Plan

- **PR L, le flou d'arrière-plan** :
  - `BackdropEffect::Blur`, la capture par segment et la pyramide dual Kawase dans veka-render-wgpu ;
  - `backdrop_blur`, `frosted_glass`, schéma, `.vk` et C ;
  - qualité, repli « réduire la transparence » ;
  - compteur de passes et tests de la liste de dessin ;
  - essais sur le PC (rendu, coût GPU).
  - Fait. Écarts : la pyramide est une réduction 2×2 puis un flou gaussien par niveau (plutôt que le dual
    Kawase montant de VekaUI) ; des effets partagent une capture dès que rien n'a été dessiné sur leur zone
    depuis (pas seulement s'ils se suivent), et un verre posé sur un autre le voit ; la teinte est la couleur
    moyenne de la peinture ; la forme se donne par `shape` (fond ou découpage de la vue : PR M) ; qualité
    `Off` / `Low` / `High`.
- **PR M, le verre liquide** :
  - `glass`, `Glass::regular` / `clear`, `.tint`, `.interactive` ;
  - `glass_group` (minimum lisse, hit-test fusionné) et `liquid_distortion` ;
  - fonds du système sur la fenêtre (`SystemBackdrop`) ;
  - exemple `verre` en Rust, `.vk` et C ;
  - essais sur le PC (rendu, coût GPU, 0 % CPU au repos).

## 5. Choix à valider

1. **Découpage** : *recommandé* = deux PR, le flou d'abord (PR L, la tuyauterie de capture), puis le verre (PR M).
   Autre possibilité : une seule PR.
2. **Capture** : *recommandé* = une capture et une pyramide **par segment**, partagées par tous les effets du
   segment (leçon de VekaUI 4a), en copiant seulement l'union de leurs rectangles. Autre possibilité : une capture
   par effet, plus simple mais plus chère quand il y a beaucoup de verre.
3. **Noms** : *recommandé* = ceux de VekaUI (`backdrop_blur`, `frosted_glass`, `glass`, `glass_group`,
   `liquid_distortion`, en `camelCase` dans le `.vk`), avec des matériaux prêts `regular` / `clear` et
   `.clarity` / `.tint` / `.interactive` comme chez Apple. Autre possibilité : les noms d'Apple (`glassEffect`,
   `GlassEffectContainer`).
4. **Forme du verre** : *recommandé* = celle du fond ou du découpage de la vue, sinon `in: Shape` ; toutes les formes
   SDF de Veka. Autre possibilité : rectangles arrondis seulement, comme VekaUI.
5. **Reflet et pointeur** : *recommandé* = reflet éclairé par le pointeur pour `interactive`, lumière fixe en haut à
   gauche sinon ; une image redessinée seulement si le pointeur bouge au-dessus d'un verre. Autre possibilité :
   toujours une lumière fixe.
6. **Fonds du système** (Mica, Acrylic, vibrancy macOS) : *recommandé* = dans la PR M, sur Windows essayé sur le PC,
   et sur macOS écrit puis essayé sur le Mac avec le reste de l'étape 2. Autre possibilité : les remettre à plus
   tard.
7. **Repli** : *recommandé* = qualité réglable sur l'app et respect de « Réduire la transparence » de l'OS (teinte
   unie). Autre possibilité : toujours le verre, sans réglage.
8. **Rendu de référence** : *recommandé* = l'aspect de la version 27 (§2.1 : bord assombri, reflet vif, lumière
   interne, curseur de clarté, ombre), réglé à l'œil faute de chiffres publiés. Autre possibilité : reprendre tel
   quel le shader de VekaUI (plus proche de 26).
9. **Hors étape 6** : les modes de fusion (Multiply, Screen… qui lisent aussi l'arrière-plan), la texture
   d'arrière-plan fournie par un moteur (Unreal, Unity) et la vue `Image`. *Recommandé* = les modes de fusion juste
   après (ils réutilisent la capture), le reste avec les liaisons des moteurs.
