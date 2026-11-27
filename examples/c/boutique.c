/* Le langage .vk depuis le C : la vue vient de `boutique.vk`, les articles du C, et les achats
 * reviennent au C. En debug, le fichier est relu dès qu'il change.
 *
 *   cmake -S examples/c -B build/c && cmake --build build/c && build/c/boutique
 */
#include <stdio.h>

#include "veka.h"

#ifndef VEKA_VK_DIR
#define VEKA_VK_DIR "."
#endif

static VekaData *article(const char *nom, int64_t prix, bool nouveau) {
    VekaData *a = veka_data_record();
    veka_data_set(a, "nom", veka_data_string(nom));
    veka_data_set(a, "prix", veka_data_int(prix));
    veka_data_set(a, "nouveau", veka_data_bool(nouveau));
    return a;
}

static void achat(const char *view, const char *event, const VekaData *value, void *user) {
    char nom[64];
    int *achats = (int *)user;
    veka_data_text(value, nom, sizeof nom);
    *achats += 1;
    printf("%s.%s : %s (achat n° %d)\n", view, event, nom, *achats);
    fflush(stdout);
}

int main(void) {
    static int achats = 0;
    VekaApp *app = veka_app_new();
    VekaWindowDesc desc = veka_window_desc_default("Boutique (.vk depuis le C)");
    desc.width = 520.0f;
    desc.height = 360.0f;
    VekaVk *vk = veka_app_add_window_vk(app, &desc, VEKA_VK_DIR "/boutique.vk", "Boutique");
    if (vk == NULL) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
        veka_app_free(app);
        return 1;
    }

    VekaData *articles = veka_data_array();
    veka_data_push(articles, article("Épée", 12, false));
    veka_data_push(articles, article("Bouclier", 8, true));
    veka_data_push(articles, article("Potion", 3, false));
    VekaData *model = veka_data_record();
    veka_data_set(model, "articles", articles);
    veka_vk_set_model(vk, model);
    veka_vk_on_event(vk, achat, &achats);

    int code = veka_app_run(app);
    if (code != 0) {
        fprintf(stderr, "Veka : %s\n", veka_last_error());
    }
    veka_vk_free(vk);
    return code;
}
