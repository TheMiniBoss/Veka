/*
 * veka.h : interface C de Veka.
 *
 * Fichier généré par `cargo xtask header` (cbindgen) : ne pas modifier à la main.
 * Règles : voir crates/veka-ffi/src/lib.rs (propriété des éléments, erreurs, fil).
 */

#ifndef VEKA_H
#define VEKA_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// La forme d'une animation.
typedef enum VekaCurve {
  VEKA_CURVE_LINEAR,
  VEKA_CURVE_EASE_IN,
  VEKA_CURVE_EASE_OUT,
  VEKA_CURVE_EASE_IN_OUT,
  VEKA_CURVE_CIRC_EASE_IN,
  VEKA_CURVE_CIRC_EASE_OUT,
  VEKA_CURVE_CIRC_EASE_IN_OUT,
  // Bézier cubique : `params` = x1, y1, x2, y2.
  VEKA_CURVE_BEZIER,
  // Ressort : `params` = masse, raideur, amortissement ; la durée est ignorée.
  VEKA_CURVE_SPRING,
} VekaCurve;

// Un bord.
typedef enum VekaEdge {
  VEKA_EDGE_LEADING,
  VEKA_EDGE_TRAILING,
  VEKA_EDGE_TOP,
  VEKA_EDGE_BOTTOM,
} VekaEdge;

// Décoration d'une fenêtre.
typedef enum VekaChrome {
  // Barre et boutons du système.
  VEKA_CHROME_SYSTEM,
  // Barre du système masquée (macOS garde ses feux).
  VEKA_CHROME_HIDDEN,
  // Aucune décoration du système : Veka dessine tout.
  VEKA_CHROME_CUSTOM,
} VekaChrome;

// Niveau d'un jeton : 100 %, 60 %, 30 % ou 18 % d'opacité.
typedef enum VekaHierarchy {
  VEKA_HIERARCHY_PRIMARY,
  VEKA_HIERARCHY_SECONDARY,
  VEKA_HIERARCHY_TERTIARY,
  VEKA_HIERARCHY_QUATERNARY,
} VekaHierarchy;

// Espace dans lequel un dégradé interpole (Oklab par défaut).
typedef enum VekaColorSpace {
  VEKA_COLOR_SPACE_OKLAB,
  VEKA_COLOR_SPACE_OK_LCH,
  VEKA_COLOR_SPACE_LINEAR,
  VEKA_COLOR_SPACE_SRGB,
} VekaColorSpace;

// Mode clair ou sombre.
typedef enum VekaColorScheme {
  VEKA_COLOR_SCHEME_LIGHT,
  VEKA_COLOR_SCHEME_DARK,
} VekaColorScheme;

// Type d'une [`VekaValue`].
typedef enum VekaValueKind {
  VEKA_VALUE_KIND_BOOL,
  VEKA_VALUE_KIND_INT,
  VEKA_VALUE_KIND_NUMBER,
  VEKA_VALUE_KIND_STRING,
  VEKA_VALUE_KIND_COLOR,
  // Une valeur d'énumération par son nom (`"title"`, `"leading"`…), dans `string`.
  VEKA_VALUE_KIND_ENUM,
  VEKA_VALUE_KIND_ACTION,
  // Une peinture (`VekaPaint*`, copiée : le C la garde).
  VEKA_VALUE_KIND_PAINT,
  // Un thème (`VekaTheme*`, partagé : le C le garde).
  VEKA_VALUE_KIND_THEME,
  // Une spec d'animation, par valeur.
  VEKA_VALUE_KIND_ANIMATION,
  // Une transition (`VekaTransition*`, copiée : le C la garde).
  VEKA_VALUE_KIND_TRANSITION,
} VekaValueKind;

// Style de texte.
typedef enum VekaFont {
  VEKA_FONT_LARGE_TITLE,
  VEKA_FONT_TITLE,
  VEKA_FONT_TITLE2,
  VEKA_FONT_TITLE3,
  VEKA_FONT_HEADLINE,
  VEKA_FONT_BODY,
  VEKA_FONT_CALLOUT,
  VEKA_FONT_SUBHEADLINE,
  VEKA_FONT_FOOTNOTE,
  VEKA_FONT_CAPTION,
  VEKA_FONT_CAPTION2,
} VekaFont;

// Graisse du texte.
typedef enum VekaWeight {
  VEKA_WEIGHT_THIN,
  VEKA_WEIGHT_ULTRA_LIGHT,
  VEKA_WEIGHT_LIGHT,
  VEKA_WEIGHT_REGULAR,
  VEKA_WEIGHT_MEDIUM,
  VEKA_WEIGHT_SEMIBOLD,
  VEKA_WEIGHT_BOLD,
  VEKA_WEIGHT_HEAVY,
  VEKA_WEIGHT_BLACK,
} VekaWeight;

// Un bouton de fenêtre.
typedef enum VekaWindowControl {
  VEKA_WINDOW_CONTROL_CLOSE,
  VEKA_WINDOW_CONTROL_MINIMIZE,
  VEKA_WINDOW_CONTROL_MAXIMIZE,
  VEKA_WINDOW_CONTROL_FULLSCREEN,
} VekaWindowControl;

// La nature d'une `VekaData`.
typedef enum VekaDataKind {
  VEKA_DATA_KIND_NONE,
  VEKA_DATA_KIND_BOOL,
  VEKA_DATA_KIND_INT,
  VEKA_DATA_KIND_FLOAT,
  VEKA_DATA_KIND_STRING,
  VEKA_DATA_KIND_ARRAY,
  VEKA_DATA_KIND_RECORD,
  // Couleur, symbole, peinture… : lisible seulement en texte.
  VEKA_DATA_KIND_OTHER,
} VekaDataKind;

// Une application. Opaque.
typedef struct VekaApp VekaApp;

// Le contexte passé aux fonctions de rappel. Opaque ; à repasser tel quel à Veka.
typedef struct VekaContext VekaContext;

// Une donnée pour (ou de) un fichier `.vk` : nombre, texte, booléen, tableau, enregistrement.
// Opaque. Celles que crée le C lui appartiennent jusqu'à ce qu'il les donne (`veka_data_push`,
// `veka_data_set`, `veka_vk_set_model` les consomment).
//
// (Côté Rust, un pointeur `VekaData` est un pointeur vers une `Val`.)
typedef struct VekaData VekaData;

// Un élément d'interface (texte, forme, pile…). Opaque.
typedef struct VekaElement VekaElement;

// Une peinture : couleur, jeton du thème ou dégradé. Opaque.
typedef struct VekaPaint VekaPaint;

// Un état. Opaque : c'est une poignée, pas un pointeur à libérer.
typedef struct VekaState VekaState;

// Un thème Colora. Opaque.
typedef struct VekaTheme VekaTheme;

// La description d'une timeline : pistes à images clés et événements. Gardée par le C
// (`veka_timeline_free`) ; `veka_state_new_timeline` en fait une copie.
typedef struct VekaTimeline VekaTimeline;

// Une transition d'entrée et de sortie. Opaque, consommée quand elle est passée à une fonction.
typedef struct VekaTransition VekaTransition;

// Un fichier `.vk` chargé. Opaque ; à libérer avec `veka_vk_free`.
typedef struct VekaVk VekaVk;

// Une animation, passée par valeur. Se construit avec `veka_animation_linear`,
// `veka_animation_spring`… puis `veka_animation_delay`, `veka_animation_repeat`…
typedef struct VekaAnimation {
  enum VekaCurve curve;
  float params[4];
  // Secondes (ignorée par un ressort).
  float duration;
  float delay;
  float speed;
  // Nombre de fois en tout ; 0 ou 1 : une fois ; -1 : sans fin.
  int32_t repeat;
  // Une itération sur deux à l'envers.
  bool autoreverse;
} VekaAnimation;

// Fonction appelée par `veka_with_animation`.
typedef void (*VekaWithFn)(struct VekaContext *cx, void *user);

// Fonction appelée par une action (clic sur un bouton, toucher…).
typedef void (*VekaActionFn)(struct VekaContext *cx, void *user);

// Fonction appelée pour chaque événement franchi, avec son nom.
typedef void (*VekaEventFn)(struct VekaContext *cx, const char *name, void *user);

// Déclare les états de la fenêtre (rappelée avant chaque `body`).
typedef void (*VekaSetupFn)(struct VekaContext *cx, void *user);

// Construit le contenu de la fenêtre. L'élément renvoyé est consommé par Veka.
typedef struct VekaElement *(*VekaBodyFn)(struct VekaContext *cx, void *user);

// Tous les réglages d'une fenêtre.
typedef struct VekaWindowDesc {
  const char *title;
  // Taille du contenu, en points.
  float width;
  float height;
  // Taille minimale (0 : aucune).
  float min_width;
  float min_height;
  bool resizable;
  enum VekaChrome chrome;
  // `VEKA_CHROME_CUSTOM` : fond transparent.
  bool transparent;
  // `VEKA_CHROME_CUSTOM` : ombre du système.
  bool shadow;
  // `VEKA_CHROME_CUSTOM` : rayon des coins, en points.
  float corner_radius;
  // Largeur des bords de redimensionnement, en points (0 : 6 points).
  float resize_margin;
} VekaWindowDesc;

// Un point dans le rectangle d'une forme : (0, 0) en haut à gauche, (1, 1) en bas à droite.
typedef struct VekaUnitPoint {
  float x;
  float y;
} VekaUnitPoint;

// Une couleur sRGB, composantes de 0 à 1.
typedef struct VekaColor {
  float r;
  float g;
  float b;
  float a;
} VekaColor;

// Un arrêt de dégradé : une couleur, ou un jeton si `token` n'est pas NULL, à la position
// `location` (0 à 1).
typedef struct VekaStop {
  struct VekaColor color;
  const char *token;
  enum VekaHierarchy hierarchy;
  float location;
} VekaStop;

// Une action : la fonction et son pointeur `user`.
typedef struct VekaAction {
  VekaActionFn callback;
  void *user;
} VekaAction;

// Contenu d'une [`VekaValue`], selon son type.
typedef union VekaValueData {
  bool boolean;
  int64_t integer;
  double number;
  const char *string;
  struct VekaColor color;
  struct VekaAction action;
  const struct VekaPaint *paint;
  const struct VekaTheme *theme;
  struct VekaAnimation animation;
  const struct VekaTransition *transition;
} VekaValueData;

// Une valeur de paramètre. Se construit avec `veka_number`, `veka_string`…
typedef struct VekaValue {
  enum VekaValueKind kind;
  union VekaValueData data;
} VekaValue;

// Un paramètre nommé, par exemple `{ "spacing", veka_number(16) }`.
typedef struct VekaParam {
  const char *name;
  struct VekaValue value;
} VekaParam;

// Reçoit un `emit` qu'aucune vue du fichier n'écoute. `value` n'est valable que pendant l'appel
// (NULL pour un événement sans valeur).
typedef void (*VekaVkEventFn)(const char *view,
                              const char *event,
                              const struct VekaData *value,
                              void *user);

#define VEKA_UNIT_TOP_LEADING (VekaUnitPoint){ .x = 0.0, .y = 0.0 }

#define VEKA_UNIT_TOP (VekaUnitPoint){ .x = 0.5, .y = 0.0 }

#define VEKA_UNIT_TOP_TRAILING (VekaUnitPoint){ .x = 1.0, .y = 0.0 }

#define VEKA_UNIT_LEADING (VekaUnitPoint){ .x = 0.0, .y = 0.5 }

#define VEKA_UNIT_CENTER (VekaUnitPoint){ .x = 0.5, .y = 0.5 }

#define VEKA_UNIT_TRAILING (VekaUnitPoint){ .x = 1.0, .y = 0.5 }

#define VEKA_UNIT_BOTTOM_LEADING (VekaUnitPoint){ .x = 0.0, .y = 1.0 }

#define VEKA_UNIT_BOTTOM (VekaUnitPoint){ .x = 0.5, .y = 1.0 }

#define VEKA_UNIT_BOTTOM_TRAILING (VekaUnitPoint){ .x = 1.0, .y = 1.0 }

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

// Le message de la dernière erreur sur ce fil, ou NULL. Valable jusqu'à la prochaine erreur.
const char *veka_last_error(void);

// Efface la dernière erreur.
void veka_clear_error(void);

// La version de Veka (« 0.1.0 »).
const char *veka_version(void);

struct VekaAnimation veka_animation_linear(float duration);

struct VekaAnimation veka_animation_ease_in(float duration);

struct VekaAnimation veka_animation_ease_out(float duration);

struct VekaAnimation veka_animation_ease_in_out(float duration);

struct VekaAnimation veka_animation_circ_ease_in(float duration);

struct VekaAnimation veka_animation_circ_ease_out(float duration);

struct VekaAnimation veka_animation_circ_ease_in_out(float duration);

// Bézier cubique, comme `cubic-bezier(x1, y1, x2, y2)` en CSS.
struct VekaAnimation veka_animation_timing_curve(float x1,
                                                 float y1,
                                                 float x2,
                                                 float y2,
                                                 float duration);

// Ressort : `response` = période (s), `damping_fraction` = 1 sans rebond.
struct VekaAnimation veka_animation_spring(float response, float damping_fraction);

// Ressort aux paramètres physiques.
struct VekaAnimation veka_animation_interpolating_spring(float mass,
                                                         float stiffness,
                                                         float damping);

// Le ressort sans rebond (l'animation par défaut).
struct VekaAnimation veka_animation_smooth(void);

// Un ressort au léger rebond.
struct VekaAnimation veka_animation_snappy(void);

// Un ressort au rebond visible.
struct VekaAnimation veka_animation_bouncy(void);

// Commence après `seconds`.
struct VekaAnimation veka_animation_delay(struct VekaAnimation animation, float seconds);

// Multiplie la vitesse.
struct VekaAnimation veka_animation_speed(struct VekaAnimation animation, float speed);

// Joue `count` fois en tout.
struct VekaAnimation veka_animation_repeat(struct VekaAnimation animation,
                                           uint32_t count,
                                           bool autoreverse);

// Joue sans fin.
struct VekaAnimation veka_animation_repeat_forever(struct VekaAnimation animation,
                                                   bool autoreverse);

// Appelle `f` : ce que changent les états modifiés dedans s'anime avec `animation` (layout,
// couleurs, opacité, entrées et sorties). Comme `withAnimation` de SwiftUI.
void veka_with_animation(struct VekaContext *_cx,
                         struct VekaAnimation animation,
                         VekaWithFn f,
                         void *user);

// Aucun effet.
struct VekaTransition *veka_transition_identity(void);

// Le fondu (la transition par défaut).
struct VekaTransition *veka_transition_opacity(void);

// Grandit depuis l'échelle `from` (0 : depuis rien).
struct VekaTransition *veka_transition_scale(float from);

// Entre par le bord de début, sort par celui de fin.
struct VekaTransition *veka_transition_slide(void);

// Entre et sort par `edge`.
struct VekaTransition *veka_transition_move(enum VekaEdge edge);

// Entre par `from`, sort par le bord opposé, en fondu.
struct VekaTransition *veka_transition_push(enum VekaEdge from);

// Décalée de (x, y) points quand elle est absente.
struct VekaTransition *veka_transition_offset(float x, float y);

// Floue quand elle est absente (rendu par un fondu pour l'instant).
struct VekaTransition *veka_transition_blur(float radius);

// Les deux à la fois (les deux sont consommées).
struct VekaTransition *veka_transition_combined(struct VekaTransition *a, struct VekaTransition *b);

// Une transition pour l'entrée, une autre pour la sortie (les deux sont consommées).
struct VekaTransition *veka_transition_asymmetric(struct VekaTransition *insertion,
                                                  struct VekaTransition *removal);

// Libère une transition donnée à personne.
void veka_transition_free(struct VekaTransition *t);

// Opacité (0 à 1), animable.
struct VekaElement *veka_opacity(struct VekaElement *element, float opacity);

// Décale le dessin sans changer le layout, animable.
struct VekaElement *veka_offset(struct VekaElement *element, float x, float y);

// Agrandit le dessin autour de son centre, animable.
struct VekaElement *veka_scale_effect(struct VekaElement *element, float scale);

// Comme `veka_scale_effect`, un facteur par axe.
struct VekaElement *veka_scale_effect_xy(struct VekaElement *element, float x, float y);

// Tourne le dessin autour de son centre (degrés, sens horaire), animable.
struct VekaElement *veka_rotation_effect(struct VekaElement *element, float degrees);

// Anime le contenu avec `*animation` chaque fois que `value` change. `animation` NULL : ne pas
// animer, même dans un `veka_with_animation`.
struct VekaElement *veka_animation(struct VekaElement *element,
                                   const struct VekaAnimation *animation,
                                   int64_t value);

// Comment le contenu entre et sort pendant une animation. La transition est consommée.
struct VekaElement *veka_transition(struct VekaElement *element, struct VekaTransition *transition);

// Relie le contenu aux vues de même `id` : quand l'une disparaît et que l'autre apparaît dans la
// même animation, la nouvelle part de la place de l'ancienne.
struct VekaElement *veka_matched_geometry_effect(struct VekaElement *element, const char *id);

// L'identité de l'élément parmi ses frères (comme `.id()` en Rust) : une ligne garde son état et
// ses animations quand la liste change, et entre ou sort seule avec sa transition.
struct VekaElement *veka_id(struct VekaElement *element,
                            int64_t id);

// Anime le contenu même si le système demande de réduire les animations.
struct VekaElement *veka_ignore_reduce_motion(struct VekaElement *element);

// Appelle `callback` quand les animations du contenu sont finies.
struct VekaElement *veka_on_animation_end(struct VekaElement *element,
                                          VekaActionFn callback,
                                          void *user);

// Une timeline vide (vitesse 1, sans boucle).
struct VekaTimeline *veka_timeline_new(void);

// Ajoute une image clé à la piste `track` (créée au besoin) : `value` à `time` secondes, `curve`
// adoucissant le segment qui part de cette clé.
void veka_timeline_key(struct VekaTimeline *t,
                       const char *track,
                       float time,
                       float value,
                       enum VekaCurve curve);

// Un événement nommé à `time` secondes (voir `veka_timeline_on_event`).
void veka_timeline_event(struct VekaTimeline *t, float time, const char *name);

// La durée en secondes (0 : jusqu'à la dernière clé ou au dernier événement).
void veka_timeline_length(struct VekaTimeline *t, float seconds);

// Recommence au début une fois au bout.
void veka_timeline_looping(struct VekaTimeline *t, bool looping);

// Multiplie la vitesse de lecture.
void veka_timeline_speed(struct VekaTimeline *t, float speed);

void veka_timeline_free(struct VekaTimeline *t);

// Un état timeline (créé la première fois avec une copie de `*timeline`, retrouvé ensuite), comme
// les autres `veka_state_new_*`. `autoplay` : lancé dès sa création.
struct VekaState *veka_state_new_timeline(struct VekaContext *_cx,
                                          const struct VekaTimeline *timeline,
                                          bool autoplay);

// Lance (ou reprend) la lecture ; arrivée au bout, elle repart du début.
void veka_timeline_play(struct VekaContext *_cx, struct VekaState *s);

void veka_timeline_pause(struct VekaContext *_cx, struct VekaState *s);

// Met en pause et revient au début.
void veka_timeline_stop(struct VekaContext *_cx, struct VekaState *s);

// Va à `time` secondes.
void veka_timeline_seek(struct VekaContext *_cx, struct VekaState *s, float time);

// Change de sens depuis l'instant présent.
void veka_timeline_reverse(struct VekaContext *_cx, struct VekaState *s);

// La valeur de la piste `track` à l'instant présent (0 si elle n'existe pas).
float veka_timeline_value(struct VekaContext *_cx, struct VekaState *s, const char *track);

// L'instant présent, en secondes.
float veka_timeline_time(struct VekaContext *_cx, struct VekaState *s);

bool veka_timeline_is_playing(struct VekaContext *_cx, struct VekaState *s);

// Appelle `callback` pour chaque événement franchi pendant la lecture. Peut être rappelée à chaque
// `body` (la dernière fonction donnée est gardée).
void veka_timeline_on_event(struct VekaContext *_cx,
                            struct VekaState *s,
                            VekaEventFn callback,
                            void *user);

// Crée une application vide.
struct VekaApp *veka_app_new(void);

// Ajoute une fenêtre de `width` × `height` points. Renvoie 0, ou -1 en cas d'erreur.
int32_t veka_app_add_window(struct VekaApp *app,
                            const char *title,
                            float width,
                            float height,
                            VekaSetupFn setup,
                            VekaBodyFn body,
                            void *user);

// Réglages par défaut : 480 × 320, redimensionnable, décoration du système.
struct VekaWindowDesc veka_window_desc_default(const char *title);

// Ajoute une fenêtre avec tous ses réglages. Renvoie 0, ou -1 en cas d'erreur.
int32_t veka_app_add_window_ex(struct VekaApp *app,
                               const struct VekaWindowDesc *desc,
                               VekaSetupFn setup,
                               VekaBodyFn body,
                               void *user);

// Ouvre les fenêtres et bloque jusqu'à la fermeture de la dernière. Libère `app`.
// Renvoie 0, ou 1 en cas d'erreur (voir `veka_last_error`).
int32_t veka_app_run(struct VekaApp *app);

// Libère une application qui n'a pas été lancée.
void veka_app_free(struct VekaApp *app);

// Un point quelconque (`VEKA_UNIT_CENTER`… pour les points usuels).
struct VekaUnitPoint veka_unit_point(float x, float y);

// Une couleur unie.
struct VekaPaint *veka_solid(struct VekaColor color);

// Un jeton du thème par son nom (`"Accent"`, `"Foreground"`, `"Marque.Fond"`…). Un jeton inconnu
// s'affiche en magenta.
struct VekaPaint *veka_token(const char *name, enum VekaHierarchy hierarchy);

// Dégradé linéaire de `from` à `to`, couleurs réparties régulièrement.
struct VekaPaint *veka_linear_gradient(const struct VekaColor *colors,
                                       size_t count,
                                       struct VekaUnitPoint from,
                                       struct VekaUnitPoint to);

// Dégradé radial autour de `center`, rayons en points.
struct VekaPaint *veka_radial_gradient(const struct VekaColor *colors,
                                       size_t count,
                                       struct VekaUnitPoint center,
                                       float start_radius,
                                       float end_radius);

// Dégradé elliptique : rayons en fraction de la demi-taille de la forme (0 à 1).
struct VekaPaint *veka_elliptical_gradient(const struct VekaColor *colors,
                                           size_t count,
                                           struct VekaUnitPoint center,
                                           float start_fraction,
                                           float end_fraction);

// Dégradé angulaire autour de `center` : angles en degrés, sens horaire depuis la droite.
struct VekaPaint *veka_angular_gradient(const struct VekaColor *colors,
                                        size_t count,
                                        struct VekaUnitPoint center,
                                        float start_angle,
                                        float end_angle);

// Dégradé maillé de `width` × `height` points (au moins 2 × 2), couleurs ligne par ligne.
// `positions` peut être NULL : grille régulière.
struct VekaPaint *veka_mesh_gradient(size_t width,
                                     size_t height,
                                     const struct VekaUnitPoint *positions,
                                     const struct VekaColor *colors);

// Remplace les arrêts d'un dégradé (pour y mettre des jetons ou des positions libres). Pour un
// dégradé maillé, les couleurs des points dans l'ordre (les positions des arrêts sont ignorées).
// `paint` est consommé ; renvoie la peinture modifiée, ou NULL.
struct VekaPaint *veka_paint_stops(struct VekaPaint *paint,
                                   const struct VekaStop *stops,
                                   size_t count);

// Choisit l'espace d'interpolation d'un dégradé. `paint` est consommé ; renvoie la peinture
// modifiée (inchangée si ce n'est pas un dégradé).
struct VekaPaint *veka_paint_space(struct VekaPaint *paint, enum VekaColorSpace space);

// Une copie de la peinture (qui, elle, n'est pas consommée).
struct VekaPaint *veka_paint_copy(const struct VekaPaint *paint);

// Libère une peinture qui n'a été donnée à personne.
void veka_paint_free(struct VekaPaint *paint);

// Remplit une forme avec une peinture (consommée).
struct VekaElement *veka_fill_paint(struct VekaElement *element, struct VekaPaint *paint);

// Contour d'une forme avec une peinture (consommée).
struct VekaElement *veka_stroke_paint(struct VekaElement *element,
                                      struct VekaPaint *paint,
                                      float line_width);

// Couleur du texte et des formes non remplies du contenu (peinture consommée).
struct VekaElement *veka_foreground_paint(struct VekaElement *element, struct VekaPaint *paint);

// Couleur d'accent du contenu (peinture consommée).
struct VekaElement *veka_tint_paint(struct VekaElement *element, struct VekaPaint *paint);

// Fond derrière le contenu (peinture consommée).
struct VekaElement *veka_background_paint(struct VekaElement *element, struct VekaPaint *paint);

// Applique un thème au contenu. Le thème n'est pas consommé.
struct VekaElement *veka_theme(struct VekaElement *element, const struct VekaTheme *theme);

// Force le mode clair ou sombre du contenu (sinon : celui du système).
struct VekaElement *veka_color_scheme(struct VekaElement *element, enum VekaColorScheme scheme);

// Ajoute `amount` (-1 à 1) à chaque composante des couleurs du contenu.
struct VekaElement *veka_brightness(struct VekaElement *element, float amount);

// Contraste du contenu (1 = inchangé).
struct VekaElement *veka_contrast(struct VekaElement *element, float amount);

// Saturation du contenu (0 = gris, 1 = inchangé).
struct VekaElement *veka_saturation(struct VekaElement *element, float amount);

// Rotation de teinte du contenu, en degrés.
struct VekaElement *veka_hue_rotation(struct VekaElement *element, float amount);

// Niveaux de gris (0 = inchangé, 1 = entièrement gris).
struct VekaElement *veka_grayscale(struct VekaElement *element, float amount);

// Un thème vide : tout jeton absent vient de `parent` (qui peut être NULL), puis de Veka.
// Le parent est copié : le modifier ensuite ne change pas ce thème.
struct VekaTheme *veka_theme_new(const char *name, const struct VekaTheme *parent);

// Valeur d'un jeton en mode clair et en mode sombre (`dark` NULL : la même). Les peintures sont
// consommées. Renvoie 0, ou -1 en cas d'erreur.
int32_t veka_theme_set_color(struct VekaTheme *theme,
                             const char *token,
                             struct VekaPaint *light,
                             struct VekaPaint *dark);

// Une mesure de design du thème (`"CornerRadius.Card"`…). Renvoie 0, ou -1 en cas d'erreur.
int32_t veka_theme_set_metric(struct VekaTheme *theme, const char *name, float value);

// Libère un thème. Les éléments qui l'utilisent gardent leur copie.
void veka_theme_free(struct VekaTheme *theme);

// Construit n'importe quel composant du schéma par son nom (`"VStack"`, `"Text"`…).
// Les enfants sont consommés.
struct VekaElement *veka_element(const char *name,
                                 const struct VekaParam *params_ptr,
                                 size_t params_count,
                                 struct VekaElement *const *children,
                                 size_t children_count);

// Applique n'importe quel modifier du schéma par son nom (`"padding"`, `"fill"`…).
// `element` est consommé.
struct VekaElement *veka_modifier(struct VekaElement *element,
                                  const char *name,
                                  const struct VekaParam *params_ptr,
                                  size_t params_count);

// Libère un élément qui n'a été donné à personne.
void veka_element_free(struct VekaElement *element);

// Du texte (UTF-8, copié).
struct VekaElement *veka_text(const char *content);

struct VekaElement *veka_rectangle(void);

struct VekaElement *veka_rounded_rect(float corner_radius);

struct VekaElement *veka_capsule(void);

struct VekaElement *veka_circle(void);

struct VekaElement *veka_ellipse(void);

// Espace flexible dans une pile.
struct VekaElement *veka_spacer(void);

// Bouton : `action` est appelée à chaque clic avec `user`.
struct VekaElement *veka_button(const char *title, VekaActionFn callback, void *user);

// Pile verticale. Les `count` éléments sont consommés.
struct VekaElement *veka_vstack(float spacing, struct VekaElement *const *items, size_t count);

// Pile horizontale. Les `count` éléments sont consommés.
struct VekaElement *veka_hstack(float spacing, struct VekaElement *const *items, size_t count);

// Pile en profondeur (le premier élément est au fond). Les `count` éléments sont consommés.
struct VekaElement *veka_zstack(struct VekaElement *const *items, size_t count);

// Marge identique sur les quatre côtés.
struct VekaElement *veka_padding(struct VekaElement *element, float all);

// Taille imposée. Une valeur négative laisse cette dimension libre.
struct VekaElement *veka_frame(struct VekaElement *element, float width, float height);

// Remplit une forme.
struct VekaElement *veka_fill(struct VekaElement *element, struct VekaColor c);

// Trace le contour d'une forme.
struct VekaElement *veka_stroke(struct VekaElement *element, struct VekaColor c, float line_width);

// Couleur du texte et des formes non remplies du contenu.
struct VekaElement *veka_foreground(struct VekaElement *element, struct VekaColor c);

// Couleur d'accent (boutons…).
struct VekaElement *veka_tint(struct VekaElement *element, struct VekaColor c);

// Fond coloré derrière le contenu.
struct VekaElement *veka_background(struct VekaElement *element, struct VekaColor c);

// Style du texte du contenu.
struct VekaElement *veka_font(struct VekaElement *element, enum VekaFont font);

// Graisse du texte du contenu.
struct VekaElement *veka_font_weight(struct VekaElement *element, enum VekaWeight weight);

// Appelle `callback` quand on touche (clique) le contenu.
struct VekaElement *veka_on_tap(struct VekaElement *element, VekaActionFn callback, void *user);

// La vue déplace la fenêtre quand on la tire (double-clic : agrandir).
struct VekaElement *veka_window_drag_area(struct VekaElement *element);

// La vue devient le bouton `control` de la fenêtre.
struct VekaElement *veka_window_control(struct VekaElement *element,
                                        enum VekaWindowControl control);

// Un bouton de fenêtre dessiné par Veka, au style du système.
struct VekaElement *veka_window_button(enum VekaWindowControl control);

// Une barre de titre prête à l'emploi (titre et boutons placés comme sur le système).
struct VekaElement *veka_title_bar(const char *title);

// Crée un état entier (la première fois) ou retrouve le même (les fois suivantes).
struct VekaState *veka_state_new_int(struct VekaContext *_cx, int64_t initial);

int64_t veka_state_get_int(struct VekaContext *_cx, struct VekaState *s);

// Change la valeur : la vue sera reconstruite avant la prochaine image.
void veka_state_set_int(struct VekaContext *_cx, struct VekaState *s, int64_t value);

struct VekaState *veka_state_new_number(struct VekaContext *_cx, double initial);

double veka_state_get_number(struct VekaContext *_cx, struct VekaState *s);

void veka_state_set_number(struct VekaContext *_cx, struct VekaState *s, double value);

struct VekaState *veka_state_new_bool(struct VekaContext *_cx, bool initial);

bool veka_state_get_bool(struct VekaContext *_cx, struct VekaState *s);

void veka_state_set_bool(struct VekaContext *_cx, struct VekaState *s, bool value);

// Couleur opaque.
struct VekaColor veka_rgb(float r, float g, float b);

// Couleur avec opacité.
struct VekaColor veka_rgba(float r, float g, float b, float a);

// Couleur depuis un entier `0xRRGGBB`.
struct VekaColor veka_hex(uint32_t rgb);

struct VekaValue veka_bool(bool value);

struct VekaValue veka_int(int64_t value);

struct VekaValue veka_number(double value);

// Chaîne UTF-8, copiée au moment de la construction de l'élément.
struct VekaValue veka_string(const char *value);

struct VekaValue veka_color(struct VekaColor value);

// Valeur d'énumération par son nom : `veka_enum("title")`.
struct VekaValue veka_enum(const char *name);

struct VekaValue veka_action(VekaActionFn callback, void *user);

// Une peinture comme valeur de paramètre (`fill`, `foregroundColor`…). Elle est copiée au moment
// de la construction de l'élément : le C la garde et la libère.
struct VekaValue veka_paint_value(const struct VekaPaint *paint);

// Un thème comme valeur de paramètre (modifier `theme`). Le C le garde.
struct VekaValue veka_theme_value(const struct VekaTheme *theme);

// Une spec d'animation comme valeur de paramètre.
struct VekaValue veka_animation_value(struct VekaAnimation animation);

// Une transition comme valeur de paramètre (copiée à l'usage : le C la garde).
struct VekaValue veka_transition_value(const struct VekaTransition *transition);

// Charge un fichier `.vk` depuis son texte. NULL si le texte a des erreurs (`veka_last_error`
// les donne, une par ligne, avec leur position).
struct VekaVk *veka_vk_load(const char *source);

// Charge un fichier `.vk` depuis le disque ; ses `import` sont relatifs à son dossier.
struct VekaVk *veka_vk_load_file(const char *path);

// Remplace le texte du fichier ; les états des vues affichées gardent leurs valeurs. Renvoie 0,
// ou -1 si le nouveau texte a des erreurs (l'ancien reste alors en place).
int32_t veka_vk_reload(struct VekaVk *vk, const char *source);

// La vue `name` du fichier, comme un élément (à donner à une pile ou à renvoyer de `body`).
struct VekaElement *veka_vk_view(struct VekaVk *vk, const char *name);

// Donne ses données au fichier (`model` dans le `.vk`) ; consomme `model`. Les vues affichées
// sont ré-évaluées.
int32_t veka_vk_set_model(struct VekaVk *vk, struct VekaData *model);

// Installe le rappel d'événements du fichier (remplace le précédent).
int32_t veka_vk_on_event(struct VekaVk *vk, VekaVkEventFn f, void *user);

// Libère un fichier `.vk` (les vues déjà construites restent valables).
void veka_vk_free(struct VekaVk *vk);

// Ajoute une fenêtre qui affiche la vue `view` du fichier `path`, relu à chaud en debug. Rend le
// fichier chargé (pour `veka_vk_set_model` et `veka_vk_on_event`), à libérer avec
// `veka_vk_free` ; NULL en cas d'erreur.
struct VekaVk *veka_app_add_window_vk(struct VekaApp *app,
                                      const struct VekaWindowDesc *desc,
                                      const char *path,
                                      const char *view);

struct VekaData *veka_data_int(int64_t v);

struct VekaData *veka_data_float(double v);

struct VekaData *veka_data_bool(bool v);

// Un texte (copié). NULL si `s` n'est pas une chaîne UTF-8.
struct VekaData *veka_data_string(const char *s);

// Un tableau vide (`veka_data_push` le remplit).
struct VekaData *veka_data_array(void);

// Un enregistrement vide (`veka_data_set` le remplit).
struct VekaData *veka_data_record(void);

// Ajoute `item` à la fin du tableau `array` ; consomme `item`. Renvoie 0, ou -1.
int32_t veka_data_push(struct VekaData *array, struct VekaData *item);

// Donne au champ `key` de l'enregistrement `record` la valeur `value` ; consomme `value`.
int32_t veka_data_set(struct VekaData *record, const char *key, struct VekaData *value);

// Libère une donnée que le C n'a donnée à personne.
void veka_data_free(struct VekaData *d);

// La nature de `d` (`VEKA_DATA_KIND_NONE` pour NULL).
enum VekaDataKind veka_data_kind(const struct VekaData *d);

// Un nombre (entier ou non) ; 0 sinon.
double veka_data_as_float(const struct VekaData *d);

// Un entier (un nombre est arrondi vers zéro) ; 0 sinon.
int64_t veka_data_as_int(const struct VekaData *d);

bool veka_data_as_bool(const struct VekaData *d);

// La donnée en texte, copiée dans `buffer` (`size` octets, zéro final compris). Renvoie la
// longueur complète en octets (sans le zéro), même si elle ne tient pas.
size_t veka_data_text(const struct VekaData *d, char *buffer, size_t size);

// Le nombre d'éléments d'un tableau (ou de champs d'un enregistrement) ; 0 sinon.
size_t veka_data_len(const struct VekaData *d);

// L'élément `i` d'un tableau, emprunté (valable tant que `d` l'est) ; NULL hors du tableau.
const struct VekaData *veka_data_at(const struct VekaData *d, size_t i);

// Le champ `key` d'un enregistrement, emprunté (valable tant que `d` l'est) ; NULL s'il manque.
const struct VekaData *veka_data_get(const struct VekaData *d, const char *key);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* VEKA_H */
