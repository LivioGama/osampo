#pragma once
#include <string>
#include <vector>

#include "json_line.h"
#include "logic/rappel.h"

namespace pup {

// Reglages du chiot, avec leurs valeurs par defaut (celles du code d origine).
struct Settings {
    int report_min = 10, rattrapage_min = 60;
    int relances_s[3] = {60, 180, 420};
    int nb_relances = 3;
    int calme_validation_min = 10;
    int patte_avant_aboie_min = 2; // reclamation Claude : patte puis aboie (SPEC-012)
    int calme_debut = 22 * 60, calme_fin = 7 * 60, nuit_debut = 20 * 60, nuit_fin = 7 * 60;
    int lum_jour = 100, lum_nuit = 20, baisse_jour = 15, gain = 20;
    int lancer_mg = 2000, lancer_dps = 300, lancer_ms = 20;
    int secousse_mg = 2000, secousse_coups = 4, secousse_fenetre_ms = 1000;
    int retourne_ms = 1500, pause_gestes_ms = 1500, journal = 0;
    int courte_min = 5, seuil_reclame = 30, retour_min = 2; // promenade (SP5h)
    int batterie_faible_mv = 0;                             // 0 : pas de repere
    char k_lancer = 't', k_secousse = 's', k_retourner = 'f';
    std::vector<Reminder> rappels = default_reminders();

    // Applique les cles presentes et valides ; une cle invalide garde sa valeur
    // et ajoute un message "cle: raison" a la liste rendue. Les relances et les
    // touches sont validees en bloc. Les cles inconnues sont ignorees.
    std::vector<std::string> apply(const JsonLine &ligne);

    // {"cfg":1,...} : ordre de la table du plan, sans espaces.
    std::string to_line() const;
};

// [debut, fin) a cheval sur minuit ; debut == fin -> false.
bool in_range(int minute, int debut, int fin);

} // namespace pup
