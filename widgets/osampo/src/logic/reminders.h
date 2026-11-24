#pragma once
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "logic/clock.h"
#include "logic/rappel.h"
#include "logic/settings.h"

namespace pup {

enum class Cue { None, Wouf, Aboie, Gemit, Jappe };

class ReminderScheduler {
public:
    explicit ReminderScheduler(std::vector<Reminder> list);
    // Remplace la liste et oublie les rappels deja sonnes.
    void set(std::vector<Reminder> list);
    // Un rappel est du de sa minute a +59 min (rattrapage), jamais en heures
    // calmes, au plus une fois par jour. Un seul rappel par appel (le plus tot).
    std::optional<Reminder> poll(const LocalTime &t);
    // Rattrapage et heures calmes ; ne touche pas aux rappels deja sonnes.
    void set_settings(const Settings &s);
    // Jours des derniers rappels sonnes, une ligne de texte (un jeton par rappel).
    std::string save() const;
    // false et etat inchange si invalide ou si le nombre de rappels differe.
    bool load(const std::string &s);

private:
    Settings cfg_;
    std::vector<Reminder> list_;
    std::vector<int64_t> last_day_;
};

class Escalation {
public:
    void start(int64_t mono_s);
    void snooze(int64_t mono_s); // silence report_min puis reprise au debut
    // Relances et report ; une escalade en cours garde son depart.
    void set_settings(const Settings &s);
    void stop();
    bool active() const;
    Cue tick(int64_t mono_s);    // Cue a jouer maintenant, sinon None

private:
    Settings cfg_;
    bool active_ = false;
    int64_t start_ = 0;
    int next_ = 0;
};

} // namespace pup
