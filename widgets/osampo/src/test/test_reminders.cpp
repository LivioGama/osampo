// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>

#include "logic/reminders.h"

namespace {

using pup::Category;
using pup::Cue;
using pup::LocalTime;

// day 0 = jeudi (weekday 3) ; day 5 = mardi (weekday 1).
LocalTime at(int64_t day, int h, int m)
{
    return {day, h * 60 + m, static_cast<int>((day + 3) % 7)};
}

void rappel_une_fois_par_jour()
{
    pup::ReminderScheduler s(pup::default_reminders());
    assert(!s.poll(at(0, 11, 59)));
    auto r = s.poll(at(0, 12, 0));
    assert(r && r->cat == Category::Repas && r->minute == 12 * 60);
    assert(!s.poll(at(0, 12, 0)));
    r = s.poll(at(1, 12, 0));
    assert(r && r->cat == Category::Repas);
}

void sport_pas_le_mardi()
{
    pup::ReminderScheduler s(pup::default_reminders());
    assert(!s.poll(at(5, 18, 0))); // mardi
    assert(s.poll(at(4, 18, 0)));  // lundi
}

void rappel_en_heure_calme_jamais_sonne()
{
    pup::ReminderScheduler s({{Category::Sortie, 23 * 60, 0x7F}});
    for (int m = 22 * 60; m < 24 * 60; ++m) assert(!s.poll({0, m, 3}));
    for (int m = 0; m <= 8 * 60; ++m) assert(!s.poll({1, m, 4}));
}

void rappel_rate_rattrape_dans_l_heure()
{
    pup::ReminderScheduler s(pup::default_reminders());
    auto r = s.poll(at(0, 12, 20));
    assert(r && r->cat == Category::Repas);
    pup::ReminderScheduler s3({{Category::Repas, 12 * 60, 0x7F}});
    assert(s3.poll(at(0, 12, 59)));
    pup::ReminderScheduler s4({{Category::Repas, 12 * 60, 0x7F}});
    assert(!s4.poll(at(0, 13, 0)));
}

void horloge_qui_recule_ne_rejoue_rien()
{
    pup::ReminderScheduler s(pup::default_reminders());
    assert(s.poll(at(0, 12, 5)));
    assert(!s.poll(at(0, 11, 58)));
    assert(!s.poll(at(0, 12, 1)));
}

void plusieurs_dus_un_par_appel()
{
    pup::ReminderScheduler s({{Category::Sortie, 12 * 60 + 30, 0x7F},
                              {Category::Repas, 12 * 60, 0x7F}});
    auto a = s.poll(at(0, 12, 40));
    assert(a && a->cat == Category::Repas);
    auto b = s.poll(at(0, 12, 40));
    assert(b && b->cat == Category::Sortie);
    assert(!s.poll(at(0, 12, 40)));
}

void set_remet_a_zero()
{
    pup::ReminderScheduler s(pup::default_reminders());
    assert(s.poll(at(0, 12, 0)));
    s.set(pup::default_reminders());
    assert(s.poll(at(0, 12, 0)));
}

void escalade()
{
    pup::Escalation e;
    assert(!e.active());
    assert(e.tick(0) == Cue::None);
    e.start(0);
    assert(e.active());
    assert(e.tick(0) == Cue::Wouf);
    assert(e.tick(0) == Cue::None);
    assert(e.tick(59) == Cue::None);
    assert(e.tick(60) == Cue::Aboie);
    assert(e.tick(180) == Cue::Aboie);
    assert(e.tick(420) == Cue::Aboie);
    assert(e.tick(10000) == Cue::None);
    assert(e.active());
    e.stop();
    assert(!e.active());
    assert(e.tick(20000) == Cue::None);
}

void escalade_tick_tardif_ne_rejoue_pas()
{
    pup::Escalation e;
    e.start(0);
    assert(e.tick(200) == Cue::Aboie); // Wouf et Aboie(60) rates, +180 rendu
    assert(e.tick(201) == Cue::None);
    assert(e.tick(420) == Cue::Aboie);
}

void escalade_snooze()
{
    pup::Escalation e;
    e.start(0);
    assert(e.tick(0) == Cue::Wouf);
    e.snooze(100);
    assert(e.active());
    assert(e.tick(699) == Cue::None);
    assert(e.tick(700) == Cue::Wouf);
    assert(e.tick(760) == Cue::Aboie);
}

void relances_reglees()
{
    pup::Settings s;
    s.relances_s[0] = 30;
    s.nb_relances = 1;
    pup::Escalation e;
    e.set_settings(s);
    e.start(0);
    assert(e.tick(0) == Cue::Wouf);
    assert(e.tick(29) == Cue::None);
    assert(e.tick(30) == Cue::Aboie);
    assert(e.tick(10000) == Cue::None);
}

void report_regle_escalade()
{
    pup::Settings s;
    s.report_min = 2;
    pup::Escalation e;
    e.set_settings(s);
    e.start(0);
    assert(e.tick(0) == Cue::Wouf);
    e.snooze(100);
    assert(e.tick(219) == Cue::None);
    assert(e.tick(220) == Cue::Wouf);
}

void rattrapage_regle()
{
    pup::Settings s;
    s.rattrapage_min = 10;
    pup::ReminderScheduler a({{Category::Repas, 12 * 60, 0x7F}});
    a.set_settings(s);
    assert(!a.poll(at(0, 12, 10)));
    pup::ReminderScheduler b({{Category::Repas, 12 * 60, 0x7F}});
    b.set_settings(s);
    assert(b.poll(at(0, 12, 9)));
}

void heures_calmes_reglees_rappel()
{
    pup::Settings s;
    s.calme_debut = 12 * 60;
    s.calme_fin = 13 * 60;
    pup::ReminderScheduler a({{Category::Repas, 12 * 60 + 30, 0x7F}});
    a.set_settings(s);
    assert(!a.poll(at(0, 12, 30)));
    pup::ReminderScheduler b({{Category::Repas, 23 * 60, 0x7F}});
    b.set_settings(s);
    assert(b.poll(at(0, 23, 0))); // 23:00 n est plus calme
}

} // namespace

int main()
{
    rappel_une_fois_par_jour();
    sport_pas_le_mardi();
    rappel_en_heure_calme_jamais_sonne();
    rappel_rate_rattrape_dans_l_heure();
    horloge_qui_recule_ne_rejoue_rien();
    plusieurs_dus_un_par_appel();
    set_remet_a_zero();
    escalade();
    escalade_tick_tardif_ne_rejoue_pas();
    escalade_snooze();
    relances_reglees();
    report_regle_escalade();
    rattrapage_regle();
    heures_calmes_reglees_rappel();
    return 0;
}
