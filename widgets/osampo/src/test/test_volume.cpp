#include "logic/volume.h"

#include <cassert>
#include <cstdio>

using namespace pup;

int main()
{
    // Bouton BOOT : coupe -> 1 -> 2 -> 3 -> coupe.
    assert(volume_suivant(0) == 1 && volume_suivant(1) == 2 && volume_suivant(2) == 3 && volume_suivant(3) == 0);
    assert(volume_suivant(7) == kVolumeDefaut && volume_suivant(-1) == kVolumeDefaut);
    // Lecture NVS : un chiffre 0..3, sinon le niveau par defaut.
    assert(volume_lire("0") == 0 && volume_lire("3") == 3);
    assert(volume_lire("") == kVolumeDefaut && volume_lire("4") == kVolumeDefaut && volume_lire("2x") == kVolumeDefaut);
    assert(volume_ecrire(1) == "1");
    // Pas de 6 dB autour du reglage valide (70 % = -6,5 dB, niveau 2 par defaut).
    assert(kVolumeDefaut == 2);
    assert(volume_db(1) == -12.5f && volume_db(2) == -6.5f && volume_db(3) == -0.5f);
    std::puts("test_volume OK");
    return 0;
}
