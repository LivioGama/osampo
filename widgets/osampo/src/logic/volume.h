#pragma once
#include <string>

// Volume du chiot (SP5e), regle au bouton BOOT de la board : 0 = coupe, 1..3.
namespace pup {

constexpr int kVolumeMax = 3;
constexpr int kVolumeDefaut = 2; // = l'ancien volume fixe (70 %, -6,5 dB)

// Niveau suivant au bouton : coupe -> 1 -> 2 -> 3 -> coupe ; hors bornes -> defaut.
int volume_suivant(int niveau);
// Valeur persistee en NVS : un chiffre 0..3, sinon le niveau par defaut.
int volume_lire(const std::string &s);
std::string volume_ecrire(int niveau);
// Gain du codec pour un niveau 1..3 : pas de 6 dB autour du defaut (choix Claude).
float volume_db(int niveau);

} // namespace pup
