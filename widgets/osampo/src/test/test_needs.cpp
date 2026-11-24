// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>
#include <string>

#include "logic/needs.h"
#include "logic/settings.h"

namespace {

using pup::Need;
using pup::Needs;

void depart_70_partout()
{
    Needs n;
    assert(n.level(Need::Bouger) == 70);
    assert(n.level(Need::Jouer) == 70);
    assert(n.level(Need::Dormir) == 70);
    assert(!n.sulking());
}

void un_jour_sans_satisfaction()
{
    Needs n;
    n.on_day(0);
    n.on_day(1);
    assert(n.level(Need::Bouger) == 55);
    assert(n.level(Need::Jouer) == 55);
    assert(n.level(Need::Dormir) == 55);
}

void premier_on_day_ne_decroit_pas()
{
    Needs n;
    n.on_day(20000);
    assert(n.level(Need::Bouger) == 70);
}

void satisfaction_protege_le_jour()
{
    Needs n;
    n.on_day(0);
    n.on_day(1);
    n.satisfy(Need::Jouer, 1);
    n.on_day(2);
    assert(n.level(Need::Jouer) == 75);
    assert(n.level(Need::Bouger) == 40);
    assert(n.level(Need::Dormir) == 40);
}

void satisfy_plafonne_a_100()
{
    Needs n;
    n.on_day(0);
    for (int i = 0; i < 5; i++) n.satisfy(Need::Dormir, 0);
    assert(n.level(Need::Dormir) == 100);
}

void trois_jours_d_absence_decroissent_trois_fois()
{
    Needs n;
    n.on_day(0);
    n.on_day(3);
    assert(n.level(Need::Bouger) == 25);
    assert(n.sulking());
}

void jour_rejoue_ne_redecroit_pas()
{
    Needs n;
    n.on_day(5);
    n.on_day(5);
    n.on_day(4);
    assert(n.level(Need::Jouer) == 70);
    n.on_day(6);
    n.on_day(6);
    n.on_day(5);
    assert(n.level(Need::Jouer) == 55);
}

void trente_jours_de_negligence_puis_retour()
{
    Needs n;
    n.on_day(0);
    n.on_day(30);
    assert(n.level(Need::Bouger) == 0);
    assert(n.level(Need::Jouer) == 0);
    assert(n.level(Need::Dormir) == 0);
    assert(n.sulking());
    n.satisfy(Need::Bouger, 30);
    assert(!n.sulking());
    assert(n.level(Need::Bouger) == 20);
}

void gros_saut_de_jours_sans_boucle_interminable()
{
    Needs n;
    n.on_day(0);
    n.on_day(4000000000000LL);
    assert(n.level(Need::Jouer) == 0);
}

void icon_aux_bornes()
{
    Needs n;
    n.on_day(0);
    // 70 -> 55 -> 40 -> 25 ; on remonte par pas de 20 : 45, 65, 85
    assert(n.icon(Need::Jouer) == 2); // 70
    n.on_day(1);
    assert(n.icon(Need::Jouer) == 1); // 55
    n.on_day(3);
    assert(n.icon(Need::Jouer) == 0); // 25
    Needs m;
    m.on_day(0);
    // 29, 30 et 69 ne sont pas atteignables par decroissance : via load.
    assert(m.load("needs1 29 30 69 0 0 x x x"));
    assert(m.icon(Need::Bouger) == 0);
    assert(m.icon(Need::Jouer) == 1);
    assert(m.icon(Need::Dormir) == 1);
    assert(m.load("needs1 70 70 70 0 0 x x x"));
    assert(m.icon(Need::Bouger) == 2);
    assert(m.load("needs1 70 100 0 0 0 x x x"));
    assert(m.icon(Need::Bouger) == 2);
    assert(m.icon(Need::Jouer) == 2);
    assert(m.icon(Need::Dormir) == 0);
}

void save_load_aller_retour()
{
    Needs a;
    a.on_day(10);
    a.on_day(12);
    a.satisfy(Need::Jouer, 12);
    std::string s = a.save();
    assert(s.find('\n') == std::string::npos);
    Needs b;
    assert(b.load(s));
    assert(b.save() == s);
    for (Need k : {Need::Bouger, Need::Jouer, Need::Dormir})
        assert(a.level(k) == b.level(k));
    assert(a.sulking() == b.sulking());
    // Les drapeaux du jour survivent : meme evolution apres rechargement.
    a.on_day(13);
    b.on_day(13);
    assert(a.level(Need::Jouer) == b.level(Need::Jouer));
    assert(b.level(Need::Jouer) == 60);
    assert(b.level(Need::Bouger) == 25);
}

void save_load_jour_non_fixe()
{
    Needs a;
    a.satisfy(Need::Dormir, 3);
    Needs b;
    assert(b.load(a.save()));
    assert(b.save() == a.save());
    // satisfy a avance le jour a 3 : on_day(4) decroit d'un jour.
    b.on_day(4);
    assert(b.level(Need::Bouger) == 55);
    assert(b.level(Need::Dormir) == 90); // satisfait le jour 3
}

void satisfy_avance_le_jour_futur()
{
    Needs n;
    n.on_day(10);
    n.satisfy(Need::Jouer, 11); // on_day(11) : tous 55, Jouer 75
    assert(n.level(Need::Jouer) == 75);
    assert(n.level(Need::Bouger) == 55);
    n.on_day(13);
    assert(n.level(Need::Jouer) == 60);
    assert(n.level(Need::Bouger) == 25);
}

void satisfy_avance_le_jour_plus_loin()
{
    Needs n;
    n.on_day(10);
    n.satisfy(Need::Jouer, 12); // 10 -> 12 : Jouer 40 + 20 = 60
    assert(n.level(Need::Jouer) == 60);
    n.on_day(13);
    assert(n.level(Need::Jouer) == 60);
    assert(n.level(Need::Bouger) == 25);
}

void satisfy_tardif_sans_exemption()
{
    Needs n;
    n.on_day(10);
    n.on_day(12); // tous 40
    n.satisfy(Need::Jouer, 11);
    assert(n.level(Need::Jouer) == 60);
    n.on_day(13);
    assert(n.level(Need::Jouer) == 45); // pas d'exemption
}

void load_invalide_laisse_l_etat()
{
    Needs n;
    n.on_day(0);
    n.on_day(2);
    std::string avant = n.save();
    assert(!n.load(""));
    assert(!n.load("n'importe quoi"));
    assert(!n.load("needs1 101 0 0 0 0 x x x"));
    assert(!n.load("needs1 -1 0 0 0 0 x x x"));
    assert(!n.load("needs1 50 50 50 2 0 x x x"));
    assert(!n.load("needs1 50 50 50 0 0 x x"));
    assert(!n.load("needs1 50 50 50 0 0 x x x extra"));
    assert(n.save() == avant);
    assert(n.level(Need::Bouger) == 40);
}

void besoins_regles()
{
    pup::Settings s;
    s.baisse_jour = 5;
    s.gain = 10;
    Needs n;
    n.set_settings(s);
    n.on_day(0);
    n.on_day(1);
    assert(n.level(Need::Bouger) == 65);
    n.satisfy(Need::Bouger, 1);
    assert(n.level(Need::Bouger) == 75);
}

} // namespace

int main()
{
    depart_70_partout();
    un_jour_sans_satisfaction();
    premier_on_day_ne_decroit_pas();
    satisfaction_protege_le_jour();
    satisfy_plafonne_a_100();
    trois_jours_d_absence_decroissent_trois_fois();
    jour_rejoue_ne_redecroit_pas();
    trente_jours_de_negligence_puis_retour();
    gros_saut_de_jours_sans_boucle_interminable();
    icon_aux_bornes();
    save_load_aller_retour();
    save_load_jour_non_fixe();
    satisfy_avance_le_jour_futur();
    satisfy_avance_le_jour_plus_loin();
    satisfy_tardif_sans_exemption();
    load_invalide_laisse_l_etat();
    besoins_regles();
    return 0;
}
