#include "logic/reminders.h"

#include <algorithm>
#include <climits>
#include <cstdint>
#include <sstream>
#include <utility>

namespace pup {

namespace {
constexpr int64_t kNever = INT64_MIN;
} // namespace

ReminderScheduler::ReminderScheduler(std::vector<Reminder> list)
{
    set(std::move(list));
}

void ReminderScheduler::set(std::vector<Reminder> list)
{
    list_ = std::move(list);
    last_day_.assign(list_.size(), kNever);
}

void ReminderScheduler::set_settings(const Settings &s)
{
    cfg_ = s;
}

std::optional<Reminder> ReminderScheduler::poll(const LocalTime &t)
{
    if (in_range(t.minute, cfg_.calme_debut, cfg_.calme_fin)) return std::nullopt;
    int best = -1;
    for (size_t i = 0; i < list_.size(); ++i) {
        const Reminder &r = list_[i];
        if (t.minute < r.minute || t.minute >= r.minute + cfg_.rattrapage_min) continue;
        if (!(r.days & (1 << t.weekday))) continue;
        if (last_day_[i] == t.day) continue;
        if (in_range(r.minute, cfg_.calme_debut, cfg_.calme_fin)) continue;
        if (best < 0 || r.minute < list_[best].minute) best = static_cast<int>(i);
    }
    if (best < 0) return std::nullopt;
    last_day_[best] = t.day;
    return list_[best];
}

std::string ReminderScheduler::save() const
{
    std::ostringstream o;
    o << list_.size();
    for (const int64_t d : last_day_) {
        o << ' ';
        if (d == kNever) o << 'x';
        else o << d;
    }
    return o.str();
}

bool ReminderScheduler::load(const std::string &s)
{
    std::istringstream in(s);
    size_t n;
    if (!(in >> n) || n != list_.size()) return false;
    std::vector<int64_t> days(n, kNever);
    for (int64_t &d : days) {
        std::string tok;
        if (!(in >> tok)) return false;
        if (tok == "x") continue;
        std::istringstream t(tok);
        char extra;
        if (!(t >> d) || (t >> extra)) return false;
    }
    last_day_ = days;
    return true;
}

void Escalation::start(int64_t mono_s)
{
    active_ = true;
    start_ = mono_s;
    next_ = 0;
}

void Escalation::snooze(int64_t mono_s)
{
    start(mono_s + static_cast<int64_t>(cfg_.report_min) * 60);
}

void Escalation::set_settings(const Settings &s)
{
    cfg_ = s;
}

void Escalation::stop()
{
    active_ = false;
}

bool Escalation::active() const
{
    return active_;
}

Cue Escalation::tick(int64_t mono_s)
{
    if (!active_) return Cue::None;
    const int64_t elapsed = mono_s - start_;
    Cue out = Cue::None;
    // Etape 0 : Wouf ; puis une relance Aboie par relances_s. Plafond : ensuite
    // le chiot reste en reclamation silencieuse.
    const int count = 1 + std::min(std::max(cfg_.nb_relances, 0), 3);
    while (next_ < count) {
        const int64_t offset = next_ == 0 ? 0 : cfg_.relances_s[next_ - 1];
        if (offset > elapsed) break;
        out = next_ == 0 ? Cue::Wouf : Cue::Aboie;
        ++next_;
    }
    return out;
}

} // namespace pup
