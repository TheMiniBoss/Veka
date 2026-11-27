/*
 * Une fenêtre entièrement dessinée par Veka, en C : barre de titre, boutons, contenu.
 * Même vue que crates/veka/examples/fenetre_perso.rs.
 */
#include <stdio.h>

#include "veka.h"

typedef struct {
    VekaState *clics;
} Etat;

static void incrementer(VekaContext *cx, void *user) {
    Etat *e = user;
    veka_state_set_int(cx, e->clics, veka_state_get_int(cx, e->clics) + 1);
}

static void setup(VekaContext *cx, void *user) {
    Etat *e = user;
    e->clics = veka_state_new_int(cx, 0);
}

static VekaElement *body(VekaContext *cx, void *user) {
    Etat *e = user;
    char libelle[64];
    snprintf(libelle, sizeof libelle, "Clics : %lld", (long long)veka_state_get_int(cx, e->clics));

    VekaElement *formes[] = {
        veka_frame(veka_fill(veka_rounded_rect(12), veka_hex(0x007AFF)), 80, 48),
        veka_frame(veka_fill(veka_capsule(), veka_hex(0xFF9500)), 96, 48),
        veka_frame(veka_fill(veka_circle(), veka_hex(0x34C759)), 48, 48),
    };
    VekaElement *contenu[] = {
        veka_font(veka_text("Aucune décoration du système"), VEKA_FONT_TITLE2),
        veka_text("Tire la barre pour déplacer, double-clique pour agrandir."),
        veka_hstack(12, formes, 3),
        veka_button(libelle, incrementer, e),
    };
    VekaElement *lignes[] = {
        veka_title_bar("Veka — fenêtre personnalisée"),
        veka_padding(veka_vstack(16, contenu, 4), 24),
        veka_spacer(),
    };
    return veka_vstack(0, lignes, 3);
}

int main(void) {
    Etat e = {0};
    VekaWindowDesc desc = veka_window_desc_default("Veka");
    desc.width = 560;
    desc.height = 380;
    desc.min_width = 360;
    desc.min_height = 240;
    desc.chrome = VEKA_CHROME_CUSTOM;

    VekaApp *app = veka_app_new();
    veka_app_add_window_ex(app, &desc, setup, body, &e);
    if (veka_app_run(app) != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
