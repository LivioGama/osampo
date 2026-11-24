// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>
#include <optional>
#include <string>

#include "logic/pup.h"
#include "logic/settings.h"

namespace {

using pup::Category;
using pup::Cue;
using pup::Input;
using pup::Need;
using pup::Pup;
using pup::Reason;
using pup::Reminder;
using pup::State;

const Reminder kSortie{Category::Sortie, 12 * 60 + 30, 0x7F};
const Reminder kSport{Category::Sport, 18 * 60, 0x15};
const Reminder kRepas{Category::Repas, 12 * 60, 0x7F};
const Reminder kDodo{Category::Dodo, 21 * 60 + 45, 0x7F};

// Jauge du daemon : vide, pleine, ou valeur donnee (donnees fraiches).
void vide(Pup &p, int64_t t) { p.jauge(0, true, 0, true, t); }
void niveau(Pup &p, int sage, int64_t t, bool actif = false, int absence = 0)
{
    p.jauge(sage, actif, absence, true, t);
}

// Aucun cue entre from et to inclus, en tickant chaque seconde.
bool silent(Pup &p, int64_t from, int64_t to)
{
    for (int64_t t = from; t <= to; ++t)
        if (p.tick(t) != Cue::None) return false;
    return true;
}

void report_regle()
{
    pup::Settings s;
    s.report_min = 2;
    Pup p;
    p.set_settings(s);
    p.reminder_due(kRepas, 0);
    assert(p.tick(0) == Cue::Wouf);
    p.input(Input::LongPress, 30);
    assert(p.state() == State::Repos);
    assert(silent(p, 30, 149));
    assert(p.state() == State::Repos);
    assert(p.tick(150) == Cue::Wouf);
    assert(p.state() == State::Reclame);
}

void calme_regle()
{
    // apres validation d un rappel repas (R7), la jauge vide attend le calme
    pup::Settings s;
    s.calme_validation_min = 2;
    Pup q;
    q.set_settings(s);
    q.reminder_due(kRepas, 0);
    q.input(Input::Tap, 10);
    for (int64_t t = 11; t < 40; ++t) q.tick(t); // fin de Content
    q.input(Input::AnimDone, 40);
    vide(q, 129);
    assert(q.state() == State::Repos);
    vide(q, 130);
    assert(q.state() == State::Reclame);
}

void set_settings_ne_touche_pas_l_etat_en_cours()
{
    pup::Settings s;
    s.report_min = 1;
    Pup p;
    p.reminder_due(kRepas, 0);
    p.tick(0);
    p.input(Input::LongPress, 30); // reprise a 630 avec les defauts
    p.set_settings(s);
    assert(silent(p, 30, 629));
    assert(p.tick(630) == Cue::Wouf);
}

// --- Regles du brief ---

void dort_prime_sur_reclame()
{
    Pup p;
    p.reminder_due(kSortie, 0);
    assert(p.state() == State::Reclame);
    p.set_quiet(true);
    assert(p.state() == State::Dort);
    assert(p.reason() == Reason::None);
    p.reminder_due(kRepas, 10);
    assert(p.state() == State::Dort);
    assert(silent(p, 10, 1000));

    Pup q;
    q.reminder_due(kSortie, 0);
    q.input(Input::FaceDown, 5);
    assert(q.state() == State::Dort);
    assert(silent(q, 5, 1000));
}

void rappel_tap_content_et_besoin()
{
    Pup p;
    p.reminder_due(kRepas, 0);
    assert(p.state() == State::Reclame);
    assert(p.reason() == Reason::Rappel);
    assert(p.current_reminder().has_value());
    assert(p.current_reminder()->cat == Category::Repas);
    assert(p.tick(0) == Cue::Wouf);
    assert(p.tick(59) == Cue::None);
    assert(p.tick(60) == Cue::Aboie);
    std::optional<Need> n = p.input(Input::Tap, 70);
    assert(!n.has_value()); // repas : aucun besoin
    assert(p.state() == State::Content);
    assert(p.reason() == Reason::None);
    assert(!p.current_reminder().has_value());
}

void rappel_appui_long_reporte_600_s()
{
    Pup p;
    p.reminder_due(kRepas, 0);
    assert(p.tick(0) == Cue::Wouf);
    assert(!p.input(Input::LongPress, 30).has_value());
    assert(p.state() == State::Repos);
    assert(p.current_reminder().has_value());
    assert(silent(p, 30, 629));
    assert(p.state() == State::Repos);
    assert(p.tick(630) == Cue::Wouf);
    assert(p.state() == State::Reclame);
    assert(p.reason() == Reason::Rappel);
    assert(p.tick(690) == Cue::Aboie);
}

void inactivite_wouf_puis_escalade()
{
    Pup p;
    vide(p, 0);
    assert(p.state() == State::Reclame);
    assert(p.reason() == Reason::Promenade);
    assert(p.tick(0) == Cue::Wouf);
    assert(p.tick(59) == Cue::None);
    assert(p.tick(60) == Cue::Aboie);
    assert(p.tick(180) == Cue::Aboie);
    assert(p.tick(420) == Cue::Aboie);
    assert(silent(p, 421, 2000));
}



void claude_un_seul_gemit()
{
    Pup p;
    p.claude_wait(true, 0);
    assert(p.state() == State::Reclame);
    assert(p.reason() == Reason::Claude);
    assert(p.tick(0) == Cue::Gemit);
    for (int64_t t = 1; t < 2000; ++t) {
        p.claude_wait(true, t);
        // un seul Gemit ; l'aboiement unique de l'escalade tombe au delai (SPEC-012)
        assert(p.tick(t) == (t == 120 ? Cue::Aboie : Cue::None));
    }
    assert(p.state() == State::Reclame);
}

void claude_sort_sur_false_ou_tap()
{
    Pup p;
    p.claude_wait(true, 0);
    p.claude_wait(false, 10);
    assert(p.state() == State::Repos);
    assert(p.reason() == Reason::None);

    Pup q;
    q.claude_wait(true, 0);
    assert(q.tick(0) == Cue::Gemit);
    assert(!q.input(Input::Tap, 5).has_value());
    assert(q.state() == State::Repos);
    q.claude_wait(true, 6);
    assert(q.state() == State::Repos);
    assert(q.tick(6) == Cue::None);
}

void caresse_content_jappe_jouer()
{
    Pup p;
    assert(p.input(Input::Stroke, 0) == Need::Jouer);
    assert(p.state() == State::Content);
    assert(p.tick(0) == Cue::Jappe);
}

void lancer_rapporte_puis_repos()
{
    Pup p;
    assert(p.input(Input::Throw, 0) == Need::Jouer);
    assert(p.state() == State::Rapporte);
    p.input(Input::AnimDone, 3);
    assert(p.state() == State::Repos);
}

// Bain (SP5e) : l'icone brosse lance le bain, soin = Jouer ; un jappement
// content ; la fin de l'animation ramene au repos.
void brosse_bain_puis_repos()
{
    Pup p;
    assert(p.input(Input::Brosse, 0) == Need::Jouer);
    assert(p.state() == State::Bain);
    assert(p.tick(0) == Cue::Jappe);
    p.input(Input::AnimDone, 12);
    assert(p.state() == State::Repos);
}

// Pas de bain quand il dort ni quand il reclame.
void brosse_ignoree_en_dort_et_reclame()
{
    Pup d;
    d.set_quiet(true);
    assert(!d.input(Input::Brosse, 0).has_value());
    assert(d.state() == State::Dort);
    Pup r;
    r.reminder_due(kSortie, 0);
    assert(!r.input(Input::Brosse, 1).has_value());
    assert(r.state() == State::Reclame);
}

// Os (SP5f) : l'icone os lui donne un os a croquer ; une friandise, aucun
// besoin rendu ; la fin de l'animation ramene au repos.
void os_mange_puis_repos()
{
    Pup p;
    assert(!p.input(Input::Os, 0).has_value());
    assert(p.state() == State::Mange);
    assert(p.tick(0) == Cue::Jappe);
    p.input(Input::AnimDone, 9);
    assert(p.state() == State::Repos);
}

void os_ignore_en_dort_et_reclame()
{
    Pup d;
    d.set_quiet(true);
    d.input(Input::Os, 0);
    assert(d.state() == State::Dort);
    Pup r;
    r.reminder_due(kSortie, 0);
    r.input(Input::Os, 1);
    assert(r.state() == State::Reclame);
}

void secouer_tournis_puis_repos()
{
    Pup p;
    assert(!p.input(Input::Shake, 0).has_value());
    assert(p.state() == State::Tournis);
    p.input(Input::AnimDone, 2);
    assert(p.state() == State::Repos);
}

void content_puis_repos()
{
    Pup p;
    p.input(Input::Stroke, 0);
    assert(p.state() == State::Content);
    p.input(Input::AnimDone, 2);
    assert(p.state() == State::Repos);
}

void boude_remplace_repos_sans_bloquer()
{
    Pup p;
    p.set_sulking(true);
    assert(p.state() == State::Boude);
    assert(p.input(Input::Throw, 0) == Need::Jouer);
    assert(p.state() == State::Rapporte);
    p.input(Input::AnimDone, 1);
    assert(p.state() == State::Boude);
    p.reminder_due(kSortie, 2);
    assert(p.state() == State::Reclame);
}

// --- Cas nommes du brief ---

void calme_coupe_tout_son()
{
    // 22:30 en heures calmes : t = 0 represente 22:30.
    Pup p;
    p.set_quiet(true);
    p.reminder_due(kDodo, 0);
    assert(p.state() == State::Dort);
    for (int64_t t = 0; t <= 3600; ++t) {
        vide(p, t);
        p.claude_wait(true, t);
        assert(p.tick(t) == Cue::None);
        assert(p.state() == State::Dort);
    }
}

void retourne_abandonne_le_rappel()
{
    Pup p;
    p.input(Input::FaceDown, 0);
    p.reminder_due(kSortie, 10);
    assert(p.state() == State::Dort);
    p.input(Input::FaceUp, 20);
    assert(p.state() == State::Repos);
    assert(!p.current_reminder().has_value());
    assert(silent(p, 20, 2000));
}


void boude_n_est_pas_terminal()
{
    Pup p;
    p.set_sulking(true);
    assert(p.state() == State::Boude);
    assert(p.input(Input::Stroke, 0) == Need::Jouer);
    assert(p.state() == State::Content);
    p.input(Input::AnimDone, 1);
    assert(p.state() == State::Boude);
    p.set_sulking(false);
    assert(p.state() == State::Repos);
}

// --- Arbitrages R-a a R-j ---

void ra_calme_abandonne_l_escalade_en_cours()
{
    // Escalade lancee a 21:59 (t = 0), calme a 22:00 (t = 60) : rien a 22:01.
    Pup p;
    p.reminder_due(kSortie, 0);
    assert(p.tick(0) == Cue::Wouf);
    p.set_quiet(true);
    assert(p.state() == State::Dort);
    assert(!p.current_reminder().has_value());
    assert(silent(p, 60, 1000));
    // Seul FaceUp agit en Dort.
    assert(!p.input(Input::Stroke, 1001).has_value());
    assert(!p.input(Input::Throw, 1002).has_value());
    assert(!p.input(Input::Tap, 1003).has_value());
    p.input(Input::Shake, 1004);
    assert(p.state() == State::Dort);
    // Fin du calme : repos, le rappel ne revient pas.
    p.set_quiet(false);
    assert(p.state() == State::Repos);
    assert(silent(p, 1005, 3000));

    // Rappel reporte puis calme : abandonne aussi.
    Pup q;
    q.reminder_due(kRepas, 0);
    q.input(Input::LongPress, 10);
    q.set_quiet(true);
    q.set_quiet(false);
    assert(!q.current_reminder().has_value());
    assert(silent(q, 11, 2000));
    assert(q.state() == State::Repos);

    // Calme et face contre table : il faut les deux sorties.
    Pup r;
    r.set_sulking(true);
    r.set_quiet(true);
    r.input(Input::FaceDown, 0);
    r.set_quiet(false);
    assert(r.state() == State::Dort);
    r.input(Input::FaceUp, 1);
    assert(r.state() == State::Boude);

    Pup s;
    s.set_quiet(true);
    s.input(Input::FaceDown, 0);
    s.input(Input::FaceUp, 1);
    assert(s.state() == State::Dort);
}

void rb_priorite_entre_raisons()
{
    // Rappel remplace Promenade.
    Pup p;
    vide(p, 0);
    p.reminder_due(kSortie, 10);
    assert(p.reason() == Reason::Rappel);
    // Promenade et Claude ignores sous Rappel.
    vide(p, 11);
    p.claude_wait(true, 12);
    assert(p.reason() == Reason::Rappel);

    // Promenade remplace Claude, Claude ignore sous Promenade.
    Pup q;
    q.claude_wait(true, 0);
    vide(q, 1);
    assert(q.reason() == Reason::Promenade);
    q.claude_wait(true, 2);
    assert(q.reason() == Reason::Promenade);

    // Reclame interrompt les animations.
    Pup r;
    r.input(Input::Throw, 0);
    r.claude_wait(true, 1);
    assert(r.state() == State::Reclame);
    Pup s;
    s.input(Input::Shake, 0);
    vide(s, 1);
    assert(s.state() == State::Reclame);
    Pup t;
    t.input(Input::Stroke, 0);
    t.reminder_due(kRepas, 1);
    assert(t.state() == State::Reclame);
}

void rc_signaux_de_niveau_idempotents()
{
    Pup p;
    for (int64_t t = 0; t <= 60; ++t) vide(p, t);
    assert(p.reason() == Reason::Promenade);
    // L'escalade n'a pas ete relancee : Wouf a 0 (rattrape), Aboie a 60.
    assert(p.tick(60) == Cue::Aboie);


    Pup r;
    r.claude_wait(true, 0);
    for (int64_t t = 0; t < 100; ++t) r.claude_wait(true, t);
    assert(r.tick(100) == Cue::Gemit);
    assert(r.tick(101) == Cue::None);
}


void re_claude_nouvelle_attente_nouveau_gemit()
{
    Pup p;
    p.claude_wait(true, 0);
    assert(p.tick(0) == Cue::Gemit);
    p.claude_wait(false, 10);
    p.claude_wait(true, 20);
    assert(p.state() == State::Reclame);
    assert(p.tick(20) == Cue::Gemit);

    // Apres un Tap, nouvelle attente seulement apres false.
    p.input(Input::Tap, 30);
    p.claude_wait(true, 31);
    assert(p.state() == State::Repos);
    p.claude_wait(false, 32);
    p.claude_wait(true, 33);
    assert(p.tick(33) == Cue::Gemit);

    // Remplacee par une raison superieure : revient sans second Gemit.
    Pup q;
    q.claude_wait(true, 0);
    assert(q.tick(0) == Cue::Gemit);
    vide(q, 1);
    niveau(q, 100, 2); // absence : content (Jappe rendu au tick)
    assert(q.tick(2) == Cue::Jappe);
    q.input(Input::AnimDone, 2);
    q.claude_wait(true, 3);
    assert(q.reason() == Reason::Claude);
    assert(silent(q, 3, 122));
    assert(q.tick(123) == Cue::Aboie); // delai recompte depuis la reprise (SPEC-012)
    assert(silent(q, 124, 500));
}

void rf_cues_une_seule_fois()
{
    Pup p;
    p.reminder_due(kRepas, 0);
    assert(p.tick(0) == Cue::Wouf);
    p.input(Input::Tap, 5);
    assert(p.tick(5) == Cue::Jappe);
    assert(silent(p, 6, 1000));

    Pup q;
    q.input(Input::Stroke, 0);
    assert(q.tick(0) == Cue::Jappe);
    assert(silent(q, 1, 100));
}

void rg_reclame_seuls_tap_et_appui_long()
{
    Pup p;
    p.reminder_due(kSortie, 0);
    assert(!p.input(Input::Stroke, 1).has_value());
    assert(!p.input(Input::Throw, 2).has_value());
    assert(!p.input(Input::Shake, 3).has_value());
    assert(!p.input(Input::AnimDone, 4).has_value());
    assert(p.state() == State::Reclame);
    assert(p.reason() == Reason::Rappel);

    // LongPress sans effet pour Claude (la jauge l'arrete : SPEC-001, voir jauge_appui_long_...).
    Pup r;
    r.claude_wait(true, 0);
    r.input(Input::LongPress, 1);
    assert(r.reason() == Reason::Claude);

    // LongPress hors Reclame : rien.
    Pup s;
    assert(!s.input(Input::LongPress, 0).has_value());
    assert(s.state() == State::Repos);
}

void rh_tap_au_repos_sans_effet()
{
    Pup p;
    assert(!p.input(Input::Tap, 0).has_value());
    assert(p.state() == State::Repos);
    assert(p.tick(0) == Cue::None);
    p.set_sulking(true);
    assert(!p.input(Input::Tap, 1).has_value());
    assert(p.state() == State::Boude);
}

void ri_besoins_rendus()
{
    // repas : un toucher ; sortie, sport, dodo : la promenade (laisse) reussie.
    Pup a;
    a.reminder_due(kRepas, 0);
    assert(!a.input(Input::Tap, 1).has_value());
    assert(a.state() == State::Content);
    struct Case {
        Reminder r;
        Need need;
    };
    const Case cases[] = {{kSortie, Need::Bouger}, {kSport, Need::Bouger}, {kDodo, Need::Dormir}};
    for (const Case &c : cases) {
        Pup p;
        p.reminder_due(c.r, 0);
        assert(!p.input(Input::Tap, 1).has_value());
        assert(p.state() == State::Reclame);
        assert(!p.input(Input::Laisse, 2).has_value());
        assert(p.state() == State::Promenade);
        niveau(p, 80, 400, false, 6);
        assert(p.input(Input::Laisse, 401) == c.need);
    }
    Pup p;
    assert(p.input(Input::Stroke, 0) == Need::Jouer);
    p.input(Input::AnimDone, 1);
    assert(p.input(Input::Throw, 2) == Need::Jouer);
}

void rj_boude_etat_de_repos()
{
    // Toutes les sorties vers le repos menent a Boude.
    Pup p;
    p.set_sulking(true);
    p.reminder_due(kRepas, 0);
    p.input(Input::LongPress, 1);
    assert(p.state() == State::Boude);
    vide(p, 2);
    niveau(p, 100, 3); // absence sans laisse : content, puis repos
    p.input(Input::AnimDone, 3);
    assert(p.state() == State::Boude);
    p.claude_wait(true, 4);
    p.claude_wait(false, 5);
    assert(p.state() == State::Boude);
    p.input(Input::Shake, 6);
    p.input(Input::AnimDone, 7);
    assert(p.state() == State::Boude);
    p.set_quiet(true);
    p.set_quiet(false);
    assert(p.state() == State::Boude);
    p.set_sulking(false);
    assert(p.state() == State::Repos);

    // set_sulking ne touche pas un etat autre que le repos.
    Pup q;
    q.input(Input::Throw, 0);
    q.set_sulking(true);
    assert(q.state() == State::Rapporte);
    q.input(Input::AnimDone, 1);
    assert(q.state() == State::Boude);
}

// R-p : valider un Rappel ou le reporter ouvre aussi la fenetre calme de 600 s.
void rp_rappel_ouvre_la_fenetre_calme()
{
    Pup p;
    vide(p, 0);
    p.reminder_due(kRepas, 10);
    p.input(Input::Tap, 20); // valide le rappel
    assert(p.state() == State::Content);
    p.input(Input::AnimDone, 21);
    vide(p, 30);
    assert(p.state() == State::Repos); // fenetre calme jusqu'a 620
    vide(p, 619);
    assert(p.state() == State::Repos);
    vide(p, 620);
    assert(p.state() == State::Reclame && p.reason() == Reason::Promenade);

    Pup q;
    vide(q, 0);
    q.reminder_due(kRepas, 10);
    q.input(Input::LongPress, 20); // report 600 s
    assert(q.state() == State::Repos);
    vide(q, 30);
    assert(q.state() == State::Repos);
    vide(q, 619);
    assert(q.state() == State::Repos);
    vide(q, 620); // le rappel revient d'abord (rang superieur)
    assert(q.state() == State::Reclame && q.reason() == Reason::Rappel);
}

// R-q : un Gemit en attente est abandonne quand Claude n'est plus la raison.
void rq_gemit_en_attente_abandonne()
{
    Pup p;
    p.claude_wait(true, 0);
    p.claude_wait(false, 0); // retour au repos avant le tick
    assert(silent(p, 0, 5));

    Pup q;
    q.claude_wait(true, 0);
    q.reminder_due(kSortie, 0); // raison superieure avant le tick
    assert(q.tick(0) == Cue::Wouf); // le Wouf du rappel, pas le Gemit
    assert(silent(q, 1, 50));

    Pup r;
    r.claude_wait(true, 0);
    r.input(Input::Tap, 0); // Tap sur Claude : repos
    assert(silent(r, 0, 5));
}

// R-q : un Gemit ne remplace jamais un Jappe en attente.
void rq_gemit_n_ecrase_pas_jappe()
{
    Pup p;
    p.input(Input::Stroke, 0); // Jappe en attente
    p.claude_wait(true, 0);
    assert(p.tick(0) == Cue::Jappe);
    assert(p.tick(1) == Cue::None);
}

// --- Promenade (SP5h) ---

void jauge_vide_reclame_la_laisse()
{
    Pup p;
    niveau(p, 40, 0, true);
    assert(p.state() == State::Repos);
    vide(p, 10);
    assert(p.state() == State::Reclame && p.reason() == Reason::Promenade);
    assert(p.tick(10) == Cue::Wouf);
    assert(p.laisse_visible());
}

void jauge_ignoree_si_perimee()
{
    Pup p;
    p.jauge(0, true, 0, false, 0);
    assert(p.state() == State::Repos);
    vide(p, 1);
    assert(p.state() == State::Reclame);
    p.jauge(0, true, 0, false, 2); // donnees perimees : il cesse de reclamer
    assert(p.state() == State::Repos);
}

void laisse_part_et_revient_content()
{
    Pup p;
    vide(p, 0);
    assert(!p.input(Input::Tap, 1).has_value()); // le toucher ne valide plus
    assert(p.state() == State::Reclame);
    assert(!p.input(Input::Laisse, 2).has_value());
    assert(p.state() == State::Promenade);
    assert(p.tick(2) == Cue::Jappe); // il part content
    assert(silent(p, 3, 1200));     // silence pendant la promenade
    niveau(p, 60, 1200, false, 20);
    assert(p.input(Input::Laisse, 1201) == Need::Bouger);
    assert(p.state() == State::Content);
    assert(p.tick(1201) == Cue::Jappe);
}

void promenade_a_la_demande_depuis_le_repos()
{
    Pup p;
    niveau(p, 80, 0, true);
    assert(p.laisse_visible());
    p.input(Input::Laisse, 1);
    assert(p.state() == State::Promenade);
    niveau(p, 100, 900, false, 15);
    assert(p.input(Input::Laisse, 901) == Need::Bouger);
    assert(p.state() == State::Content);
}

void promenade_trop_courte_redemande_apres_600s()
{
    Pup p;
    vide(p, 0);
    p.input(Input::Laisse, 1);
    niveau(p, 20, 200, false, 3);
    assert(!p.input(Input::Laisse, 201).has_value());
    assert(p.state() == State::Repos);
    niveau(p, 20, 799, true);
    assert(p.state() == State::Repos);
    niveau(p, 20, 801, true); // sous seuil_reclame (30) : il redemande
    assert(p.state() == State::Reclame && p.reason() == Reason::Promenade);
    // au-dessus du seuil, l hysteresis retombe : il attend de nouveau la jauge vide
    Pup q;
    vide(q, 0);
    q.input(Input::Laisse, 1);
    niveau(q, 20, 200, false, 3);
    q.input(Input::Laisse, 201);
    niveau(q, 35, 300, false, 6);
    niveau(q, 20, 900, true);
    assert(q.state() == State::Repos);
    vide(q, 901);
    assert(q.state() == State::Reclame);
}

void absence_sans_laisse_calme_la_reclamation()
{
    Pup p;
    vide(p, 0);
    niveau(p, 25, 100, false, 2);
    assert(p.state() == State::Reclame);
    niveau(p, 40, 400, false, 7);
    assert(p.state() == State::Content);
    assert(p.take_need() == Need::Bouger);
    assert(!p.take_need().has_value());
}

void retour_au_pc_termine_la_promenade()
{
    Pup p;
    vide(p, 0);
    p.input(Input::Laisse, 1);
    p.tick(1);
    niveau(p, 70, 1000, false, 16);
    niveau(p, 70, 1010, true);
    niveau(p, 70, 1129, true);
    assert(p.state() == State::Promenade);
    niveau(p, 70, 1130, true); // 2 min d activite continue
    assert(p.state() == State::Content);
    assert(p.tick(1130) == Cue::Wouf); // tu es au PC
    assert(p.take_need() == Need::Bouger);
}

void garde_fou_trois_heures()
{
    Pup p;
    p.input(Input::Laisse, 0);
    assert(p.state() == State::Promenade);
    for (int64_t t = 1; t < 3 * 3600; t += 60) p.tick(t);
    assert(p.state() == State::Promenade);
    p.tick(3 * 3600);
    assert(p.state() != State::Promenade);
}

void sortie_jugee_sur_l_absence()
{
    Pup p;
    niveau(p, 80, 0, true);
    p.reminder_due(kSortie, 1);
    niveau(p, 80, 2, true); // jauge haute : la reclamation reste
    assert(p.state() == State::Reclame && p.reason() == Reason::Rappel);
    p.input(Input::Laisse, 3);
    niveau(p, 100, 400, false, 6);
    assert(p.input(Input::Laisse, 401) == Need::Bouger);
    assert(p.state() == State::Content);
}

void sortie_redemandee_deux_fois_au_plus()
{
    Pup p;
    p.reminder_due(kSortie, 0);
    int64_t t = 0;
    for (int essai = 0; essai < 3; ++essai) {
        assert(p.state() == State::Reclame && p.reason() == Reason::Rappel);
        p.input(Input::Laisse, t + 1);
        niveau(p, 90, t + 60, false, 1);
        p.input(Input::Laisse, t + 61);
        assert(p.state() == State::Repos);
        t += 61;
        assert(silent(p, t, t + 599));
        t += 600;
        p.tick(t);
    }
    assert(p.state() == State::Repos); // troisieme echec : abandon
    assert(silent(p, t, t + 2000));
}

void fin_sans_donnees_fraiches_content()
{
    Pup p;
    vide(p, 0);
    p.input(Input::Laisse, 1);
    p.jauge(0, true, 0, false, 500); // hors de portee
    assert(p.state() == State::Promenade);
    assert(p.input(Input::Laisse, 900) == Need::Bouger);
    assert(p.state() == State::Content);
}

void dodo_puis_dort_jusqu_au_matin()
{
    Pup p;
    p.reminder_due(kDodo, 0);
    assert(p.dodo_reclame());
    p.input(Input::Laisse, 1);
    niveau(p, 90, 600, false, 10);
    assert(p.input(Input::Laisse, 601) == Need::Dormir);
    assert(p.state() == State::Dort && p.dodo_apres_promenade());
    p.input(Input::Tap, 700);
    p.input(Input::FaceUp, 701);
    vide(p, 702);
    p.reminder_due(kRepas, 703);
    assert(p.state() == State::Dort);
    assert(silent(p, 700, 2000));
    p.set_quiet(true);  // 22:00
    p.set_quiet(false); // 07:00
    assert(p.state() == State::Repos && !p.dodo_apres_promenade());
}

void dodo_survit_au_debut_des_heures_calmes_en_silence()
{
    Pup p;
    p.reminder_due(kDodo, 0);
    assert(p.tick(0) == Cue::Wouf);
    p.set_quiet(true);
    assert(p.state() == State::Reclame && p.dodo_reclame());
    assert(silent(p, 1, 600));
    p.input(Input::Laisse, 601);
    assert(p.state() == State::Promenade);
    niveau(p, 90, 1200, false, 10);
    p.input(Input::Laisse, 1201);
    assert(p.state() == State::Dort);
    assert(silent(p, 1201, 3000));
    // une promenade en cours survit aussi au debut des heures calmes
    Pup q;
    q.input(Input::Laisse, 0);
    q.set_quiet(true);
    assert(q.state() == State::Promenade);
    niveau(q, 100, 900, false, 15);
    q.input(Input::Laisse, 901);
    assert(q.state() == State::Dort);
}

void rappel_abandonne_pendant_la_promenade()
{
    Pup p;
    p.input(Input::Laisse, 0);
    p.reminder_due(kRepas, 10);
    assert(p.state() == State::Promenade);
    p.claude_wait(true, 11);
    assert(p.state() == State::Promenade);
    p.input(Input::FaceDown, 12); // board dans la poche
    assert(p.state() == State::Promenade);
    niveau(p, 100, 900, false, 15);
    p.input(Input::Laisse, 901);
    assert(!p.current_reminder().has_value());
}

void repas_garde_le_report()
{
    Pup p;
    p.reminder_due(kRepas, 0);
    p.input(Input::LongPress, 1);
    assert(p.state() == State::Repos && p.current_reminder().has_value());
}

// ADR-001 : sortie, sport et dodo se reportent comme un repas (report_min = 10 par defaut).
void sortie_sport_dodo_se_reportent()
{
    for (const Reminder &r : {kSortie, kSport, kDodo}) {
        Pup p;
        p.reminder_due(r, 0);
        assert(p.tick(0) == Cue::Wouf);
        assert(!p.input(Input::LongPress, 30).has_value());
        assert(p.state() == State::Repos && p.reason() == Reason::None);
        assert(p.current_reminder() == r); // reporte, pas abandonne
        assert(silent(p, 30, 629));
        assert(p.state() == State::Repos);
        assert(p.tick(630) == Cue::Wouf); // escalade reprise au debut
        assert(p.state() == State::Reclame && p.reason() == Reason::Rappel);
    }
}

void evenements_report_accepte_abandonne()
{
    // Report : evenement avec l'id du rappel, un seul.
    Pup p;
    p.reminder_due(kSortie, 0);
    p.input(Input::LongPress, 5);
    const auto e = p.take_events();
    assert(e.size() == 1 && e[0].kind == pup::EvtKind::Reporte && e[0].rappel == kSortie);
    assert(p.take_events().empty());

    // Laisse sur la reclamation : accepte + promenade.
    Pup q;
    q.reminder_due(kSport, 0);
    q.input(Input::Laisse, 5);
    const auto a = q.take_events();
    assert(a.size() == 1 && a[0].kind == pup::EvtKind::Accepte && a[0].rappel == kSport);
    assert(q.state() == State::Promenade);

    // Repas : aucun evenement, ni Tap ni report.
    Pup r;
    r.reminder_due(kRepas, 0);
    r.input(Input::LongPress, 5);
    r.set_quiet(true);
    assert(r.take_events().empty());
    Pup r2;
    r2.reminder_due(kRepas, 0);
    r2.input(Input::Tap, 5);
    assert(r2.take_events().empty());

    // Abandon a l'entree en Dort : reclame ou reporte.
    Pup s;
    s.reminder_due(kSortie, 0);
    s.input(Input::FaceDown, 5);
    const auto b = s.take_events();
    assert(b.size() == 1 && b[0].kind == pup::EvtKind::Abandonne && b[0].rappel == kSortie);
    s.input(Input::FaceDown, 6);
    assert(s.take_events().empty());
    Pup t;
    t.reminder_due(kSport, 0);
    t.input(Input::LongPress, 5);
    t.take_events();
    t.set_quiet(true);
    const auto c = t.take_events();
    assert(c.size() == 1 && c[0].kind == pup::EvtKind::Abandonne && c[0].rappel == kSport);
}

void evenements_caresse_et_os()
{
    Pup p;
    p.input(Input::Stroke, 0);
    auto e = p.take_events();
    assert(e.size() == 1 && e[0].kind == pup::EvtKind::Chatouille && !e[0].rappel);
    p.input(Input::AnimDone, 1);
    p.input(Input::Os, 2);
    e = p.take_events();
    assert(e.size() == 1 && e[0].kind == pup::EvtKind::Os);
    p.input(Input::Os, 3); // deja en train de manger : pris en compte de nouveau
    assert(p.take_events().size() == 1);

    // Ignores en Dort, en reclamation, en promenade : rien.
    Pup d;
    d.input(Input::FaceDown, 0);
    d.input(Input::Stroke, 1);
    d.input(Input::Os, 2);
    assert(d.take_events().empty());
    Pup r;
    r.reminder_due(kSortie, 0);
    r.input(Input::Stroke, 1);
    r.input(Input::Os, 2);
    assert(r.take_events().empty());
    Pup w;
    w.input(Input::Laisse, 0);
    w.take_events();
    w.input(Input::Stroke, 1);
    w.input(Input::Os, 2);
    assert(w.take_events().empty());
}

void jauge_appui_long_arrete_la_reclamation()
{
    Pup p;
    vide(p, 0);
    assert(p.state() == State::Reclame && p.reason() == Reason::Promenade);
    assert(p.tick(0) == Cue::Wouf);
    p.input(Input::LongPress, 100);
    assert(p.state() == State::Repos && p.reason() == Reason::None);
    const auto e = p.take_events();
    assert(e.size() == 1 && e[0].kind == pup::EvtKind::Reporte && !e[0].rappel);
    // Jauge toujours a 0 : pas de reclamation avant 100 + 600 s.
    for (int64_t t = 101; t < 700; t += 7) {
        vide(p, t);
        assert(p.state() == State::Repos);
        assert(p.tick(t) == Cue::None);
    }
    vide(p, 700);
    assert(p.state() == State::Reclame && p.reason() == Reason::Promenade);
    assert(p.tick(700) == Cue::Wouf);

    // Laisse sur la jauge : accepte sans rappel.
    p.take_events();
    p.input(Input::Laisse, 710);
    const auto a = p.take_events();
    assert(a.size() == 1 && a[0].kind == pup::EvtKind::Accepte && !a[0].rappel);
    assert(p.state() == State::Promenade);

    // Report_min regle.
    pup::Settings s;
    s.report_min = 1;
    Pup q;
    q.set_settings(s);
    vide(q, 0);
    q.input(Input::LongPress, 10);
    vide(q, 69);
    assert(q.state() == State::Repos);
    vide(q, 70);
    assert(q.state() == State::Reclame);
}

void accepter_un_rappel_reporte()
{
    Pup p;
    p.reminder_due(kSortie, 0);
    assert(!p.accepter_reporte(1)); // reclame, pas reporte
    p.input(Input::LongPress, 5);
    p.take_events();
    assert(p.rappel_reporte());
    assert(p.accepter_reporte(100));
    assert(p.state() == State::Promenade && !p.current_reminder().has_value());
    const auto e = p.take_events();
    assert(e.size() == 1 && e[0].kind == pup::EvtKind::Accepte && e[0].rappel == kSortie);
    assert(p.promenade_debut() == 100);
    // Plus de retour du rappel reporte.
    assert(!p.accepter_reporte(101));

    // Repas reporte : pas concerne.
    Pup r;
    r.reminder_due(kRepas, 0);
    r.input(Input::LongPress, 5);
    assert(!r.rappel_reporte() && !r.accepter_reporte(6));
}

void dodo_reporte_survit_au_calme_en_silence()
{
    Pup p;
    p.reminder_due(kDodo, 0);
    p.input(Input::LongPress, 5); // revient a 605
    p.take_events();
    p.set_quiet(true);
    assert(p.state() == State::Repos && p.current_reminder() == kDodo);
    assert(p.take_events().empty());
    assert(p.tick(300) == Cue::None);
    assert(p.tick(605) == Cue::None); // en silence
    assert(p.state() == State::Reclame && p.dodo_reclame());
    // Matin sans reponse : abandonne.
    p.set_quiet(false);
    assert(p.state() == State::Repos && !p.current_reminder().has_value());

    // Dodo reporte, matin avant le retour : abandonne aussi.
    Pup q;
    q.reminder_due(kDodo, 0);
    q.input(Input::LongPress, 5);
    q.set_quiet(true);
    q.set_quiet(false);
    assert(!q.current_reminder().has_value());
    assert(silent(q, 6, 2000));
}

void promenade_reussie_rend_bouger()
{
    Pup p;
    p.input(Input::Laisse, 0);
    niveau(p, 100, 900, false, 15);
    assert(p.input(Input::Laisse, 901) == Need::Bouger);
    assert(!p.take_need().has_value()); // rendu une seule fois
}

void promenade_survit_a_save_load()
{
    Pup p;
    p.reminder_due(kSortie, 0);
    p.input(Input::Laisse, 1);
    niveau(p, 50, 400, false, 6);
    const std::string s = p.save();
    Pup q;
    assert(q.load(s));
    assert(q.state() == State::Promenade);
    assert(q.input(Input::Laisse, 10) == Need::Bouger); // absence vue avant le redemarrage
    Pup r;
    assert(!r.load("x"));
    assert(r.state() == State::Repos);
    Pup v;
    assert(v.load(Pup().save()));
    assert(v.state() == State::Repos);
}

// Un Gemit retenu par un Jappe en attente part au tick suivant (bug SP5h :
// patte levee en silence au retour de promenade).
void gemit_differe_apres_jappe()
{
    Pup p;
    p.input(Input::Stroke, 0); // Jappe en attente
    p.claude_wait(true, 0);
    assert(p.tick(0) == Cue::Jappe);
    p.claude_wait(true, 1);
    assert(p.tick(1) == Cue::Gemit);
    p.claude_wait(true, 2);
    assert(p.tick(2) == Cue::None); // un seul par attente

    Pup q; // retour de promenade avec Claude qui attend
    q.input(Input::Laisse, 0);
    q.tick(0);
    q.claude_wait(true, 5);
    q.jauge(100, false, 15, true, 900);
    q.input(Input::Laisse, 901);
    q.claude_wait(true, 901);
    assert(q.tick(901) == Cue::Jappe);
    q.claude_wait(true, 902);
    assert(q.tick(902) == Cue::Gemit);
}

// C1 (relecture finale) : sortie faite hors de portee, PC en veille ; a la
// reconnexion l'absence est deja terminee, sa duree vient de absence_finie.
void sortie_hors_portee_pc_en_veille_reussie()
{
    Pup p;
    p.reminder_due(kSortie, 0);
    p.input(Input::Laisse, 1);
    for (int64_t t = 2; t < 1900; t += 60) p.jauge(0, true, 0, false, t); // hors de portee
    p.jauge(100, true, 0, true, 1900, 30, 0); // retour : absence de 30 min, finie a l'instant
    assert(p.state() == State::Promenade);
    assert(p.input(Input::Laisse, 1901) == Need::Bouger);

    // une absence terminee avant le depart ne compte pas
    Pup q;
    q.reminder_due(kSortie, 0);
    q.jauge(100, true, 0, true, 1000, 30, 10); // absence finie a ~400
    q.input(Input::Laisse, 1000);
    q.jauge(100, true, 0, true, 1100, 30, 11);
    assert(!q.input(Input::Laisse, 1101).has_value());
    assert(q.state() == State::Repos);
}

// C2 (relecture finale) : une reclamation dodo sans reponse est abandonnee au
// matin ; les rappels du jour sonnent, la laisse ne l'endort pas 12 h.
void dodo_sans_reponse_abandonne_au_matin()
{
    Pup p;
    p.reminder_due(kDodo, 0);
    p.set_quiet(true);
    assert(p.dodo_reclame());
    p.set_quiet(false); // 07:00
    assert(p.state() == State::Repos && !p.current_reminder().has_value());
    p.input(Input::Laisse, 40000);
    niveau(p, 100, 41000, false, 15);
    p.input(Input::Laisse, 41001);
    assert(p.state() == State::Content);
    p.reminder_due(kRepas, 41100);
    assert(p.state() == State::Reclame && p.reason() == Reason::Rappel);

    // une redemande dodo en attente est abandonnee aussi
    Pup q;
    q.reminder_due(kDodo, 0);
    q.input(Input::Laisse, 1);
    niveau(q, 90, 60, false, 1);
    q.input(Input::Laisse, 61); // trop courte : redemande a 661
    q.set_quiet(true);
    q.set_quiet(false);
    q.tick(700);
    assert(q.state() == State::Repos && !q.dodo_reclame());
}

} // namespace

// --- SPEC-012 : patte puis aboie, pose ---

std::string pose(const Pup &p) { return p.pose(); }

void claude_patte_puis_aboie_au_delai()
{
    Pup p; // delai par defaut : 2 min
    p.claude_wait(true, 0);
    assert(p.state() == State::Reclame && p.reason() == Reason::Claude);
    assert(pose(p) == "patte");
    assert(p.tick(0) == Cue::Gemit);
    assert(silent(p, 1, 119));
    assert(pose(p) == "patte");
    assert(p.tick(120) == Cue::Aboie);
    assert(pose(p) == "aboie");
    assert(silent(p, 121, 1000)); // un seul aboiement
    p.claude_wait(true, 1000);
    assert(pose(p) == "aboie" && silent(p, 1001, 1100));
}

void claude_aboie_apres_le_gemit_en_attente()
{
    Pup p;
    p.claude_wait(true, 0); // Gemit jamais tire
    p.claude_wait(true, 130); // passe le delai sans tick
    assert(pose(p) == "aboie");
    assert(p.tick(130) == Cue::Gemit);
    assert(p.tick(131) == Cue::Aboie);
    assert(silent(p, 132, 300));
}

void claude_traitee_avant_le_delai_jamais_aboie()
{
    {   // tape
        Pup p;
        p.claude_wait(true, 0);
        p.tick(0);
        p.input(Input::Tap, 60);
        assert(pose(p) == "-");
        p.claude_wait(true, 61); // Tap = attente ecartee : pas de nouvelle reclamation
        assert(silent(p, 61, 600) && pose(p) == "-");
    }
    {   // laisse -> promenade
        Pup p;
        p.claude_wait(true, 0);
        p.tick(0);
        p.input(Input::Laisse, 60);
        assert(p.state() == State::Promenade && pose(p) == "-");
        assert(p.tick(61) == Cue::Jappe); // depart de promenade, pas d'aboiement
        assert(silent(p, 62, 600));
    }
    {   // fin de l'attente
        Pup p;
        p.claude_wait(true, 0);
        p.tick(0);
        p.claude_wait(false, 60);
        assert(p.state() == State::Repos && pose(p) == "-");
        assert(silent(p, 61, 600));
    }
    {   // fin de l'attente juste au delai : pas d'aboiement
        Pup p;
        p.claude_wait(true, 0);
        p.tick(0);
        p.claude_wait(false, 120);
        assert(silent(p, 120, 600) && pose(p) == "-");
    }
}

void claude_remplacee_puis_reprise_recompte_le_delai()
{
    Pup p;
    p.claude_wait(true, 0);
    p.tick(0);
    p.reminder_due(kSortie, 100); // le rappel remplace Claude : escalade arretee
    assert(pose(p) == "laisse");
    p.input(Input::LongPress, 110); // rappel reporte, retour au repos
    p.claude_wait(true, 111);       // Claude attend toujours : nouvelle reclamation
    assert(p.state() == State::Reclame && p.reason() == Reason::Claude);
    assert(pose(p) == "patte");
    for (int64_t t = 112; t < 111 + 120; ++t) {
        p.claude_wait(true, t);
        assert(p.tick(t) != Cue::Aboie);
        assert(pose(p) == "patte");
    }
    p.claude_wait(true, 111 + 120);
    assert(p.tick(111 + 120) == Cue::Aboie);
    assert(pose(p) == "aboie");
}

void claude_delai_regle()
{
    pup::Settings s;
    s.patte_avant_aboie_min = 5;
    Pup p;
    p.set_settings(s);
    p.claude_wait(true, 0);
    p.tick(0);
    assert(silent(p, 1, 299) && pose(p) == "patte");
    assert(p.tick(300) == Cue::Aboie && pose(p) == "aboie");
    // un nouveau delai ne touche pas une reclamation deja commencee
    s.patte_avant_aboie_min = 1;
    Pup q;
    q.claude_wait(true, 0);
    q.tick(0);
    q.set_settings(s);
    assert(silent(q, 1, 119) && pose(q) == "patte");
    assert(q.tick(120) == Cue::Aboie);
}

void claude_aboie_remplace_par_rappel_arrete_tout()
{
    Pup p;
    p.claude_wait(true, 0);
    p.tick(0);
    p.tick(120);
    assert(pose(p) == "aboie");
    p.reminder_due(kSortie, 121);
    assert(pose(p) == "laisse");
    p.input(Input::LongPress, 122);
    assert(pose(p) == "-");
}

void poses_des_autres_reclamations()
{
    Pup r;
    r.reminder_due(kRepas, 0);
    assert(pose(r) == "aboie");
    Pup q;
    q.reminder_due(kSortie, 0);
    assert(pose(q) == "laisse");
    Pup s;
    s.reminder_due(kSport, 0);
    assert(pose(s) == "laisse");
    Pup d;
    d.reminder_due(kDodo, 0);
    assert(pose(d) == "laisse");
    Pup p;
    assert(pose(p) == "-");
    vide(p, 0);
    assert(pose(p) == "laisse");
    p.input(Input::Laisse, 1);
    assert(pose(p) == "-");
}

// --- SPEC-012 / ADR-007 : pose aboie 3 s a chaque relance d'une reclamation en laisse ---

// Relances a 60, 180 et 420 s : aboie 3 s (t, t+1, t+2), laisse ensuite.
void relances_en_laisse(Pup &p, int64_t t0)
{
    assert(p.tick(t0) == Cue::Wouf && pose(p) == "laisse"); // Wouf : pas une relance
    int64_t prec = t0;
    for (const int64_t r : {60, 180, 420}) {
        for (int64_t t = prec + 1; t < t0 + r; ++t) {
            assert(p.tick(t) == Cue::None && pose(p) == "laisse");
        }
        assert(p.tick(t0 + r) == Cue::Aboie && pose(p) == "aboie");
        assert(p.tick(t0 + r + 1) == Cue::None && pose(p) == "aboie");
        assert(p.tick(t0 + r + 2) == Cue::None && pose(p) == "aboie");
        assert(p.tick(t0 + r + 3) == Cue::None && pose(p) == "laisse");
        prec = t0 + r + 3;
    }
    assert(silent(p, t0 + 424, t0 + 2000) && pose(p) == "laisse");
}

void relances_en_laisse_rappel_sortie()
{
    for (const Reminder &r : {kSortie, kSport, kDodo}) {
        Pup p;
        p.reminder_due(r, 0);
        relances_en_laisse(p, 0);
    }
}

void relances_en_laisse_jauge_vide()
{
    Pup p;
    vide(p, 0);
    relances_en_laisse(p, 0);
}

void relance_repas_aboie_de_bout_en_bout()
{
    Pup p;
    p.reminder_due(kRepas, 0);
    for (int64_t t = 0; t <= 500; ++t) {
        p.tick(t);
        assert(pose(p) == "aboie");
    }
}

void relance_arretee_par_la_fin_de_la_reclamation()
{
    {   // laisse posee sur le chiot
        Pup p;
        p.reminder_due(kSortie, 0);
        p.tick(0);
        assert(p.tick(60) == Cue::Aboie);
        p.input(Input::Laisse, 61);
        assert(pose(p) == "-" && p.state() == State::Promenade);
    }
    {   // appui long : report ; la reclamation revenue reprend a la laisse
        Pup p;
        p.reminder_due(kSortie, 0);
        p.tick(0);
        assert(p.tick(60) == Cue::Aboie);
        p.input(Input::LongPress, 61);
        assert(pose(p) == "-");
        assert(p.tick(61 + 600) == Cue::Wouf && pose(p) == "laisse");
        assert(p.tick(61 + 600 + 1) == Cue::None && pose(p) == "laisse");
    }
    {   // passage en Dort pendant les 3 s
        Pup p;
        p.reminder_due(kSortie, 0);
        p.tick(0);
        assert(p.tick(60) == Cue::Aboie);
        p.input(Input::FaceDown, 61);
        assert(p.state() == State::Dort && pose(p) == "-");
        p.input(Input::FaceUp, 62);
        assert(pose(p) == "-" && p.state() == State::Repos);
    }
    {   // rappel sortie qui remplace la jauge pendant les 3 s : sa propre pose
        Pup p;
        vide(p, 0);
        p.tick(0);
        assert(p.tick(60) == Cue::Aboie && pose(p) == "aboie");
        p.reminder_due(kSortie, 61);
        assert(pose(p) == "laisse");
        assert(p.tick(61) == Cue::Wouf && pose(p) == "laisse");
        assert(p.tick(62) == Cue::None && pose(p) == "laisse");
    }
    {   // rappel qui remplace une reclamation Claude aboyante : laisse
        Pup p;
        p.claude_wait(true, 0);
        p.tick(120);
        assert(pose(p) == "aboie");
        p.reminder_due(kSortie, 121);
        assert(pose(p) == "laisse");
    }
    {   // fin de la reclamation de la jauge (donnees perimees)
        Pup p;
        vide(p, 0);
        p.tick(0);
        assert(p.tick(60) == Cue::Aboie && pose(p) == "aboie");
        p.jauge(100, false, 0, false, 61);
        assert(pose(p) == "-");
    }
}

int main()
{
    relances_en_laisse_rappel_sortie();
    relances_en_laisse_jauge_vide();
    relance_repas_aboie_de_bout_en_bout();
    relance_arretee_par_la_fin_de_la_reclamation();
    claude_patte_puis_aboie_au_delai();
    claude_aboie_apres_le_gemit_en_attente();
    claude_traitee_avant_le_delai_jamais_aboie();
    claude_remplacee_puis_reprise_recompte_le_delai();
    claude_delai_regle();
    claude_aboie_remplace_par_rappel_arrete_tout();
    poses_des_autres_reclamations();
    sortie_hors_portee_pc_en_veille_reussie();
    dodo_sans_reponse_abandonne_au_matin();
    gemit_differe_apres_jappe();
    jauge_vide_reclame_la_laisse();
    jauge_ignoree_si_perimee();
    laisse_part_et_revient_content();
    promenade_a_la_demande_depuis_le_repos();
    promenade_trop_courte_redemande_apres_600s();
    absence_sans_laisse_calme_la_reclamation();
    retour_au_pc_termine_la_promenade();
    garde_fou_trois_heures();
    sortie_jugee_sur_l_absence();
    sortie_redemandee_deux_fois_au_plus();
    fin_sans_donnees_fraiches_content();
    dodo_puis_dort_jusqu_au_matin();
    dodo_survit_au_debut_des_heures_calmes_en_silence();
    rappel_abandonne_pendant_la_promenade();
    repas_garde_le_report();
    sortie_sport_dodo_se_reportent();
    evenements_report_accepte_abandonne();
    evenements_caresse_et_os();
    jauge_appui_long_arrete_la_reclamation();
    accepter_un_rappel_reporte();
    dodo_reporte_survit_au_calme_en_silence();
    promenade_reussie_rend_bouger();
    promenade_survit_a_save_load();
    dort_prime_sur_reclame();
    rappel_tap_content_et_besoin();
    rappel_appui_long_reporte_600_s();
    inactivite_wouf_puis_escalade();
    claude_un_seul_gemit();
    claude_sort_sur_false_ou_tap();
    caresse_content_jappe_jouer();
    lancer_rapporte_puis_repos();
    secouer_tournis_puis_repos();
    brosse_bain_puis_repos();
    os_mange_puis_repos();
    os_ignore_en_dort_et_reclame();
    brosse_ignoree_en_dort_et_reclame();
    content_puis_repos();
    boude_remplace_repos_sans_bloquer();

    report_regle();
    calme_regle();
    set_settings_ne_touche_pas_l_etat_en_cours();
    calme_coupe_tout_son();
    retourne_abandonne_le_rappel();
    boude_n_est_pas_terminal();

    ra_calme_abandonne_l_escalade_en_cours();
    rb_priorite_entre_raisons();
    rc_signaux_de_niveau_idempotents();
    re_claude_nouvelle_attente_nouveau_gemit();
    rf_cues_une_seule_fois();
    rg_reclame_seuls_tap_et_appui_long();
    rh_tap_au_repos_sans_effet();
    ri_besoins_rendus();
    rj_boude_etat_de_repos();
    rp_rappel_ouvre_la_fenetre_calme();
    rq_gemit_en_attente_abandonne();
    rq_gemit_n_ecrase_pas_jappe();
    return 0;
}
