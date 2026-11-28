# Veka : étape 8, modes de fusion

Statut : **validé** (les 6 choix recommandés) ; PR P faite. Fait suite à `VEKA_ETAPE7.md`. Reprend le choix 9 de `VEKA_ETAPE6.md` :
« les modes de fusion juste après, ils réutilisent la capture ».

## 1. Objectif

Une vue peut se **mélanger** avec ce qui est dessiné derrière elle, comme dans SwiftUI, Figma ou Photoshop :
produit (Multiply), écran (Screen), incrustation (Overlay), différence…

```rust
// Rust
Circle().fill(Color::RED).blend_mode(BlendMode::Multiply)
Text("Titre").font(Font::LargeTitle).blend_mode(BlendMode::Difference)

ZStack![a, b].compositing_group().opacity(0.5)   // le groupe devient transparent d'un bloc
```

```swift
// .vk : mêmes noms qu'en SwiftUI
Circle().fill(.red).blendMode(.multiply)
ZStack { … }.compositingGroup().opacity(0.5)
```

```c
/* C */
veka_blend_mode(element, VEKA_BLEND_MULTIPLY);
veka_compositing_group(element);
```

## 2. Comment ça marche aujourd'hui

- La liste de dessin est coupée en **segments** par les commandes `Backdrop` (verre, flou). À la fin d'un segment,
  le renderer copie la zone dessinée dans une texture (la **capture**), puis le verre la lit.
- `PushLayer { opacity }` ne crée **pas** de calque : l'opacité est multipliée dans chaque commande. Deux formes
  qui se chevauchent dans un groupe à 50 % laissent donc voir leur recouvrement.
- La surface est en sRGB et le GPU mélange en lumière **linéaire** (reste connu : un noir à 60 % paraît plus pâle
  que chez Apple).

## 3. Fonctionnement proposé

### 3.1 Un vrai calque

Nouvelle commande `PushGroup { blend: BlendMode, opacity: f32 }` … `PopGroup`. Le renderer :

1. termine le segment courant et **capture** la zone du groupe (ce qui est derrière) ;
2. dessine le sous-arbre dans une **texture à part**, transparente, à la taille de sa boîte (bornée à la fenêtre) ;
3. **compose** cette texture sur la fenêtre avec un shader qui lit la capture et applique la formule du mode, puis
   l'opacité du groupe.

Les groupes s'imbriquent (pile de textures, réutilisées d'une image à l'autre). Un groupe en mode normal avec une
opacité de 1 ne crée rien : aucun coût pour qui ne s'en sert pas.

`blend_mode(…)` et `compositing_group()` créent tous les deux ce calque. Avec `compositing_group().opacity(x)`,
l'opacité s'applique au groupe entier (les recouvrements ne se voient plus), comme dans SwiftUI. `opacity(x)`
seul garde le comportement actuel, plus rapide.

### 3.2 Les modes

Les formules de la norme W3C *Compositing and Blending*, celles de SwiftUI, CSS et Figma :

| Famille | Modes |
|---|---|
| Assombrir | `multiply`, `darken`, `colorBurn`, `plusDarker` |
| Éclaircir | `screen`, `lighten`, `colorDodge`, `plusLighter` |
| Contraste | `overlay`, `softLight`, `hardLight` |
| Différence | `difference`, `exclusion` |
| Composantes | `hue`, `saturation`, `color`, `luminosity` |
| Découpe (dans un groupe) | `sourceAtop`, `destinationOver`, `destinationOut` |

`normal` reste le mode par défaut. Les modes de découpe n'ont de sens qu'à l'intérieur d'un `compositing_group`
(par exemple percer un texte dans un fond avec `destinationOut`).

### 3.3 Espace de couleur

Le mélange se fait sur les valeurs **sRGB** (non linéaires), comme SwiftUI, CSS et Photoshop : un `multiply` donne
exactement les mêmes couleurs que dans Figma. Le shader décode la capture, applique la formule en sRGB et
réencode. La lumière linéaire du reste du dessin ne change pas dans cette étape.

### 3.4 Avec le verre

Un verre ou un flou à l'intérieur d'un groupe lit le **contenu du groupe** (sa texture), pas la fenêtre. Un groupe
à l'intérieur d'un verre se mélange avec ce qui est déjà dessiné, verre compris. Le hit-test ne change pas.

### 3.5 Coût

Une capture et une texture par groupe visible. Sur une fenêtre de 900 × 560, une dizaine de groupes restent bien
en dessous d'une milliseconde de GPU. Pas de seuil : on mesure sur l'exemple et on l'ajoute au test de mesure.

## 4. Exemple et tests

- Exemple `fusion` (Rust, `.vk` et C) : une grille d'un même motif sur un fond en dégradé, un mode par case, avec
  son nom. Un bouton fait tourner le motif pour voir les modes en mouvement.
- Tests : les formules de chaque mode comparées à des valeurs de référence calculées à la main (côté processeur,
  même code que le shader) ; une capture de la grille comparée à une image de référence avec une tolérance ; le
  dessin `.vk` identique au dessin Rust.

## 5. Plan

- **PR P, le calque et les modes** : `PushGroup`/`PopGroup`, textures de calque, shader de composition, les 21
  modes, `blend_mode` et `compositing_group` en Rust et en `.vk`, l'exemple Rust et `.vk`, les tests ; essais sur
  le PC et le Mac.
  - Fait. Écarts : un groupe normal et opaque ne crée un calque que s'il isole quelque chose (un mode ou un
    verre à l'intérieur) ; les calques font la taille de la fenêtre. Le test d'image compare des pixels choisis
    (case percée par `destinationOut`, formule de chaque mode) plutôt qu'une image de référence entière.
- **PR Q, les liaisons** : C, C++ et C# (`veka_blend_mode`, `veka_compositing_group`, l'énumération), exemple C,
  en-tête vérifié par la CI.

## 6. Choix à valider

1. **Portée d'un mode** : *recommandé* = `blend_mode` mélange le sous-arbre **d'un bloc** (calque), comme SwiftUI.
   Autre possibilité : chaque forme se mélange séparément (sans calque, plus rapide, mais une vue faite de
   plusieurs formes se mélange aussi avec elle-même).
2. **Modes** : *recommandé* = les 21 de SwiftUI d'un coup (un seul shader). Autre possibilité : seulement les plus
   courants (multiply, screen, overlay, darken, lighten, plusLighter) d'abord.
3. **Espace de couleur** : *recommandé* = mélange en sRGB, comme SwiftUI et Figma. Autre possibilité : en lumière
   linéaire, comme le reste du rendu (couleurs différentes de celles des outils de dessin).
4. **`compositing_group`** : *recommandé* = l'ajouter dans la même étape (même calque, et il corrige l'opacité des
   groupes qui se chevauchent). Autre possibilité : plus tard.
5. **Découpage** : *recommandé* = deux PR, le moteur et Rust/`.vk` d'abord (PR P), les liaisons C/C++/C# ensuite
   (PR Q). Autre possibilité : une seule PR.
6. **Les petits restes** (bord sombre du verre, ondulation par à-coups, opacité mélangée en linéaire, barre de
   titre macOS en mode clair forcé, marge transparente qui capte les clics) : *recommandé* = après cette étape,
   dans une PR de corrections. Autre possibilité : les régler d'abord.
