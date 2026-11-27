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

// Une application. Opaque.
typedef struct VekaApp VekaApp;

// Le contexte passé aux fonctions de rappel. Opaque ; à repasser tel quel à Veka.
typedef struct VekaContext VekaContext;

// Un élément d'interface (texte, forme, pile…). Opaque.
typedef struct VekaElement VekaElement;

// Un état. Opaque : c'est une poignée, pas un pointeur à libérer.
typedef struct VekaState VekaState;

// Déclare les états de la fenêtre (rappelée avant chaque `body`).
typedef void (*VekaSetupFn)(struct VekaContext *cx, void *user);

// Construit le contenu de la fenêtre. L'élément renvoyé est consommé par Veka.
typedef struct VekaElement *(*VekaBodyFn)(struct VekaContext *cx, void *user);

// Une couleur sRGB, composantes de 0 à 1.
typedef struct VekaColor {
  float r;
  float g;
  float b;
  float a;
} VekaColor;

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

// Ouvre les fenêtres et bloque jusqu'à la fermeture de la dernière. Libère `app`.
// Renvoie 0, ou 1 en cas d'erreur (voir `veka_last_error`).
int32_t veka_app_run(struct VekaApp *app);

// Libère une application qui n'a pas été lancée.
void veka_app_free(struct VekaApp *app);

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

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* VEKA_H */
