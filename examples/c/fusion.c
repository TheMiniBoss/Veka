/*
 * Modes de fusion en C : un même motif dans chaque case, mélangé avec le dégradé de la case par un mode
 * différent ; en bas, une opacité sans et avec groupe. Même idée que crates/veka/examples/fusion.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>

#include "veka.h"

static const char *const NOMS[21] = {
    "normal",     "multiply",   "screen",     "overlay",    "darken",     "lighten",    "colorDodge",
    "colorBurn",  "softLight",  "hardLight",  "difference", "exclusion",  "hue",        "saturation",
    "color",      "luminosity", "sourceAtop", "destinationOver", "destinationOut", "plusDarker", "plusLighter",
};

typedef struct {
    VekaState *tour; /* angle du motif, en degrés */
} Fusion;

static void setup(VekaContext *cx, void *user) {
    Fusion *f = user;
    f->tour = veka_state_new_number(cx, 0.0);
}

static void ajouter(VekaContext *cx, void *user) {
    Fusion *f = user;
    veka_state_set_number(cx, f->tour, veka_state_get_number(cx, f->tour) + 90.0);
}

static void tourner(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_smooth(), ajouter, user);
}

static VekaElement *cercle(VekaColor c, float x, float y) {
    return veka_offset(veka_frame(veka_fill(veka_circle(), c), 56, 56), x, y);
}

static VekaElement *une_case(int mode, float angle) {
    VekaColor couleurs[] = {veka_hex(0xFFCC00), veka_hex(0x30B0C7)};
    VekaElement *fond = veka_fill_paint(
        veka_rounded_rect(12),
        veka_linear_gradient(couleurs, 2, VEKA_UNIT_TOP_LEADING, VEKA_UNIT_BOTTOM_TRAILING));
    VekaElement *cercles[] = {cercle(veka_hex(0xFF2D55), -14, -8), cercle(veka_hex(0x5856D6), 14, 8)};
    VekaElement *motif = veka_blend_mode(veka_rotation_effect(veka_zstack(cercles, 2), angle), (VekaBlendMode)mode);
    VekaElement *couches[] = {fond, motif};
    /* Le mélange reste dans la case (les découpes ne percent pas la fenêtre). */
    VekaElement *carre = veka_compositing_group(veka_frame(veka_zstack(couches, 2), 120, 96));
    VekaElement *lignes[] = {carre, veka_font(veka_text(NOMS[mode]), VEKA_FONT_CAPTION)};
    return veka_vstack(6, lignes, 2);
}

static VekaElement *opacite(bool groupe) {
    VekaElement *cercles[] = {
        veka_offset(veka_frame(veka_fill(veka_circle(), veka_hex(0x007AFF)), 60, 60), -18, 0),
        veka_offset(veka_frame(veka_fill(veka_circle(), veka_hex(0x007AFF)), 60, 60), 18, 0),
    };
    VekaElement *paire = veka_frame(veka_zstack(cercles, 2), 120, 64);
    if (groupe) {
        paire = veka_compositing_group(paire);
    }
    VekaElement *lignes[] = {
        veka_opacity(paire, 0.5f),
        veka_font(veka_text(groupe ? "compositing_group + opacity(0.5)" : "opacity(0.5)"), VEKA_FONT_CAPTION),
    };
    return veka_vstack(6, lignes, 2);
}

static VekaElement *body(VekaContext *cx, void *user) {
    Fusion *f = user;
    float angle = (float)veka_state_get_number(cx, f->tour);
    VekaElement *lignes[5];
    VekaElement *titre[] = {veka_font(veka_text("Modes de fusion"), VEKA_FONT_TITLE2),
                            veka_button("Tourner", tourner, f)};
    lignes[0] = veka_hstack(16, titre, 2);
    for (int r = 0; r < 3; r++) {
        VekaElement *cases[7];
        for (int c = 0; c < 7; c++) {
            cases[c] = une_case(r * 7 + c, angle);
        }
        lignes[1 + r] = veka_hstack(10, cases, 7);
    }
    VekaElement *paires[] = {opacite(false), opacite(true)};
    lignes[4] = veka_hstack(40, paires, 2);
    return veka_padding(veka_vstack(16, lignes, 5), 24);
}

int main(void) {
    Fusion f = {0};
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Modes de fusion en C", 980, 600, setup, body, &f);
    int code = veka_app_run(app);
    if (code != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
