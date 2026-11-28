/*
 * L'exemple de l'étape 10, en C : une longue liste qui défile, des cartes qui défilent de côté, et des
 * boutons qui vont en haut, au milieu et en bas. Comme crates/veka/examples/liste.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>

#include "veka.h"

#define LIGNES 100
#define CARTES 12

static void aller_en_haut(VekaContext *cx, void *user) {
    (void)cx;
    (void)user;
    veka_scroll_to(0, veka_unit_point(0.5f, 0.0f));
}

static void haut(VekaContext *cx, void *user) {
    /* Animé : dans `veka_with_animation`. */
    veka_with_animation(cx, veka_animation_smooth(), aller_en_haut, user);
}

static void milieu(VekaContext *cx, void *user) {
    (void)cx;
    (void)user;
    veka_scroll_to(50, veka_unit_point(0.5f, 0.5f));
}

static void aller_en_bas(VekaContext *cx, void *user) {
    (void)cx;
    (void)user;
    veka_scroll_to(LIGNES - 1, veka_unit_point(0.5f, 1.0f));
}

static void bas(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_smooth(), aller_en_bas, user);
}

static VekaElement *carte(int c) {
    char titre[32];
    snprintf(titre, sizeof titre, "Carte %d", c);
    VekaElement *couches[] = {
        veka_fill_paint(veka_rounded_rect(12), veka_token("Indigo", VEKA_HIERARCHY_PRIMARY)),
        veka_foreground(veka_text(titre), veka_hex(0xFFFFFF)),
    };
    return veka_frame(veka_zstack(couches, 2), 120, 80);
}

static VekaElement *ligne(int i) {
    char titre[32];
    snprintf(titre, sizeof titre, "Ligne %d", i);
    VekaElement *morceaux[] = {
        veka_frame(veka_fill_paint(veka_circle(), veka_token("Green", VEKA_HIERARCHY_PRIMARY)), 24, 24),
        veka_text(titre),
        veka_spacer(),
    };
    /* `veka_id` : la cible de `veka_scroll_to`. */
    return veka_id(veka_padding(veka_hstack(12, morceaux, 3), 6), i);
}

static VekaElement *body(VekaContext *cx, void *user) {
    (void)cx;
    VekaElement *cartes[CARTES];
    for (int c = 0; c < CARTES; c++) {
        cartes[c] = carte(c);
    }
    VekaElement *contenu[LIGNES + 1];
    contenu[0] = veka_scroll_view(VEKA_SCROLL_HORIZONTAL, veka_padding(veka_hstack(12, cartes, CARTES), 4));
    for (int i = 0; i < LIGNES; i++) {
        contenu[i + 1] = ligne(i);
    }
    VekaElement *boutons[] = {
        veka_button("Haut", haut, user),
        veka_button("Ligne 50", milieu, user),
        veka_button("Bas", bas, user),
    };
    VekaElement *lignes[] = {
        veka_font(veka_text("Liste"), VEKA_FONT_TITLE),
        veka_hstack(12, boutons, 3),
        veka_scroll_view(VEKA_SCROLL_VERTICAL, veka_vstack(8, contenu, LIGNES + 1)),
        veka_font(veka_text("Molette ou pavé tactile · Page préc. / suiv. · Origine / Fin"), VEKA_FONT_CAPTION),
    };
    return veka_padding(veka_vstack(12, lignes, 4), 24);
}

int main(void) {
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Liste", 520, 640, NULL, body, NULL);
    if (veka_app_run(app) != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
