#pragma once
#include <cstdint>
#include <string>

#include "logic/settings.h"

namespace pup {

enum class Need { Bouger, Jouer, Dormir };

// Besoins du chiot, comptes en jours (int64_t, independants de clock.h).
//
// Regles :
// - Avant le premier on_day, aucune journee de reference n'existe : le premier
//   on_day(d) enregistre seulement d, sans decroissance (une carte qui demarre
//   au jour 20000 ne doit pas decroitre de 20000 jours).
// - on_day(d) avec d <= dernier jour ne fait rien. Sinon, pour chaque jour k de
//   dernier+1 a d, chaque besoin non satisfait le jour k-1 perd 15 (plancher 0).
// - satisfy(n, jour) appelle d'abord on_day(jour), donc le jour memorise est
//   toujours le dernier jour. +20 plafonne a 100 ; seule la satisfaction du
//   dernier jour compte pour la decroissance et la bouderie. Si jour est
//   anterieur au dernier jour (evenement tardif) : +20 mais aucune marque.
// - sulking() : un besoin < 30 et aucun besoin satisfait le dernier jour.
// - Le chiot ne meurt jamais : pas d'etat terminal, tout satisfy met fin a la
//   bouderie (A1).
class Needs {
public:
    int level(Need n) const;           // 0 a 100, depart 70
    int icon(Need n) const;            // 0 bas (< 30), 1 moyen, 2 haut (>= 70)
    void satisfy(Need n, int64_t day); // avance au jour, +20 plafonne a 100, marque
    void on_day(int64_t day);
    void set_settings(const Settings &s); // baisse par jour et gain ; niveaux conserves
    bool sulking() const;
    std::string save() const;          // une ligne de texte
    bool load(const std::string &s);   // false et etat inchange si invalide

private:
    static constexpr int64_t kNone = INT64_MIN;
    Settings cfg_;
    int level_[3] = {70, 70, 70};
    int64_t satisfied_[3] = {kNone, kNone, kNone};
    bool has_last_ = false;
    int64_t last_day_ = 0;
};

} // namespace pup
