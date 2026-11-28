# Étape 10 : défilement (`ScrollView`) et texte sur plusieurs lignes

Statut : **validé** (choix 2 : option B ; les autres, recommandés) ; PR V faite.

## 1. Objectif

Aujourd'hui, une vue plus grande que sa fenêtre est coupée, et la molette ne fait rien. Cette étape ajoute le
défilement, avec les mêmes noms que SwiftUI, puis le champ de texte sur plusieurs lignes, qui en a besoin :

```rust
// Rust
ScrollView![
    VStack![ForEach(articles, |a| ligne(a))],
]
.scroll_indicators(Visibility::Automatic)

ScrollViewReader(|proxy| {
    VStack![
        Button("Haut", move || proxy.scroll_to("haut", UnitPoint::TOP)),
        ScrollView![contenu()],
    ]
})

TextEditor(notes)          // State<String>, plusieurs lignes
```

```swift
// .vk : mêmes noms qu'en SwiftUI
ScrollView(.vertical) {
    VStack { … }
}
.scrollIndicators(.hidden)

TextEditor(text: $notes)
```

## 2. Comment ça marche aujourd'hui

- La mise en page suit SwiftUI : le parent propose une taille, et `None` veut dire « ta taille idéale ». Une
  `ScrollView` n'a donc qu'à proposer `None` sur l'axe qui défile.
- Le dessin sait déjà couper (`PushClip`) et déplacer (`PushTransform`), et les animations (ressorts, courbes)
  existent depuis l'étape 4.
- La plateforme n'envoie pas encore la molette ni les gestes du pavé tactile.

## 3. Fonctionnement proposé

### 3.1 Événements de la plateforme

- `Scroll { position, delta, precise, phase }` :
  - `delta` en points ;
  - `precise` : pavé tactile ou souris à défilement fin (sinon des crans de molette) ;
  - `phase` : début, suite et fin du geste (l'élan du système est ignoré : Veka calcule le sien).

| | Windows | macOS |
|---|---|---|
| Molette | `WM_MOUSEWHEEL` / `WM_MOUSEHWHEEL` (crans de 120) | `scrollWheel:` |
| Pavé tactile | `WM_MOUSEWHEEL` en petits pas (pavé de précision) | `scrollingDeltaX/Y`, `hasPreciseScrollingDeltas`, `phase`, `momentumPhase` |
| Maj + molette | défilement horizontal | défilement horizontal (fait par le système) |

### 3.2 `ScrollView`

- Elle défile sur un axe (`.vertical` par défaut, `.horizontal`) ou les deux, et coupe ce qui dépasse.
- Le contenu garde sa taille idéale sur l'axe qui défile, et prend la largeur (ou la hauteur) proposée sur
  l'autre axe, comme SwiftUI.
- Une `ScrollView` dans une autre défile en premier, puis passe le reste du geste à celle du dessus quand elle
  arrive au bout.
- Le défilement est le même sur Windows et sur macOS (choix 2, option B) :
  - **pavé tactile** : le contenu suit le doigt. Quand on lâche, l'élan est calculé par Veka (une
    décélération, comme sur iOS et macOS), et non pris au système, pour être identique partout ;
  - **molette** : chaque cran défile en douceur sur les deux systèmes (environ 3 lignes, animé sur
    150 ms environ). Des crans rapides s'enchaînent et accélèrent. Sur macOS aussi, où la molette d'une
    souris classique saute d'habitude par crans ;
  - **bords élastiques partout** : le contenu dépasse un peu, de plus en plus difficilement, puis revient avec
    un ressort. Avec la molette, le bord ne se tire pas, mais un défilement rapide qui arrive au bout rebondit
    légèrement ;
  - limite sur Windows : un pavé tactile de précision envoie déjà son propre élan sous forme de molette, sans
    dire quand le doigt se lève. Veka le suit tel quel, et l'élastique s'y applique quand même. Faire mieux
    demanderait DirectManipulation, plus lourd, qui pourrait venir plus tard ;
  - `.scroll_bounce(Never)` coupe l'élastique (comme `.scrollBounceBehavior` de SwiftUI).
- Au clavier : quand le focus passe (Tab) à un champ caché, la vue défile pour le montrer. Page préc. et Page
  suiv., Origine et Fin font défiler la `ScrollView` qui contient le focus, ou celle sous le pointeur.

### 3.3 Barres de défilement

- Des barres fines qui apparaissent pendant le défilement et disparaissent après une seconde, comme sur macOS
  et Windows 11. Elles s'élargissent au survol, et on peut les attraper et les glisser.
- `.scroll_indicators(.automatic | .visible | .hidden)`, comme `.scrollIndicators` de SwiftUI.

### 3.4 Code

- `ScrollViewReader` et `proxy.scroll_to(id, ancre)` défilent jusqu'à la vue qui porte cet `.id(…)`, avec
  l'animation en cours (`with_animation`).
- `.scroll_disabled(true)` empêche de défiler.
- `.on_scroll_offset_change(|décalage| …)` : le décalage actuel, pour une barre de titre qui change en
  défilant par exemple (sous SwiftUI, c'est `onScrollGeometryChange`).

### 3.5 `TextEditor`

- Un texte sur plusieurs lignes, lié à un `State<String>`. Il reprend l'éditeur des champs (PR U), avec
  Entrée pour aller à la ligne, les lignes qui se coupent à la largeur du champ, ↑ et ↓, et le défilement de
  sa propre `ScrollView` qui suit le curseur.

### 3.6 `.vk` et C

- En `.vk` : `ScrollView`, `.scrollIndicators`, `.scrollDisabled`, `ScrollViewReader` et `TextEditor`.
- En C : `veka_scroll_view`, `veka_scroll_indicators`, `veka_scroll_to` et `veka_text_editor`.

## 4. Exemple et tests

- Exemple `liste` en Rust, en `.vk` et en C : une longue liste de lignes, un bouton « Haut » qui remonte, une
  barre de titre qui change en défilant, et une zone de notes (`TextEditor`).
- Tests sans fenêtre (`Surface` reçoit des défilements simulés) :
  - molette (douce, avec des crans qui s'enchaînent) et pavé tactile, bords, élan et ressort, avec l'horloge
    des tests ;
  - `ScrollView` imbriquées ;
  - `scroll_to`, et le focus qui fait défiler ;
  - barres : apparition, glisser ;
  - `TextEditor` : lignes, ↑ ↓, défilement qui suit le curseur ;
  - le dessin `.vk` identique au Rust.
- Essais sur le PC (molette, pavé tactile) et sur le Mac (pavé tactile, élan, bords élastiques).

## 5. Plan

- **PR V, le défilement** : événements de la plateforme sur Windows et macOS, `ScrollView`, barres,
  `ScrollViewReader`, le focus qui fait défiler, `.vk`, C, exemple `liste` et tests.
  - Fait. Écarts :
    - une `ScrollView` imbriquée prend les gestes dans son axe ; elle ne passe pas le reste du geste à celle
      du dessus en arrivant au bout (elle rebondit, comme sur macOS) ;
    - `scroll_to` animé glisse en douceur, sans suivre la courbe exacte du `with_animation` ;
    - Page préc. / suiv., Origine et Fin vont à la vue qui contient le focus, ou sans focus à la plus
      profonde (pas encore celle sous le pointeur) ;
    - `.on_scroll_offset_change` existe en Rust seulement, et l'exemple n'a pas de barre de titre qui
      change en défilant ;
    - tout le contenu est dessiné, même hors de la vue (le découpage suffit jusqu'à quelques milliers de
      lignes ; `LazyVStack` viendra ensuite).
- **PR W, `TextEditor`** : texte sur plusieurs lignes, en Rust, `.vk` et C, ajouté à l'exemple.

## 6. Choix à valider

1. **Contenu de l'étape** :
   - *recommandé* : `ScrollView`, molette et pavé tactile, barres, `scroll_to`, puis `TextEditor` ;
   - autre possibilité : `ScrollView` seule, et `TextEditor` plus tard.
2. **Sensation du défilement** : **validé, option B**. La même partout : bords élastiques, élan calculé par
   Veka, et une molette douce sur Windows comme sur macOS.
3. **Barres de défilement** :
   - *recommandé* : fines, qui apparaissent en défilant et qu'on peut glisser (comme macOS et Windows 11) ;
   - autre possibilité : toujours visibles, ou pas de barres pour commencer.
4. **Listes très longues** :
   - *recommandé* : pour l'instant, la `ScrollView` construit tout son contenu, ce qui suffit jusqu'à
     quelques milliers de lignes. `LazyVStack` et `List`, qui ne construisent que les lignes visibles,
     viendraient à l'étape suivante ;
   - autre possibilité : `LazyVStack` dès maintenant.
5. **API** :
   - *recommandé* : les noms et le comportement de SwiftUI (`ScrollView`, `ScrollViewReader`,
     `.scrollIndicators`, `.scrollDisabled`, `TextEditor`) ;
   - autre possibilité : une API Veka plus simple (un `offset` lié à un état).
6. **Découpage** :
   - *recommandé* : deux PR, le défilement (PR V) puis `TextEditor` (PR W) ;
   - autre possibilité : une seule PR.
