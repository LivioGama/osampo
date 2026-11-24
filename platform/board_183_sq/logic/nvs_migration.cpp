#include "nvs_migration.h"

#include <algorithm>

namespace board_183_sq {

std::vector<std::string> cles_a_migrer(const std::vector<std::string> &anciennes,
                                       const std::vector<std::string> &nouvelles)
{
    std::vector<std::string> out;
    for (const auto &k : anciennes)
        if (std::find(nouvelles.begin(), nouvelles.end(), k) == nouvelles.end()) out.push_back(k);
    return out;
}

std::vector<std::string> cles_a_effacer(const std::vector<std::string> &anciennes,
                                        const std::vector<std::string> &echouees)
{
    return cles_a_migrer(anciennes, echouees);
}

} // namespace board_183_sq
