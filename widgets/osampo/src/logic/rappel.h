#pragma once
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

namespace pup {

enum class Category { Sortie, Sport, Repas, Dodo };

struct Reminder {
    Category cat;
    int minute;      // minute du jour
    uint8_t days;    // bit 0 = lundi
};

inline bool operator==(const Reminder &a, const Reminder &b)
{
    return a.cat == b.cat && a.minute == b.minute && a.days == b.days;
}

constexpr size_t kMaxRappels = 16;

std::vector<Reminder> default_reminders();

// Texte du fil (cle "rappels" de la ligne cfg, SP5h) : rappels separes par ';',
// chacun "categorie,HHMM,masque" (masque decimal 1..127, bit 0 = lundi).
std::string reminders_to_text(const std::vector<Reminder> &l);
// nullopt si vide, mal forme ou plus de kMaxRappels rappels.
std::optional<std::vector<Reminder>> reminders_from_text(const std::string &s);

} // namespace pup
