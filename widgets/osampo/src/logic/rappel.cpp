#include "logic/rappel.h"

namespace pup {

namespace {

const char *NOMS[] = {"sortie", "sport", "repas", "dodo"};

std::optional<int> nombre(const std::string &s)
{
    if (s.empty() || s.size() > 4) return std::nullopt;
    int v = 0;
    for (char c : s) {
        if (c < '0' || c > '9') return std::nullopt;
        v = v * 10 + (c - '0');
    }
    return v;
}

} // namespace

std::vector<Reminder> default_reminders()
{
    return {
        {Category::Repas, 12 * 60, 0x7F},
        {Category::Sortie, 12 * 60 + 30, 0x7F},
        {Category::Sport, 18 * 60, 0x01 | 0x04 | 0x10}, // lun, mer, ven
        {Category::Repas, 19 * 60 + 30, 0x7F},
        {Category::Dodo, 21 * 60 + 45, 0x7F},
    };
}

std::string reminders_to_text(const std::vector<Reminder> &l)
{
    std::string s;
    for (const Reminder &r : l) {
        if (!s.empty()) s += ';';
        char hm[5];
        hm[0] = static_cast<char>('0' + r.minute / 600);
        hm[1] = static_cast<char>('0' + r.minute / 60 % 10);
        hm[2] = static_cast<char>('0' + r.minute % 60 / 10);
        hm[3] = static_cast<char>('0' + r.minute % 10);
        hm[4] = 0;
        s += NOMS[static_cast<int>(r.cat)];
        s += ',';
        s += hm;
        s += ',';
        s += std::to_string(r.days);
    }
    return s;
}

std::optional<std::vector<Reminder>> reminders_from_text(const std::string &s)
{
    std::vector<Reminder> l;
    size_t i = 0;
    if (s.empty()) return std::nullopt;
    while (true) {
        size_t fin = s.find(';', i);
        std::string r = s.substr(i, fin == std::string::npos ? std::string::npos : fin - i);
        size_t a = r.find(','), b = a == std::string::npos ? a : r.find(',', a + 1);
        if (b == std::string::npos) return std::nullopt;
        std::string nom = r.substr(0, a), hm = r.substr(a + 1, b - a - 1);
        const int masque = nombre(r.substr(b + 1)).value_or(0);
        const int h = hm.size() == 4 ? nombre(hm).value_or(-1) : -1;
        int cat = -1;
        for (int k = 0; k < 4; ++k)
            if (nom == NOMS[k]) cat = k;
        if (cat < 0 || h < 0 || masque < 1 || masque > 127) return std::nullopt;
        if (h / 100 > 23 || h % 100 > 59) return std::nullopt;
        l.push_back({static_cast<Category>(cat), h / 100 * 60 + h % 100, static_cast<uint8_t>(masque)});
        if (l.size() > kMaxRappels) return std::nullopt;
        if (fin == std::string::npos) break;
        i = fin + 1;
    }
    return l;
}

} // namespace pup
