/*
 * Verre en C : verre dépoli, verre liquide, gouttes qui fusionnent et eau, au-dessus d'un fond
 * coloré. Même idée que crates/veka/examples/verre.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>

#include "veka.h"

typedef struct {
    VekaState *clarte; /* 0 = teinté, 1 = par défaut (au départ), 2 = clair */
    VekaState *serre;
} Verre;

static void setup(VekaContext *cx, void *user) {
    Verre *v = user;
    v->clarte = veka_state_new_int(cx, 1);
    v->serre = veka_state_new_bool(cx, false);
}

static void changer_clarte(VekaContext *cx, void *user) {
    Verre *v = user;
    veka_state_set_int(cx, v->clarte, (veka_state_get_int(cx, v->clarte) + 1) % 3);
}

static void basculer_serre(VekaContext *cx, void *user) {
    Verre *v = user;
    veka_state_set_bool(cx, v->serre, !veka_state_get_bool(cx, v->serre));
}

static void clarte(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_smooth(), changer_clarte, user);
}

static void serrer(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_spring(0.5f, 0.7f), basculer_serre, user);
}

/* Un dégradé et des traits nets : le flou et la réfraction se voient. */
static VekaElement *fond(void) {
    VekaColor couleurs[] = {veka_hex(0x5856D6), veka_hex(0x30B0C7)};
    VekaElement *degrade = veka_fill_paint(
        veka_rectangle(), veka_linear_gradient(couleurs, 2, VEKA_UNIT_TOP_LEADING, VEKA_UNIT_BOTTOM_TRAILING));
    VekaElement *traits[40];
    for (int i = 0; i < 40; i++) {
        traits[i] = veka_frame(veka_fill(veka_capsule(), i % 2 == 0 ? veka_hex(0xFFFFFF) : veka_hex(0x000000)), 6, 260);
    }
    VekaElement *bandes = veka_rotation_effect(veka_hstack(14, traits, 40), -20);
    VekaElement *couches[] = {degrade, bandes};
    return veka_zstack(couches, 2);
}

static VekaElement *panneau(void) {
    VekaElement *lignes[] = {
        veka_font(veka_text("Verre dépoli"), VEKA_FONT_TITLE2),
        veka_font(veka_text("Le fond est flouté, teinté et un peu granuleux."), VEKA_FONT_FOOTNOTE),
    };
    return veka_frosted_glass(veka_frame(veka_padding(veka_vstack(10, lignes, 2), 20), 300, 150));
}

static VekaElement *carte(VekaContext *cx, Verre *v) {
    static const char *const noms[] = {"Clarté : Teinté", "Clarté : Par défaut", "Clarté : Clair"};
    static const float valeurs[] = {0.0f, 0.4f, 0.9f};
    int64_t c = veka_state_get_int(cx, v->clarte);
    VekaElement *lignes[] = {
        veka_font(veka_text("Liquid Glass"), VEKA_FONT_TITLE2),
        veka_font(veka_text(noms[c]), VEKA_FONT_FOOTNOTE),
        veka_button("Changer", clarte, v),
    };
    VekaGlass g = veka_glass_regular();
    g.clarity = valeurs[c];
    g.interactive = 1;
    g.shape = VEKA_BACKDROP_SHAPE_ROUNDED_RECTANGLE;
    g.corner_radius = 28;
    return veka_glass(veka_frame(veka_padding(veka_vstack(10, lignes, 3), 22), 240, 150), &g, NULL);
}

static VekaElement *goutte(void) {
    VekaGlass g = veka_glass_clear();
    g.shape = VEKA_BACKDROP_SHAPE_CIRCLE;
    VekaElement *eau = veka_frame(veka_font(veka_text("Eau"), VEKA_FONT_HEADLINE), 110, 110);
    eau = veka_liquid_distortion(eau, 10, 90, 1, VEKA_BACKDROP_SHAPE_CIRCLE, 0);
    return veka_glass(eau, &g, NULL);
}

/* Trois onglets dans un groupe : rapprochés, leurs verres fusionnent. */
static VekaElement *barre(VekaContext *cx, Verre *v) {
    static const char *const noms[] = {"Accueil", "Recherche", "Réglages"};
    bool serre = veka_state_get_bool(cx, v->serre);
    VekaGlass g = veka_glass_regular();
    g.interactive = 1;
    VekaElement *onglets[3];
    for (int i = 0; i < 3; i++) {
        onglets[i] = veka_glass(veka_padding(veka_text(noms[i]), 12), &g, NULL);
    }
    VekaElement *groupe = veka_glass_group(veka_hstack(serre ? 2 : 18, onglets, 3), 14, &g, NULL);
    VekaElement *lignes[] = {groupe, veka_button(serre ? "Écarter" : "Rapprocher", serrer, v)};
    return veka_vstack(12, lignes, 2);
}

static VekaElement *body(VekaContext *cx, void *user) {
    Verre *v = user;
    VekaElement *haut[] = {panneau(), carte(cx, v), goutte()};
    VekaElement *lignes[] = {veka_hstack(28, haut, 3), barre(cx, v)};
    VekaElement *couches[] = {fond(), veka_vstack(28, lignes, 2)};
    return veka_zstack(couches, 2);
}

int main(void) {
    Verre v = {0};
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Verre en C", 900, 560, setup, body, &v);
    int code = veka_app_run(app);
    if (code != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
