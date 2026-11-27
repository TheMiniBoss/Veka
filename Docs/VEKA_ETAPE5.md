# Veka : étape 5, le langage `.vk`

Statut : **validé** (les 8 choix recommandés) ; PR J faite, PR K à venir. Fait suite à `VEKA_ETAPE4.md`. Reprend le langage de VekaUI (`VekaLang` : `view`, `@State`,
`@Param`, `@Slot`, `@Event`, `model`, `ForEach`, `withAnimation`, `emit`) et la leçon des feux d'artifice (écrits
en C++ faute de `random`, `cos` et d'horloge dans le langage). On garde la conception et la syntaxe, pas le code.

## 1. Objectif

Un fichier `.vk` décrit des vues **sans autre langage**, et tout ce que Veka sait faire en Rust ou en C s'y écrit :
layout, Colora, Anima, fenêtres. Veka lit le fichier au lancement, construit les mêmes `Element` que le code Rust
(par le schéma), et **recharge le fichier à chaud** quand on l'enregistre, sans perdre l'état.

```swift
// compteur.vk
view Compteur {
    @State var clics: Int = 0

    VStack(spacing: 16) {
        Text("Bonjour Veka")
            .font(.title)
        Button("Clics : \(clics)") {
            withAnimation(.snappy) { clics += 1 }
        }
        if clics > 3 {
            Text("Bravo !")
                .foregroundColor(.Green)
                .transition(.scale(0.8).combined(.opacity))
        }
    }
    .padding(24)
}
```

```rust
// Rust : une fenêtre dont la vue vient d'un fichier.
App::new().window_vk("compteur.vk", "Compteur").run()
```

```c
/* C : pareil, et le C reçoit les événements du fichier. */
veka_app_add_window_vk(app, "Compteur", 480, 320, "compteur.vk", "Compteur", on_event, user);
```

**Critères d'acceptation**

1. Les exemples `compteur`, `colora` et `anima` réécrits en `.vk` donnent **la même liste de dessin** que leurs
   versions Rust (test automatique, comme le schéma à l'étape 1).
2. Des feux d'artifice écrits **100 % en `.vk`** (`ForEach` sur une plage, `random(seed)`, `sin` / `cos`, une
   horloge), fluides sur le PC.
3. Enregistrer le fichier pendant que l'app tourne met la fenêtre à jour en moins d'une image utile, en gardant les
   états dont le nom et le type n'ont pas changé. Une erreur affiche un bandeau (ligne:colonne, message) au lieu de
   fermer l'app, et l'ancienne vue reste là.
4. Un `.vk` envoie des événements au Rust ou au C (`emit`) et lit des données de l'hôte (`model`).

## 2. Le langage

### 2.1 Vues et composants

```swift
view Article {
    @Param var titre: String = "Article"
    @Param var prix: Int = 10
    @Slot badge
    @Event acheter(Int)

    HStack(spacing: 10) {
        Text(titre).font(.headline)
        Slot(badge)
        Spacer()
        Button("Acheter \(prix) or") { emit acheter(prix) }
    }
}

view Boutique {
    @State var dernier: String = ""

    VStack(spacing: 8, alignment: .leading) {
        ForEach(model.articles) { a in
            Article(titre: a.nom, prix: a.prix) {
                if a.nouveau { Text("NOUVEAU").font(.caption2) }
            }
            .on(acheter) { prix in dernier = a.nom }
            .id(a.id)
        }
        Text("Dernier achat : \(dernier)")
    }
}
```

- `view Nom { déclarations ; corps }` : une vue = une fonction de Veka (un `component`), ses `@State` sont des
  `State` (retrouvés par **nom**, pas par ordre : un `.vk` peut déclarer ses états où il veut, et le rechargement à
  chaud les garde).
- `@Param` (avec défaut), `@Slot` (des enfants donnés par l'appelant), `@Event` (ce que la vue envoie) :
  une vue d'un fichier s'utilise comme un composant de Veka, depuis un autre `.vk`, depuis Rust et depuis le C.
- Plusieurs vues par fichier ; `import "boutons.vk"` pour en partager.

### 2.2 Composants, modifiers et valeurs : le schéma

Le parseur ne connaît **aucun** composant : `VStack`, `.padding`, `.matchedGeometryEffect`… sont cherchés dans le
schéma de Veka (et dans les entrées que l'app y ajoute). Un composant Rust enregistré dans le schéma s'utilise donc
en `.vk` sans rien écrire de plus. Les noms sont ceux du schéma (camelCase, comme SwiftUI).

- Arguments : le premier sans nom (`Text("a")`, `.padding(24)`), les autres nommés (`spacing: 16`).
- Énumérations : `.title`, `.leading`, `.dark`.
- Couleurs et peintures : `#FF6A00`, `#FF6A0080`, `rgb(1, 0.4, 0)`, jetons Colora `.Accent`, `.Blue.secondary`,
  dégradés `LinearGradient([.Blue, .Teal], from: .topLeading, to: .bottomTrailing)`.
- Animations : `.smooth`, `.bouncy`, `.spring(response: 0.5, damping: 0.7)`, `.linear(1).delay(0.2).repeatForever()`.
- Transitions : `.opacity`, `.scale(0.8)`, `.move(.top).combined(.opacity)`, `.asymmetric(insertion:, removal:)`.

### 2.3 Expressions

Littéraux, variables, opérateurs (`+ - * / % == != < <= > >= && || !`), ternaire `a ? b : c`, interpolation
`"\(x) or"`, membres `a.nom`, tableaux `[1, 2, 3]`, plages `0..<10` et `1...5`, enregistrements `{ x: 1, y: 2 }`.

Bibliothèque (la leçon des feux d'artifice) :

- maths : `sin`, `cos`, `tan`, `atan2`, `abs`, `sqrt`, `pow`, `exp`, `min`, `max`, `clamp`, `lerp`, `floor`,
  `round`, `pi` ;
- hasard **déterministe** : `random(seed)` (0 à 1) et `random(seed, min, max)`, même suite partout ;
- temps : `@Clock var t` (secondes depuis l'apparition de la vue, qui avance à chaque image **seulement** quand la vue
  lit `t`, comme `TimelineView`) ;
- `len(tableau)`, `format("%.1f", x)`, `String(x)`.

Fonctions de l'utilisateur, pures, au niveau du fichier :

```swift
func etincelle(i: Int, t: Float) -> Float { sin(t * 3 + Float(i)) * 0.5 + 0.5 }
```

### 2.4 Contrôle et actions

- Dans un corps : `if / else`, `ForEach(collection) { e in … }` ou `{ e, i in … }`, `let x = …`.
- Dans une action (`Button { … }`, `.onTapGesture { … }`, `.on(evenement) { … }`, `.onAnimationEnd { … }`) :
  affectation (`=`, `+=`…), `x.toggle()`, `withAnimation(.bouncy) { … }`, `emit evenement(valeur)`, contrôle des
  timelines (`pulsation.play()`, `.pause()`, `.reverse()`, `.seek(0.5)`), tableaux d'état (`liste.append(x)`,
  `liste.remove(at: 0)`, `liste.insert(x, at: 0)`).

### 2.5 Timelines et thèmes dans le fichier

```swift
timeline Pulsation(loop: true, length: 0.9) {
    track echelle { 0: 1 .easeOut ; 0.15: 1.35 ; 0.5: 1 }
    event 0.15 battement
}

theme Marque {
    Tint: #FF6A00 / #FF8A33          // clair / sombre
    Marque.Fond: LinearGradient([#FF6A00, #FF2D55], from: .leading, to: .trailing)
}

view Coeur {
    @Timeline var p = Pulsation(autoplay: true)
    Circle().fill(.Pink).frame(28, 28).scaleEffect(p.echelle)
        .onTimelineEvent(p, battement) { … }
}
```

## 3. L'hôte : Rust et C

- `veka-lang` lit un fichier (ou une chaîne) et donne un **module** : ses vues, fonctions, timelines, thèmes et ses
  erreurs (avec positions).
- Rust : `App::window_vk(chemin, vue)`, `vk::Module::load(chemin)?.view("Boutique")` qui renvoie un `Element`
  utilisable partout, `.model(valeur)` pour les données de l'hôte, `.on_event(|nom, valeur| …)`.
- Données de l'hôte : une `Value` (nombres, textes, tableaux, enregistrements) que le Rust ou le C met à jour ;
  les vues qui lisent `model` sont ré-évaluées, et c'est tout.
- C : `veka_vk_load`, `veka_vk_view`, `veka_vk_set_model` (des `VekaValue`, plus tableaux et enregistrements),
  `veka_vk_on_event`, `veka_app_add_window_vk`. Les erreurs passent par `veka_last_error`.
- **Rechargement à chaud** : veka-app surveille les fichiers chargés (Windows : `ReadDirectoryChangesW`,
  macOS : FSEvents, en dehors du cœur) et recharge le module ; les états gardent leur valeur quand leur nom et leur
  type sont les mêmes.

## 4. Fonctionnement

| Pièce | Rôle |
|---|---|
| lexer, parseur (écrits à la main) | texte → AST avec positions ; erreurs qui ne bloquent pas tout le fichier |
| imprimeur | AST → texte canonique ; `parse(print(ast)) == ast` est testé (prépare l'éditeur) |
| analyse | contre le schéma : composant, modifier, paramètre, type, jeton Colora, état inconnus ; « vouliez-vous dire » |
| interpréteur | une vue `.vk` est un `component` dont le corps parcourt l'AST : il lit les états, évalue les expressions et appelle le schéma ; les actions deviennent des `Action` |

- Pas de bytecode au départ : l'arbre n'est évalué qu'au changement d'état (ou à chaque image pour une vue qui lit
  une horloge), comme le code Rust.
- `veka-lang` est dans le **cœur** (aucun appel système, compile en WebAssembly) ; seule la surveillance des
  fichiers est dans veka-app.
- Les vues `.vk` passent toutes par le schéma : un `.vk` n'a rien de plus ni de moins que le C.

## 5. Plan

- **PR J** :
  - `veka-lang` : lexer, parseur, AST, imprimeur, analyse ;
  - interpréteur : vues, `@State`, expressions, `if`, `ForEach`, actions, `withAnimation` ;
  - `App::window_vk`, rechargement à chaud et bandeau d'erreur ;
  - `compteur.vk`, `colora.vk` et `anima.vk`, avec les tests « même liste de dessin » ;
  - essais sur le PC.
  - Fait. Écarts : les thèmes dans le fichier sont arrivés dès la PR J ; `.blue` (minuscule) est la couleur
    fixe `Color::Blue`, `.Blue` le jeton ; `.colorScheme(.system)` suit le parent ; le rechargement se règle
    sur l'app (`App::hot_reload(false)`) ; la pulsation en timeline d'`anima.vk` attend la PR K.
- **PR K** :
  - composants (`@Param`, `@Slot`, `@Event`, `.on`), `import`, `model` ;
  - fonctions de l'utilisateur, bibliothèque maths, `random`, `@Clock` ;
  - timelines et thèmes dans le fichier ;
  - interface C et exemple C ;
  - les feux d'artifice en `.vk` ;
  - essais sur le PC.

## 6. Choix (validés)

1. **Syntaxe** : *recommandé* = celle de VekaUI, proche de SwiftUI (`view`, `@State`, modifiers en `.nom(…)`), en
   gardant les fichiers de VekaUI lisibles à quelques noms près. Autre possibilité : un format de données (JSON,
   KDL), plus simple à lire pour une machine mais pénible à écrire à la main.
2. **Exécution** : *recommandé* = un interpréteur d'arbre, évalué seulement au changement d'état, avec le parseur
   **embarqué** dans le runtime (Veka lit des `.vk` au lancement, et le rechargement à chaud en a besoin). Autre
   possibilité : compiler les `.vk` en Rust au build (`build.rs`) ; c'est plus rapide, mais sans rechargement à chaud
   ni chargement par le C.
3. **États retrouvés par nom** dans un `.vk` (et non par ordre comme en Rust) : *recommandé*, pour que le
   rechargement à chaud les garde et qu'on puisse les déclarer dans un `if`. Autre possibilité : l'ordre, comme en
   Rust.
4. **Types** : *recommandé* = typage léger vérifié au chargement (types des `@State` et `@Param` déclarés,
   expressions contrôlées contre le schéma, `Int` et `Float` distincts mais convertis implicitement de `Int` vers
   `Float`). Autre possibilité : tout dynamique, avec les erreurs seulement à l'exécution.
5. **Le langage peut tout faire** : *recommandé* = maths, `random(seed)` déterministe, `@Clock`, fonctions pures de
   l'utilisateur et `ForEach` sur des plages dès l'étape 5 (PR K). Pas de boucles `while`, ni d'effets de bord hors
   des actions : un corps de vue reste une description. Autre possibilité : seulement des vues statiques et des
   états, la logique restant en Rust ou en C.
6. **Données de l'hôte** : *recommandé* = un `model` (une `Value` en arbre) que l'hôte remplace ou modifie, et des
   `@Event` que l'hôte reçoit, comme dans VekaUI. Autre possibilité : laisser l'hôte lire et écrire directement les
   `@State` du fichier.
7. **Rechargement à chaud** : *recommandé* = actif par défaut en debug, désactivable (`Window::hot_reload(false)`),
   absent en release sauf demande. Autre possibilité : jamais par défaut.
8. **Hors étape 5** : la vue `Canvas` (dessin libre en `.vk`), l'éditeur et le serveur de langage (complétion,
   erreurs dans l'éditeur de code), la localisation des textes, le format binaire précompilé. *Recommandé* = les
   garder pour après Liquid Glass.
