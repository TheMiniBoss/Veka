/*
 * L'exemple de l'étape 1, en C : une fenêtre avec du texte, des formes, des piles et un bouton
 * qui compte les clics. Même vue que crates/veka/examples/compteur.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>

#include "veka.h"

typedef struct {
    VekaState *clics;
} Compteur;

static void incrementer(VekaContext *cx, void *user) {
    Compteur *c = user;
    veka_state_set_int(cx, c->clics, veka_state_get_int(cx, c->clics) + 1);
}

/* Les états de la vue, toujours déclarés dans le même ordre. */
static void setup(VekaContext *cx, void *user) {
    Compteur *c = user;
    c->clics = veka_state_new_int(cx, 0);
}

/* La vue : rappelée par Veka quand un état change. */
static VekaElement *body(VekaContext *cx, void *user) {
    Compteur *c = user;
    char libelle[64];
    snprintf(libelle, sizeof libelle, "Clics : %lld", (long long)veka_state_get_int(cx, c->clics));

    VekaElement *formes[] = {
        veka_frame(veka_fill(veka_rounded_rect(12), veka_hex(0x007AFF)), 80, 48),
        veka_frame(veka_fill(veka_capsule(), veka_hex(0xFF9500)), 96, 48),
        veka_frame(veka_fill(veka_circle(), veka_hex(0x34C759)), 48, 48),
    };
    VekaElement *lignes[] = {
        veka_font(veka_text("Bonjour Veka"), VEKA_FONT_TITLE),
        veka_hstack(12, formes, 3),
        veka_button(libelle, incrementer, c),
    };
    return veka_padding(veka_vstack(16, lignes, 3), 24);
}

int main(void) {
    Compteur c = {0};
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Compteur", 480, 320, setup, body, &c);
    if (veka_app_run(app) != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
