# Étape 9 : clavier, focus et saisie de texte

Statut : conception, à valider.

## 1. Objectif

Aujourd'hui, Veka ne reçoit que la souris : pas de touche, pas de champ de texte. Cette étape ajoute ce qu'il
faut pour un vrai formulaire, avec les mêmes noms que SwiftUI :

```rust
// Rust
let nom = state(String::new());
TextField("Nom", nom.binding())
    .focused(focus.binding(), Champ::Nom)
    .on_submit(|| valider())

SecureField("Mot de passe", mdp.binding())
Button("Annuler", annuler).keyboard_shortcut(Key::Escape)
VStack![…].on_key_press(Key::Character('r'), |_| { recharger(); KeyPress::Handled })
```

```swift
// .vk : mêmes noms qu'en SwiftUI
@State var nom = ""
@FocusState var focus: Champ?
TextField("Nom", text: $nom)
    .focused($focus, equals: .nom)
    .onSubmit { valider() }
```

## 2. Comment ça marche aujourd'hui

- La plateforme envoie `PointerMoved`, `PointerButton` et `PointerLeft`, mais aucun événement clavier.
- Le texte est mis en page par parley 0.13 (déjà une dépendance), qui contient aussi un éditeur (`PlainEditor`) :
  curseur, sélection, déplacement par mot et par ligne, texte en cours de composition (IME).

## 3. Fonctionnement proposé

### 3.1 Événements de la plateforme

- `Key { key, code, modifiers, pressed, repeat }` :
  - `key` est la touche logique (`Character('a')`, `Enter`, `ArrowLeft`…), selon la disposition du clavier ;
  - `code` est la touche physique, utile pour les jeux et les raccourcis.
- `Text { text }` : le texte tapé, après les touches mortes (`^` puis `e` donne `ê`).
- `Ime { Preedit { text, cursor } | Commit { text } }` : la composition du japonais, du chinois, du coréen…
- La plateforme reçoit en retour la zone du curseur (`set_ime_area`), pour placer la fenêtre de l'IME juste
  sous le texte.

| | Windows | macOS |
|---|---|---|
| Touches | `WM_KEYDOWN` / `WM_KEYUP` (et `SYS`) | `keyDown:` / `keyUp:` / `flagsChanged:` |
| Texte | `WM_CHAR` (paires UTF-16) | `insertText:` de `NSTextInputClient` |
| IME | IMM32 (`WM_IME_COMPOSITION`) | `setMarkedText:` de `NSTextInputClient` |
| Presse-papiers | `CF_UNICODETEXT` | `NSPasteboard` |

### 3.2 Focus

- Une seule vue a le focus dans une fenêtre. Un clic sur une vue qui peut l'avoir le lui donne, et un clic
  ailleurs le retire.
- Tab et Maj+Tab passent à la vue suivante ou précédente, dans l'ordre de lecture (de haut en bas, puis de
  gauche à droite).
- `@FocusState` et `.focused($focus, equals: valeur)` lisent et donnent le focus depuis le code, comme SwiftUI.
- Pour l'instant, seuls les champs de texte prennent le focus. Les boutons le prendront avec l'accessibilité,
  comme sur macOS par défaut.
- La vue qui a le focus est entourée d'un anneau à la couleur d'accent, comme sur macOS.

### 3.3 Touches

- `.on_key_press(touche, action)` : l'action reçoit la touche et renvoie `Handled` ou `Ignored`. La touche
  remonte de la vue qui a le focus jusqu'à la racine, et s'arrête à la première vue qui la traite.
- `.keyboard_shortcut(touche, modificateurs)` sur un bouton le déclenche de n'importe où dans la fenêtre :
  Entrée pour le bouton par défaut, Échap pour annuler, ⌘S ou Ctrl+S…
- `Modifiers::COMMAND` vaut ⌘ sur le Mac et Ctrl sur Windows, pour écrire un seul raccourci pour les deux.

### 3.4 Champs de texte

- `TextField` (une ligne) et `SecureField` (points à la place des caractères, sans copier).
- Le champ est lié à une chaîne (`Binding<String>`), et `on_submit` est appelé à l'appui sur Entrée.
- L'édition passe par le `PlainEditor` de parley. Il gère :
  - le curseur qui clignote et la sélection, à la souris (glisser, double-clic pour un mot) et au clavier
    (Maj+flèches, Ctrl ou ⌥ pour aller de mot en mot, Origine et Fin) ;
  - copier, couper, coller, tout sélectionner et annuler (Ctrl+Z ou ⌘Z) ;
  - la composition IME, soulignée dans le champ.
- Le texte qui dépasse défile pour que le curseur reste visible.
- Style : celui de SwiftUI sur la plateforme (`.textFieldStyle(.roundedBorder)` par défaut), et `.plain` pour
  un champ sans cadre.

### 3.5 `.vk` et C

- En `.vk` : `TextField`, `SecureField`, `@FocusState`, `.focused`, `.onSubmit`, `.onKeyPress` et
  `.keyboardShortcut`.
- En C : `veka_text_field`, `veka_secure_field`, `veka_on_submit`, `veka_on_key_press`, `veka_keyboard_shortcut`
  et `veka_focus` / `veka_set_focus`.

## 4. Exemple et tests

- Exemple `formulaire` en Rust, en `.vk` et en C : nom, e-mail, mot de passe, Tab pour passer d'un champ à
  l'autre, Entrée pour valider, Échap pour effacer.
- Tests sans fenêtre (`Surface` reçoit des touches simulées) :
  - frappe, flèches, sélection et effacement ;
  - Tab et `@FocusState` ;
  - remontée des touches et raccourcis ;
  - composition IME simulée ;
  - le dessin `.vk` identique au Rust.
- Essais sur le PC et le Mac : clavier français AZERTY, touches mortes, IME japonais, copier-coller avec une
  autre app.

## 5. Plan

- **PR T, le clavier et le focus** :
  - événements de touche et de texte sur Windows et macOS, et modificateurs ;
  - focus, Tab, `@FocusState` / `.focused`, `on_key_press` et `keyboard_shortcut` ;
  - Rust et `.vk`, avec tests.
- **PR U, les champs de texte** :
  - `TextField` et `SecureField` avec le `PlainEditor` de parley ;
  - IME sur Windows (IMM32) et macOS (`NSTextInputClient`), et presse-papiers ;
  - liaisons C, exemple `formulaire` en Rust, `.vk` et C.

## 6. Choix à valider

1. **Contenu de l'étape** :
   - *recommandé* : clavier, focus et champs d'une ligne (`TextField`, `SecureField`) ;
   - autre possibilité : ajouter aussi le texte sur plusieurs lignes (`TextEditor`), qui demande le défilement.
     Il viendrait plutôt avec `ScrollView` et la molette, à l'étape suivante.
2. **Moteur d'édition** :
   - *recommandé* : le `PlainEditor` de parley, déjà dans les dépendances et éprouvé (curseur, sélection, IME) ;
   - autre possibilité : un éditeur écrit pour Veka (plus de code et plus de bugs à trouver).
3. **IME sur Windows** :
   - *recommandé* : IMM32 d'abord, simple et suffisant pour les IME japonais, chinois et coréen ;
   - autre possibilité : TSF tout de suite. C'est plus moderne (dictée, écriture manuscrite), mais bien plus
     lourd. Il peut venir plus tard sans changer l'API.
4. **Focus des boutons** :
   - *recommandé* : seuls les champs prennent le focus (comme macOS par défaut), et les boutons ont leurs
     raccourcis ;
   - autre possibilité : tous les contrôles le prennent dès maintenant (comme Windows).
5. **API** :
   - *recommandé* : les noms et le comportement de SwiftUI (`@FocusState`, `.focused`, `.onSubmit`,
     `.onKeyPress`, `.keyboardShortcut`) ;
   - autre possibilité : une API Veka plus simple (`.on_key`), moins proche de SwiftUI.
6. **Découpage** :
   - *recommandé* : deux PR, le clavier et le focus (PR T), puis les champs de texte (PR U) ;
   - autre possibilité : une seule PR.
