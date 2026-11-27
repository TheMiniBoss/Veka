# Veka : étape 2, les fenêtres personnalisées

Statut : **conception, à valider**. Fait suite à `VEKA_ETAPE1.md` §8.4 (choix n° 7).

## 1. Objectif

Une fenêtre peut être **dessinée entièrement par Veka** : barre de titre, boutons fermer / réduire / agrandir,
fond, coins. Elle doit malgré tout se comporter comme une vraie fenêtre de l'OS :

- se déplacer en tirant sa barre, s'agrandir en double-cliquant dessus ;
- se redimensionner par ses bords et ses coins ;
- garder l'ombre et les coins arrondis du système ;
- sur Windows 11, afficher les **dispositions d'ancrage** (Snap Layouts) au survol du bouton agrandir, et
  accepter l'ancrage clavier (⊞ + flèches) et l'ancrage au bord de l'écran ;
- sur macOS, garder le plein écran, l'ancrage et Mission Control du système.

**Critères d'acceptation**

1. Un exemple `fenetre_perso` (Rust et C) : barre Veka avec titre et trois boutons, contenu en dessous.
2. Sur le PC Windows 11 : déplacement, double-clic, bords, Snap Layouts au survol du bouton agrandir, ancrage,
   ombre et coins arrondis, passage 100 % ↔ 150 % sans défaut.
3. Sur le Mac : mêmes vérifications, plein écran compris (quand le Mac sera accessible).
4. Une fenêtre `transparent: true` laisse voir le bureau là où Veka ne dessine rien.
5. 0 % CPU au repos, comme à l'étape 1.

## 2. Ce que l'utilisateur écrit

### 2.1 Rust

```rust
use veka::prelude::*;

fn fenetre() -> impl View {
    VStack![spacing: 0;
        TitleBar("Mon app"),              // barre prête à l'emploi (choix n° 2)
        Text("Contenu").padding(24),
        Spacer(),
    ]
}

fn main() {
    App::new()
        .window_with(Window::new("Mon app").chrome(Chrome::Custom::default()), fenetre)
        .run()
        .unwrap();
}
```

Ou entièrement à la main, avec les briques de base :

```rust
HStack![
    Text("Mon app").padding(12),
    Spacer(),
    Button("—", || {}).window_control(WindowControl::Minimize),
    Button("□", || {}).window_control(WindowControl::Maximize),
    Button("✕", || {}).window_control(WindowControl::Close),
]
.window_drag_area()
```

- `.window_drag_area()` : la zone déplace la fenêtre (double-clic = agrandir / restaurer).
- `.window_control(c)` : la vue devient le bouton `c` de la fenêtre. Veka exécute l'action ; sur Windows, la
  zone `Maximize` est annoncée au système comme bouton agrandir (c'est ce qui donne les Snap Layouts).
- Les enfants interactifs d'une zone de déplacement (un bouton dans la barre) restent cliquables.

### 2.2 L'état de la fenêtre dans les vues

Une barre doit savoir si la fenêtre est agrandie (icône □ ou ❐) et si elle est active (barre atténuée sinon).
Veka le met dans l'**environnement** :

```rust
let fenetre = Environment::window();   // WindowInfo { maximized, fullscreen, active, chrome }
```

Et une vue peut agir sur la fenêtre sans bouton dédié : `WindowAction::Minimize.perform()`.

### 2.3 C

```c
veka_app_add_window_ex(app, &(VekaWindowDesc){
    .title = "Mon app", .width = 480, .height = 320,
    .chrome = VEKA_CHROME_CUSTOM, .transparent = false, .shadow = true, .corner_radius = 8,
}, setup, body, &etat);

VekaElement *barre = veka_window_drag_area(veka_hstack(0, items, n));
VekaElement *fermer = veka_window_control(veka_button("✕", NULL, NULL), VEKA_WINDOW_CONTROL_CLOSE);
VekaElement *barre_prete = veka_title_bar("Mon app");
```

Tout passe, comme à l'étape 1, par le schéma : `windowDragArea`, `windowControl(control:)`, `TitleBar(title:)`.

## 3. Les trois décorations

| `Chrome` | Windows | macOS |
|---|---|---|
| `System` | barre et bordure de Windows (étape 1) | barre et feux de macOS (étape 1) |
| `Hidden` | barre supprimée, bordure fine, ombre et coins du système gardés | barre transparente, contenu dessous, **feux gardés** |
| `Custom { transparent, shadow, corner_radius }` | aucune décoration ; ombre et coins demandés à DWM | barre et feux masqués, contenu partout ; ombre et coins du système |

Sur macOS, `Hidden` est le style de beaucoup d'apps récentes (Xcode, Safari) ; sur Windows, c'est l'équivalent le
plus proche (pas de boutons de l'OS), donc la vue doit fournir ses propres boutons comme pour `Custom`.

## 4. Comment ça marche

### 4.1 Des régions envoyées à la plateforme

Après chaque layout, la `Surface` collecte les rectangles des vues marquées (`window_drag_area`,
`window_control`) et des vues interactives qu'elles contiennent, puis les envoie à
`PlatformWindow::set_regions` (déjà dans l'API depuis l'étape 1). Seulement quand elles changent. Le cœur ne fait
toujours aucun appel au système.

La plateforme répond au système **sans attendre l'application** : sur Windows, `WM_NCHITTEST` arrive en rafale
pendant un déplacement et doit répondre tout de suite à partir des régions déjà connues.

### 4.2 Windows

- `WM_NCCALCSIZE` : la zone client couvre toute la fenêtre (plus de cadre dessiné par Windows). Fenêtre
  agrandie : on retire la marge que Windows ajoute hors de l'écran.
- `WM_NCHITTEST` : bords et coins (`HTLEFT`… `HTBOTTOMRIGHT`, marge de 6 points), zone de déplacement
  (`HTCAPTION`), boutons (`HTCLOSE`, `HTMINBUTTON`, **`HTMAXBUTTON` → Snap Layouts**), sinon `HTCLIENT`.
- Les boutons annoncés au système ne reçoivent plus `WM_MOUSEMOVE` mais `WM_NCMOUSEMOVE` /
  `WM_NCLBUTTONDOWN` : la plateforme les traduit en événements de pointeur normaux pour que le bouton Veka
  s'allume au survol et réagisse au clic.
- Ombre : `DwmExtendFrameIntoClientArea`. Coins : `DWMWA_WINDOW_CORNER_PREFERENCE` (Windows 11 ; sur Windows 10,
  coins droits).
- Transparence : wgpu sait présenter par **DirectComposition** (`Dx12SwapchainKind::DxgiFromVisual`, alpha
  prémultiplié). La fenêtre `transparent: true` utilise ce mode ; les autres gardent le mode normal.
- Actions : `ShowWindow(SW_MINIMIZE / SW_MAXIMIZE / SW_RESTORE)`, `WM_CLOSE`.
- État : `WM_SIZE` (agrandie ou non), `WM_ACTIVATE` (active ou non) → événement `WindowStateChanged`.

### 4.3 macOS

- `NSWindowStyleMaskFullSizeContentView`, `titlebarAppearsTransparent`, `titleVisibility = hidden`.
- `Custom` : feux masqués (`standardWindowButton(…).setHidden(true)`), fenêtre gardée « titrée » pour conserver
  bords redimensionnables, ombre, coins et plein écran (une fenêtre sans bordure perd tout cela).
- Zone de déplacement : `performWindowDragWithEvent:` au clic ; double-clic selon le réglage du système
  (`AppleActionOnDoubleClick` : agrandir ou réduire).
- Transparence : `isOpaque = false`, `backgroundColor = clear`, couche Metal non opaque.
- Actions : `miniaturize:`, `zoom:`, `toggleFullScreen:`, `performClose:`.

### 4.4 Ce qui change dans les crates

| Crate | Changement |
|---|---|
| `veka-platform` | `WindowState { maximized, fullscreen, active }`, événement `WindowStateChanged`, `PlatformWindow::perform(WindowAction)` |
| `veka-ui` | modifiers `window_drag_area` et `window_control`, vue `TitleBar`, `Environment::window()`, collecte des régions dans `Surface` |
| `veka-platform-windows` | §4.2 |
| `veka-platform-macos` | §4.3 |
| `veka-render-wgpu` | surface transparente (DirectComposition sur Windows, couche non opaque sur macOS) |
| `veka-app` | relie régions, état et actions entre la `Surface` et la plateforme ; `Window` (alias plus lisible de `WindowDesc`) |
| `veka-ffi` | `VekaWindowDesc`, `veka_app_add_window_ex`, `veka_window_drag_area`, `veka_window_control`, `veka_title_bar` |

## 5. Plan

- **PR D**, plateforme et cœur : régions, état, actions ; Windows complet ; exemple Rust ; essais sur le PC.
- **PR E** : transparence, macOS, interface C et exemple C ; essais sur le PC, puis sur le Mac.

## 6. Choix à valider

1. **API** : modifiers `.window_drag_area()` et `.window_control(…)` sur n'importe quelle vue, plus l'état de la
   fenêtre dans l'environnement (§2). *Recommandé.* Autre possibilité : un composant `TitleBar` seul, sans briques
   de base (plus simple mais fermé).
2. **Barre prête à l'emploi `TitleBar`** : *recommandé* de la fournir, avec des boutons **au style Veka mais placés
   comme sur l'OS** (à droite sur Windows, à gauche sur macOS). Autres possibilités : un seul placement partout,
   ou ne pas fournir de barre (chacun la dessine).
3. **`Chrome::Hidden` sur Windows** : *recommandé* = barre supprimée mais bordure fine et ombre du système (§3).
   Autre possibilité : identique à `Custom` sur Windows.
4. **Transparence** dès cette étape, par DirectComposition sur Windows (§4.2). *Recommandé* : c'est la base du
   Liquid Glass plus tard, et wgpu le gère déjà. Autre possibilité : fenêtres opaques seulement pour l'instant.
5. **Marge de redimensionnement** : 6 points, réglable (`resize_margin`). *Recommandé.*
6. **Fonds du système (Mica, Acrylic, vibrancy macOS)** : *recommandé* de les garder pour l'étape Liquid Glass, pour
   ne pas mélanger deux sujets. Autre possibilité : un réglage `backdrop: Mica` dès maintenant.
