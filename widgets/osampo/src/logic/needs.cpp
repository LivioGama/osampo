#include "logic/needs.h"

#include <algorithm>
#include <sstream>

namespace pup {

namespace {
constexpr int kMax = 100;
constexpr int kLow = 30;
constexpr int kHigh = 70;
constexpr const char *kTag = "needs1";

int idx(Need n) { return static_cast<int>(n); }
} // namespace

int Needs::level(Need n) const { return level_[idx(n)]; }

int Needs::icon(Need n) const
{
    int l = level(n);
    if (l < kLow) return 0;
    if (l >= kHigh) return 2;
    return 1;
}

void Needs::satisfy(Need n, int64_t day)
{
    on_day(day); // garantit satisfied_ == last_day_ quand on marque
    level_[idx(n)] = std::min(kMax, level_[idx(n)] + cfg_.gain);
    // Evenement tardif (jour deja passe) : +20 mais aucune exemption.
    if (day >= last_day_) satisfied_[idx(n)] = day;
}

void Needs::set_settings(const Settings &s)
{
    cfg_ = s;
}

void Needs::on_day(int64_t day)
{
    if (!has_last_) {
        has_last_ = true;
        last_day_ = day;
        return;
    }
    if (day <= last_day_) return;
    // Seul le dernier jour peut avoir une satisfaction ; les jours suivants
    // sont tous sans satisfaction. Au-dela de 10 jours supplementaires tout
    // est a 0 : on borne le calcul. Soustraction non signee : pas de
    // debordement (day > last_day_).
    uint64_t steps = static_cast<uint64_t>(day) - static_cast<uint64_t>(last_day_);
    int extra = static_cast<int>(std::min<uint64_t>(steps - 1, 10));
    for (int i = 0; i < 3; i++) {
        int loss = extra * cfg_.baisse_jour;
        if (satisfied_[i] != last_day_) loss += cfg_.baisse_jour;
        level_[i] = std::max(0, level_[i] - loss);
    }
    last_day_ = day;
}

bool Needs::sulking() const
{
    bool low = false;
    bool fed = false;
    for (int i = 0; i < 3; i++) {
        if (level_[i] < kLow) low = true;
        if (has_last_ && satisfied_[i] == last_day_) fed = true;
    }
    return low && !fed;
}

std::string Needs::save() const
{
    std::ostringstream o;
    o << kTag << ' ' << level_[0] << ' ' << level_[1] << ' ' << level_[2] << ' '
      << (has_last_ ? 1 : 0) << ' ' << last_day_;
    for (int i = 0; i < 3; i++) {
        o << ' ';
        if (satisfied_[i] == kNone) o << 'x';
        else o << satisfied_[i];
    }
    return o.str();
}

bool Needs::load(const std::string &s)
{
    std::istringstream in(s);
    std::string tag;
    if (!(in >> tag) || tag != kTag) return false;
    int lv[3];
    for (int &l : lv)
        if (!(in >> l) || l < 0 || l > kMax) return false;
    int has;
    int64_t last;
    if (!(in >> has >> last) || (has != 0 && has != 1)) return false;
    int64_t sat[3];
    for (int64_t &v : sat) {
        std::string tok;
        if (!(in >> tok)) return false;
        if (tok == "x") {
            v = kNone;
        } else {
            std::istringstream t(tok);
            char c;
            if (!(t >> v) || (t >> c)) return false;
        }
    }
    std::string extra;
    if (in >> extra) return false;
    std::copy(lv, lv + 3, level_);
    std::copy(sat, sat + 3, satisfied_);
    has_last_ = has == 1;
    last_day_ = last;
    return true;
}

} // namespace pup
