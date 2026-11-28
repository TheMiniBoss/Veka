/*
 * L'exemple de l'étape 9, en C : nom, e-mail et mot de passe. Tab passe d'un champ à l'autre, Entrée
 * valide et Échap efface. Même vue que crates/veka/examples/formulaire.rs.
 *
 * Construire et lancer : voir examples/c/CMakeLists.txt.
 */
#include <stdio.h>
#include <string.h>

#include "veka.h"

typedef struct {
    VekaState *nom;
    VekaState *email;
    VekaState *mdp;
    VekaState *message;
} Formulaire;

static void valider(VekaContext *cx, void *user) {
    Formulaire *f = user;
    char nom[256], email[256], texte[600];
    /* `veka_state_get_string` ne garde son texte que jusqu'à l'appel suivant : on le copie. */
    snprintf(nom, sizeof nom, "%s", veka_state_get_string(cx, f->nom));
    snprintf(email, sizeof email, "%s", veka_state_get_string(cx, f->email));
    if (nom[0] == '\0') {
        veka_state_set_string(cx, f->message, "Il manque le nom.");
    } else {
        snprintf(texte, sizeof texte, "Bienvenue, %s (%s) !", nom, email);
        veka_state_set_string(cx, f->message, texte);
    }
}

static void effacer(VekaContext *cx, void *user) {
    Formulaire *f = user;
    veka_state_set_string(cx, f->nom, "");
    veka_state_set_string(cx, f->email, "");
    veka_state_set_string(cx, f->mdp, "");
    veka_state_set_string(cx, f->message, "Formulaire effacé.");
}

/* Les états de la vue, toujours déclarés dans le même ordre. */
static void setup(VekaContext *cx, void *user) {
    Formulaire *f = user;
    f->nom = veka_state_new_string(cx, "");
    f->email = veka_state_new_string(cx, "");
    f->mdp = veka_state_new_string(cx, "");
    f->message = veka_state_new_string(cx, "Remplissez le formulaire, puis Entrée.");
}

static VekaElement *body(VekaContext *cx, void *user) {
    Formulaire *f = user;
    VekaElement *boutons[] = {
        veka_keyboard_shortcut(veka_button("Effacer", effacer, f), "escape", 0),
        veka_button("Valider", valider, f),
    };
    VekaElement *lignes[] = {
        veka_font(veka_text("Inscription"), VEKA_FONT_TITLE),
        veka_text("Nom"),
        veka_frame(veka_text_field("Ana Martin", f->nom), 280, -1),
        veka_text("E-mail"),
        veka_frame(veka_text_field("ana@exemple.fr", f->email), 280, -1),
        veka_text("Mot de passe"),
        veka_frame(veka_secure_field("Au moins 8 caractères", f->mdp), 280, -1),
        veka_font(veka_text(veka_state_get_string(cx, f->message)), VEKA_FONT_HEADLINE),
        veka_hstack(12, boutons, 2),
        veka_font(veka_text("Tab : champ suivant · Entrée : valider · Échap : effacer"), VEKA_FONT_CAPTION),
    };
    /* Une pile alignée à gauche, par l'accès direct au schéma. */
    VekaParam pile[] = {
        {"alignment", veka_enum("leading")},
        {"spacing", veka_number(12)},
    };
    VekaElement *vue = veka_element("VStack", pile, 2, lignes, sizeof lignes / sizeof lignes[0]);
    return veka_padding(veka_on_submit(vue, valider, f), 24);
}

int main(void) {
    Formulaire f = {0};
    VekaApp *app = veka_app_new();
    veka_app_add_window(app, "Formulaire", 420, 520, setup, body, &f);
    if (veka_app_run(app) != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        return 1;
    }
    return 0;
}
