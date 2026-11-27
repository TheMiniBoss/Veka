# Veka : étape 7, un `.vk` rapide

Statut : **validé** (les 6 choix recommandés) ; PR N faite. Fait suite à `VEKA_ETAPE6.md`. Reprend la note de `VEKA_ETAPE5.md` :
« pas de bytecode au départ ». L'interpréteur d'arbre a tenu tant que les vues ne bougeaient qu'au clic. Avec
`@Clock` et les feux d'artifice, une vue est réévaluée à chaque image : c'est maintenant le goulot.

## 1. Objectif

- Les feux d'artifice (`cargo run --example vk -- feux`) **fluides en debug**, à 60 images par seconde sans à-coups.
- **Moins de CPU en release** pour toute vue `.vk` qui s'anime.
- Garder ce qui fait le `.vk` : rechargement à chaud instantané, messages d'erreur avec l'endroit et « vouliez-vous
  dire ». Le cœur doit toujours compiler pour WebAssembly. Aucun changement du langage.

Cibles, sur les feux en 720 × 480, mesurées hors GPU par le test de mesure (§3.5) :

| | Aujourd'hui | Cible |
|---|---|---|
| debug | 31 ms par image | ≤ 4 ms |
| release | 6,3 ms par image | ≤ 1,5 ms |

## 2. Où part le temps (mesuré)

Mesure prise sur 20 images des feux, en release, avec callgrind. Environ 1 000 étincelles sont visibles, soit
3 600 commandes de dessin.

| Poste | Part | Ce qui coûte |
|---|---|---|
| **Interpréteur** (corps de la vue) | **72 %** | voir le détail ci-dessous |
| Animations après le layout | 10 % | `after_layout` compare les valeurs animables de chaque nœud, même sans animation en cours |
| Placement | 6 % | |
| Environnement | 3 % | |
| Zones (clics, fenêtre) | 2 % | |
| Liste de dessin | 1 % | |

Dans l'interpréteur :

- **Les allocations prennent environ 25 % du temps total.** `malloc` et `free` arrivent en tête, devant l'évaluation
  elle-même. À chaque appel de fonction, on crée un nouveau contexte complet (4 `Rc`, des `HashMap`) et on alloue
  une chaîne par paramètre et par `let`. Les arguments sont des `Vec<(Option<String>, Val)>`.
- **Les noms sont cherchés à chaque usage** : variables par comparaison de chaînes, puis `HashMap` (SipHash) ;
  entrées du schéma par nom ; paramètres d'une vue par nom, deux fois (à l'évaluation, puis dans
  `Schema::build`).
- **Les appels de fonctions pures font 20 %.** Ce sont surtout `hasard(base + k * 7.1)`, deux par étincelle et par
  image, alors que le résultat ne dépend pas du temps.

En debug, le même code tourne sans optimisation : environ 5 fois plus lent. Les dépendances externes sont déjà
compilées en `opt-level = 2`, mais pas les crates de Veka.

**Essai : avec `opt-level = 1` pour les crates de Veka en debug, les feux passent de 31 à 8 ms par image.** Le reste
du gain doit venir de l'interpréteur.

## 3. Fonctionnement proposé

### 3.1 Debug : le cœur optimisé

Dans `Cargo.toml` à la racine : `[profile.dev.package.veka-lang]` (et `veka-ui`, `veka-colora`, `veka-anima`,
`veka-graphics`, `veka-foundation`) en `opt-level = 1`. Le pas-à-pas reste correct à ce niveau, et la compilation
ne ralentit presque pas. Une application qui dépend de Veka garde son propre profil : on documente la ligne à
copier.

### 3.2 Compiler au chargement

Aujourd'hui, l'AST est relu à chaque évaluation. Au chargement, et à chaque rechargement à chaud, on le transforme
une fois en une forme **résolue**, puis en **fermetures** : chaque expression devient une `Box<dyn Fn(&mut Frame)
-> Val>`.

- **Variables** : chaque `let`, paramètre de `ForEach`, de fermeture ou de fonction, `@State`, `@Param` et horloge
  reçoit un **numéro de case**. Une lecture devient `frame.slots[3]`, sans chaîne ni hachage. Les noms restent dans
  la table pour les messages d'erreur.
- **Schéma** : chaque appel de vue ou de modifier pointe directement sur son entrée. Ses arguments sont déjà
  rangés dans l'ordre des paramètres, avec le type attendu. `Schema::build` reçoit les valeurs par position : une
  nouvelle entrée sans noms, `build_indexed`, à côté de l'actuelle.
- **Constantes repliées** : `2 * pi`, `Float(3)`, `[.white, .clear]` et les dégradés constants sont calculés une
  seule fois.
- **Fonctions** : un appel réserve ses cases dans le même `Frame`, sans nouveau contexte. Les arguments passent dans
  un petit tableau sur la pile, sans `Vec` ni `String`.
- **Valeurs** : `Val` reste tel quel. Les chaînes et les tableaux sont déjà partagés (`Rc`).

Les erreurs de l'analyse au chargement ne changent pas : elles se calculent sur l'AST, avant la compilation. Les
erreurs d'exécution (division par zéro, index hors limites…) gardent leur `Span`, que chaque fermeture emporte.

### 3.3 Fonctions pures mémorisées

Une `func` du `.vk` est pure par construction : elle ne contient que des `let` et sa valeur. On mémorise donc ses
résultats **à chaque site d'appel**, par la valeur exacte de ses arguments. Le cache est petit et borné, par
exemple 4 096 entrées, avec éviction par génération. Il est vidé au rechargement. Dans les feux, `hasard(base + k *
7.1)` n'est alors calculé qu'à la première image. Une fonction qui appelle `random(…)` est déterministe, donc
reste pure.

### 3.4 Côté interface

- `after_layout` ne compare plus les valeurs animables quand aucune transaction n'est ouverte et qu'aucun nœud n'a
  de `.animation(…)`. C'est le cas des feux.
- `update_regions` ne parcourt l'arbre que s'il existe des zones (déplacement, boutons de fenêtre).
- On garde le reste. Le layout et la liste de dessin coûtent peu.

### 3.5 Mesurer

- Un test de mesure `cargo test -p veka --release --test mesure -- --ignored` affiche le temps par image des feux,
  de l'anima et de la boutique, en debug et en release. La CI le lance et affiche les chiffres, sans seuil, parce
  que les machines de la CI sont trop variables.
- `VEKA_STATS=1` sur une application affiche toutes les secondes, sur la sortie d'erreur, le temps moyen et maximal
  d'une image, réparti entre évaluation, layout et rendu. Cela sert aussi aux essais sur le PC et le Mac.

## 4. Plan

- **PR N, les gains simples** :
  - le cœur en `opt-level = 1` en debug ;
  - les gains côté interface (§3.4) ;
  - le test de mesure et `VEKA_STATS` ;
  - les allocations évitables de l'interpréteur actuel : contexte de fonction, chaînes des `let`, arguments ;
  - essais sur le PC.
  - Fait. Feux en 720 × 480 : debug de 31 à 6,5 ms, release de 6,3 à 5,3 ms par image. `VEKA_STATS` sépare
    l'interface du rendu et de la présentation (le moteur ne mesure pas le temps : il compile pour WebAssembly).
- **PR O, l'interpréteur compilé** :
  - forme résolue, cases, entrées du schéma résolues, `build_indexed`, constantes repliées, fermetures ;
  - fonctions pures mémorisées ;
  - tous les tests `.vk` actuels inchangés (même dessin que Rust) ;
  - essais sur le PC et le Mac.

## 5. Choix à valider

1. **Debug** : *recommandé* = les crates de Veka en `opt-level = 1` dans le profil debug du dépôt (essai : 31 →
   8 ms). Autre possibilité : ne rien changer et compter seulement sur l'interpréteur.
2. **Exécution** : *recommandé* = compiler au chargement en fermetures, avec des noms résolus en cases et des entrées
   du schéma résolues. Le rechargement à chaud reste instantané. Autres possibilités :
   - seulement retoucher l'interpréteur d'arbre (environ 2 fois plus rapide, pas plus) ;
   - un bytecode et une machine virtuelle (aussi rapide que les fermetures, mais beaucoup plus de code) ;
   - générer du Rust au build (le plus rapide, mais plus de rechargement à chaud).
3. **Fonctions pures** : *recommandé* = mémoriser automatiquement leurs résultats par site d'appel, avec un cache
   borné. Autre possibilité : ne pas mémoriser.
4. **Réutiliser les sous-arbres qui ne bougent pas** (ne pas réévaluer ce qui ne lit pas l'horloge) : *recommandé* =
   **plus tard**. Dans les feux, presque tout dépend du temps, donc le gain y serait faible pour beaucoup de
   complexité. Autre possibilité : le faire dans la PR O.
5. **Découpage** : *recommandé* = deux PR, les gains simples d'abord (PR N, utile tout de suite), puis
   l'interpréteur compilé (PR O). Autre possibilité : une seule PR.
6. **Mesure** : *recommandé* = un test de mesure affiché par la CI sans seuil, plus `VEKA_STATS=1`. Autre possibilité :
   une mesure seulement à la main.
