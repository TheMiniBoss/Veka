/*
 * Anima en C : une carte à ressort, une liste avec transitions, une pastille qui change de place
 * (géométrie liée) et une pulsation écrite en timeline. Même esprit que
 * crates/veka/examples/anima.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>

#include "veka.h"

typedef struct {
    VekaState *ouvert;
    VekaState *prochain;
    VekaState *nombre;
    VekaState *droite;
    VekaState *pulsation;
    VekaTimeline *modele;
    int64_t ids[32];
    int nids;
    int battements;
} Anima;

/* --- Actions ------------------------------------------------------------------------------- */

static void basculer_carte(VekaContext *cx, void *user) {
    Anima *a = user;
    veka_state_set_bool(cx, a->ouvert, !veka_state_get_bool(cx, a->ouvert));
}

static void toucher_carte(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_bouncy(), basculer_carte, user);
}

/* La liste vit dans `ids` ; l'état `nombre` ne sert qu'à reconstruire la vue. */
static void ajouter_ligne(VekaContext *cx, void *user) {
    Anima *a = user;
    if (a->nids == 32) return;
    for (int i = a->nids; i > 0; i--) a->ids[i] = a->ids[i - 1];
    int64_t id = veka_state_get_int(cx, a->prochain);
    a->ids[0] = id;
    a->nids++;
    veka_state_set_int(cx, a->prochain, id + 1);
    veka_state_set_int(cx, a->nombre, a->nids);
}

static void retirer_ligne(VekaContext *cx, void *user) {
    Anima *a = user;
    if (a->nids == 0) return;
    a->nids--;
    veka_state_set_int(cx, a->nombre, a->nids);
}

static void ajouter(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_snappy(), ajouter_ligne, user);
}

static void retirer(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_smooth(), retirer_ligne, user);
}

static void changer_cote(VekaContext *cx, void *user) {
    Anima *a = user;
    veka_state_set_bool(cx, a->droite, !veka_state_get_bool(cx, a->droite));
}

static void deplacer(VekaContext *cx, void *user) {
    veka_with_animation(cx, veka_animation_spring(0.45f, 0.75f), changer_cote, user);
}

static void lecture(VekaContext *cx, void *user) {
    Anima *a = user;
    if (veka_timeline_is_playing(cx, a->pulsation)) {
        veka_timeline_pause(cx, a->pulsation);
    } else {
        veka_timeline_play(cx, a->pulsation);
    }
}

static void battement(VekaContext *cx, const char *nom, void *user) {
    (void)cx;
    (void)nom;
    Anima *a = user;
    a->battements++;
}

/* --- Vues ---------------------------------------------------------------------------------- */

static VekaElement *carte(VekaContext *cx, Anima *a) {
    bool o = veka_state_get_bool(cx, a->ouvert);
    VekaColor couleurs[2] = {veka_hex(o ? 0x5856D6 : 0x007AFF), veka_hex(o ? 0xFF2D55 : 0x00C7BE)};
    VekaPaint *degrade = veka_linear_gradient(couleurs, 2, VEKA_UNIT_TOP_LEADING, VEKA_UNIT_BOTTOM_TRAILING);
    VekaElement *forme = veka_frame(veka_fill_paint(veka_rounded_rect(o ? 24.0f : 12.0f), degrade), o ? 300.0f : 140.0f,
                                    o ? 120.0f : 64.0f);
    VekaAnimation douce = veka_animation_ease_in_out(0.25f);
    VekaElement *lignes[] = {
        veka_on_tap(forme, toucher_carte, a),
        veka_animation(veka_opacity(veka_font(veka_text("Touche la carte (ressort « bouncy »)"), VEKA_FONT_FOOTNOTE),
                                    o ? 0.4f : 1.0f),
                       &douce, o),
    };
    return veka_vstack(8, lignes, 2);
}

static VekaElement *liste(Anima *a) {
    VekaElement *lignes[32];
    for (int i = 0; i < a->nids; i++) {
        char texte[32];
        snprintf(texte, sizeof texte, "Élément %lld", (long long)a->ids[i]);
        VekaElement *contenu[] = {
            veka_frame(veka_fill_paint(veka_circle(), veka_token("Accent", VEKA_HIERARCHY_PRIMARY)), 10, 10),
            veka_text(texte),
        };
        VekaElement *ligne = veka_padding(veka_hstack(8, contenu, 2), 8);
        /* Un fond en capsule, par le schéma : background(paint:, shape: "capsule"). */
        VekaPaint *fond = veka_token("Fill", VEKA_HIERARCHY_PRIMARY);
        VekaParam fond_params[] = {{"paint", veka_paint_value(fond)}, {"shape", veka_enum("capsule")}};
        ligne = veka_modifier(ligne, "background", fond_params, 2);
        veka_paint_free(fond);
        /* Descend un peu en apparaissant (les lignes s'ajoutent en tête, sous les boutons), part en se
         * réduisant. */
        VekaTransition *t = veka_transition_asymmetric(
            veka_transition_combined(veka_transition_combined(veka_transition_offset(0, -12), veka_transition_scale(0.9f)),
                                     veka_transition_opacity()),
            veka_transition_combined(veka_transition_scale(0.8f), veka_transition_opacity()));
        lignes[i] = veka_id(veka_transition(ligne, t), a->ids[i]);
    }
    VekaElement *boutons[] = {veka_button("Ajouter", ajouter, a), veka_button("Retirer", retirer, a)};
    VekaElement *col[] = {veka_hstack(8, boutons, 2), veka_vstack(6, lignes, (size_t)a->nids)};
    return veka_vstack(8, col, 2);
}

/* Une pastille qui passe d'un côté à l'autre, et change de taille, avec matched_geometry_effect. */
static VekaElement *geometrie(VekaContext *cx, Anima *a) {
    bool d = veka_state_get_bool(cx, a->droite);
    VekaElement *pastille = veka_id(
        veka_matched_geometry_effect(
            veka_frame(veka_fill_paint(veka_circle(), veka_token("Orange", VEKA_HIERARCHY_PRIMARY)), d ? 44.0f : 20.0f,
                       d ? 44.0f : 20.0f),
            "pastille"),
        d ? 2 : 1);
    VekaElement *rail = veka_id(veka_frame(veka_fill_paint(veka_capsule(), veka_token("Fill", VEKA_HIERARCHY_PRIMARY)),
                                           180, 6),
                                0);
    VekaElement *ligne[2];
    if (d) {
        ligne[0] = rail;
        ligne[1] = pastille;
    } else {
        ligne[0] = pastille;
        ligne[1] = rail;
    }
    VekaElement *lignes[] = {veka_button("Déplacer", deplacer, a), veka_hstack(12, ligne, 2)};
    return veka_vstack(8, lignes, 2);
}

/* Une pulsation : échelle et opacité écrites en images clés, en boucle. */
static VekaElement *pulsation(VekaContext *cx, Anima *a) {
    veka_timeline_on_event(cx, a->pulsation, battement, a);
    float echelle = veka_timeline_value(cx, a->pulsation, "échelle");
    float opacite = veka_timeline_value(cx, a->pulsation, "opacité");
    char texte[48];
    snprintf(texte, sizeof texte, "Battements : %d", a->battements);
    VekaElement *coeur = veka_opacity(
        veka_scale_effect(veka_frame(veka_fill_paint(veka_circle(), veka_token("Pink", VEKA_HIERARCHY_PRIMARY)), 28, 28),
                          echelle),
        opacite);
    VekaElement *lignes[] = {
        veka_button(veka_timeline_is_playing(cx, a->pulsation) ? "Pause" : "Lecture", lecture, a),
        veka_frame(coeur, 44, 44),
        veka_font(veka_text(texte), VEKA_FONT_FOOTNOTE),
    };
    return veka_hstack(12, lignes, 3);
}

static void setup(VekaContext *cx, void *user) {
    Anima *a = user;
    a->ouvert = veka_state_new_bool(cx, false);
    a->prochain = veka_state_new_int(cx, 4);
    a->nombre = veka_state_new_int(cx, 3);
    a->droite = veka_state_new_bool(cx, false);
    a->pulsation = veka_state_new_timeline(cx, a->modele, true);
}

static VekaElement *body(VekaContext *cx, void *user) {
    Anima *a = user;
    VekaElement *gauche[] = {
        veka_font(veka_text("Anima en C"), VEKA_FONT_LARGE_TITLE),
        carte(cx, a),
        geometrie(cx, a),
        pulsation(cx, a),
    };
    VekaElement *colonnes[] = {veka_vstack(24, gauche, 4), liste(a)};
    VekaElement *contenu = veka_padding(veka_hstack(32, colonnes, 2), 24);
    VekaElement *fond = veka_fill_paint(veka_rectangle(), veka_token("WindowBackground", VEKA_HIERARCHY_PRIMARY));
    VekaElement *couches[] = {fond, contenu};
    return veka_zstack(couches, 2);
}

int main(void) {
    Anima a = {0};
    a.ids[0] = 1;
    a.ids[1] = 2;
    a.ids[2] = 3;
    a.nids = 3;

    a.modele = veka_timeline_new();
    veka_timeline_key(a.modele, "échelle", 0.0f, 1.0f, VEKA_CURVE_EASE_OUT);
    veka_timeline_key(a.modele, "échelle", 0.15f, 1.35f, VEKA_CURVE_EASE_IN_OUT);
    veka_timeline_key(a.modele, "échelle", 0.5f, 1.0f, VEKA_CURVE_LINEAR);
    veka_timeline_key(a.modele, "opacité", 0.0f, 0.6f, VEKA_CURVE_EASE_OUT);
    veka_timeline_key(a.modele, "opacité", 0.15f, 1.0f, VEKA_CURVE_EASE_IN_OUT);
    veka_timeline_key(a.modele, "opacité", 0.9f, 0.6f, VEKA_CURVE_LINEAR);
    veka_timeline_event(a.modele, 0.15f, "battement");
    veka_timeline_length(a.modele, 0.9f);
    veka_timeline_looping(a.modele, true);

    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Anima en C", 820, 560, setup, body, &a);
    int code = veka_app_run(app);
    veka_timeline_free(a.modele);
    if (code != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
