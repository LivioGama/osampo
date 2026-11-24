#include "logic/volume.h"

namespace pup {

int volume_suivant(int niveau)
{
    if (niveau < 0 || niveau > kVolumeMax) return kVolumeDefaut;
    return niveau == kVolumeMax ? 0 : niveau + 1;
}

int volume_lire(const std::string &s)
{
    if (s.size() != 1 || s[0] < '0' || s[0] > '0' + kVolumeMax) return kVolumeDefaut;
    return s[0] - '0';
}

std::string volume_ecrire(int niveau)
{
    return std::to_string(niveau);
}

float volume_db(int niveau)
{
    switch (niveau) {
    case 1: return -12.5f;
    case 3: return -0.5f;
    default: return -6.5f;
    }
}

} // namespace pup
