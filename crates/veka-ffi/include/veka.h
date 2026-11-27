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

// Une application. Opaque.
typedef struct VekaApp VekaApp;

// Le contexte passé aux fonctions de rappel. Opaque ; à repasser tel quel à Veka.
typedef struct VekaContext VekaContext;

// Un élément d'interface (texte, forme, pile…). Opaque.
typedef struct VekaElement VekaElement;

// Une peinture : couleur, jeton du thème ou dégradé. Opaque.
typedef struct VekaPaint VekaPaint;

// Un état. Opaque : c'est une poignée, pas un pointeur à libérer.
typedef struct VekaState VekaState;

// Un thème Colora. Opaque.
typedef struct VekaTheme VekaTheme;

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

// Fonction appelée par une action (clic sur un bouton, toucher…).
typedef void (*VekaActionFn)(struct VekaContext *cx, void *user);

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

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* VEKA_H */
