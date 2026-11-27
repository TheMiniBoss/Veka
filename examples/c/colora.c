/*
 * Colora en C : jetons du thème, les cinq dégradés, un thème maison, le mode clair / sombre et les
 * réglages de couleur. Même vue que crates/veka/examples/colora.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>

#include "veka.h"

typedef struct {
    VekaState *mode; /* 0 = comme le système, 1 = clair, 2 = sombre */
    VekaState *gris;
    VekaTheme *marque;
} Colora;

static void changer_mode(VekaContext *cx, void *user) {
    Colora *c = user;
    veka_state_set_int(cx, c->mode, (veka_state_get_int(cx, c->mode) + 1) % 3);
}

static void changer_gris(VekaContext *cx, void *user) {
    Colora *c = user;
    veka_state_set_bool(cx, c->gris, !veka_state_get_bool(cx, c->gris));
}

static void rien(VekaContext *cx, void *user) {
    (void)cx;
    (void)user;
}

static void setup(VekaContext *cx, void *user) {
    Colora *c = user;
    c->mode = veka_state_new_int(cx, 0);
    c->gris = veka_state_new_bool(cx, false);
}

/* Un jeton comme arrêt de dégradé. */
static VekaStop jeton(const char *nom, float position) {
    VekaStop s = {veka_rgba(0, 0, 0, 0), nom, VEKA_HIERARCHY_PRIMARY, position};
    return s;
}

static VekaElement *nuancier(void) {
    static const char *const palette[] = {"Red",  "Orange", "Yellow", "Green",  "Mint", "Teal",
                                          "Cyan", "Blue",   "Indigo", "Purple", "Pink", "Brown"};
    VekaElement *pastilles[12];
    for (int i = 0; i < 12; i++) {
        pastilles[i] = veka_frame(veka_fill_paint(veka_circle(), veka_token(palette[i], VEKA_HIERARCHY_PRIMARY)),
                                  28, 28);
    }
    return veka_hstack(8, pastilles, 12);
}

static VekaElement *carte(const char *titre, VekaPaint *peinture) {
    VekaElement *lignes[] = {
        veka_frame(veka_fill_paint(veka_rounded_rect(14), peinture), 112, 84),
        veka_foreground_paint(veka_font(veka_text(titre), VEKA_FONT_FOOTNOTE),
                              veka_token("Foreground", VEKA_HIERARCHY_SECONDARY)),
    };
    return veka_vstack(6, lignes, 2);
}

static VekaElement *degrades(void) {
    /* Des dégradés de jetons : couleurs provisoires, puis arrêts remplacés par des jetons. */
    VekaColor deux[2] = {0};
    VekaStop bleu_violet[] = {jeton("Blue", 0), jeton("Purple", 1)};
    VekaPaint *lineaire = veka_paint_stops(
        veka_linear_gradient(deux, 2, VEKA_UNIT_TOP_LEADING, VEKA_UNIT_BOTTOM_TRAILING), bleu_violet, 2);

    VekaStop feu[] = {jeton("Yellow", 0), jeton("Orange", 0.5f), jeton("Red", 1)};
    VekaPaint *radial = veka_paint_stops(veka_radial_gradient(deux, 2, VEKA_UNIT_CENTER, 0, 60), feu, 3);

    VekaColor ciel[] = {veka_hex(0xFFFFFF), veka_hex(0x5AC8FA), veka_hex(0x0040DD)};
    VekaPaint *elliptique = veka_elliptical_gradient(ciel, 3, VEKA_UNIT_CENTER, 0, 1);

    VekaStop arc[] = {jeton("Red", 0),      jeton("Yellow", 1.0f / 6), jeton("Green", 2.0f / 6),
                      jeton("Cyan", 0.5f),  jeton("Blue", 4.0f / 6),   jeton("Purple", 5.0f / 6),
                      jeton("Red", 1)};
    VekaPaint *angulaire = veka_paint_stops(veka_angular_gradient(deux, 2, VEKA_UNIT_CENTER, 0, 360), arc, 7);

    VekaColor maille[] = {
        veka_hex(0xFF2D55), veka_hex(0xFF9500), veka_hex(0xFFCC00),
        veka_hex(0xAF52DE), veka_hex(0xFFFFFF), veka_hex(0x34C759),
        veka_hex(0x5856D6), veka_hex(0x007AFF), veka_hex(0x00C7BE),
    };
    VekaPaint *mesh = veka_mesh_gradient(3, 3, NULL, maille);

    VekaElement *cartes[] = {
        carte("Linéaire", lineaire), carte("Radial", radial),   carte("Elliptique", elliptique),
        carte("Angulaire", angulaire), carte("Maillé", mesh),
    };
    return veka_hstack(12, cartes, 5);
}

static VekaTheme *marque(void) {
    VekaTheme *t = veka_theme_new("Marque", NULL);
    veka_theme_set_color(t, "Tint", veka_solid(veka_hex(0xFF6A00)), veka_solid(veka_hex(0xFF8A33)));
    VekaColor clair[] = {veka_hex(0xFF6A00), veka_hex(0xFF2D55)};
    VekaColor sombre[] = {veka_hex(0x7A2E00), veka_hex(0x4A0018)};
    veka_theme_set_color(t, "Marque.Bandeau",
                         veka_linear_gradient(clair, 2, VEKA_UNIT_LEADING, VEKA_UNIT_TRAILING),
                         veka_linear_gradient(sombre, 2, VEKA_UNIT_LEADING, VEKA_UNIT_TRAILING));
    return t;
}

static VekaElement *section(const char *titre, VekaElement *contenu) {
    VekaElement *lignes[] = {veka_font(veka_text(titre), VEKA_FONT_HEADLINE), contenu};
    return veka_vstack(8, lignes, 2);
}

/* Applique le mode choisi (0 : celui du système). */
static VekaElement *avec_mode(VekaElement *e, int64_t mode) {
    if (mode == 1) return veka_color_scheme(e, VEKA_COLOR_SCHEME_LIGHT);
    if (mode == 2) return veka_color_scheme(e, VEKA_COLOR_SCHEME_DARK);
    return e;
}

static VekaElement *body(VekaContext *cx, void *user) {
    Colora *c = user;
    int64_t mode = veka_state_get_int(cx, c->mode);
    bool gris = veka_state_get_bool(cx, c->gris);
    const char *libelle = mode == 0 ? "Mode : système" : mode == 1 ? "Mode : clair" : "Mode : sombre";

    VekaElement *theme_items[] = {
        veka_button("Valider", rien, NULL),
        veka_frame(veka_fill_paint(veka_capsule(), veka_token("Marque.Bandeau", VEKA_HIERARCHY_PRIMARY)), 160, 36),
    };
    VekaElement *boutons[] = {
        veka_button(libelle, changer_mode, c),
        veka_button(gris ? "Couleurs" : "Niveaux de gris", changer_gris, c),
    };
    VekaElement *lignes[] = {
        veka_font(veka_text("Colora"), VEKA_FONT_LARGE_TITLE),
        section("Jetons", nuancier()),
        section("Dégradés (Oklab)", degrades()),
        section("Thème « Marque » sur cette partie", veka_theme(veka_hstack(12, theme_items, 2), c->marque)),
        veka_hstack(12, boutons, 2),
    };
    VekaElement *contenu = avec_mode(veka_padding(veka_vstack(20, lignes, 5), 24), mode);
    if (gris) contenu = veka_grayscale(contenu, 1);

    /* Le fond de toute la fenêtre suit le mode choisi. */
    VekaElement *fond = avec_mode(veka_fill_paint(veka_rectangle(), veka_token("WindowBackground", VEKA_HIERARCHY_PRIMARY)), mode);
    VekaElement *couches[] = {fond, contenu};
    return veka_zstack(couches, 2);
}

int main(void) {
    Colora c = {0};
    c.marque = marque();
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Colora", 680, 520, setup, body, &c);
    int code = veka_app_run(app);
    veka_theme_free(c.marque);
    if (code != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
