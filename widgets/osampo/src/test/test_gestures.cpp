// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "logic/gestures.h"
#include "logic/settings.h"

namespace {

using pup::GestureDetector;
using pup::Input;
using pup::Sample;
using pup::Settings;

// Seuils de lancer sur lesquels les scenarios synthetiques ont ete cales
// (avant la calibration). Les traces reelles, elles, rejouent les defauts.
Settings base()
{
    Settings s;
    s.lancer_mg = 2500;
    s.lancer_ms = 60;
    return s;
}

// Genere une trace a 100 Hz et note les gestes rendus avec leur instant.
struct Trace {
    GestureDetector det;
    int64_t t = 0;
    std::vector<std::pair<int64_t, Input>> gestes;

    explicit Trace(const Settings &s = base()) { det.set_settings(s); }

    // dur ms d'un etat constant (un echantillon toutes les 10 ms).
    void tenir(int dur, float ax, float ay, float az, float gx = 0, float gy = 0, float gz = 0)
    {
        for (int k = 0; k < dur; k += 10) {
            auto r = det.feed(Sample{t, ax, ay, az, gx, gy, gz});
            if (r)
                gestes.emplace_back(t, *r);
            t += 10;
        }
    }
    void a_plat(int dur) { tenir(dur, 0, 0, 1.0f); }
    void a_l_envers(int dur) { tenir(dur, 0, 0, -1.0f); }

    int compte(Input i) const
    {
        int n = 0;
        for (auto &g : gestes)
            n += g.second == i;
        return n;
    }
};

// Retournement : 300 ms de rotation (gyro 200 deg/s), az de +1 a -1.
void retourner(Trace &tr, float vers)
{
    for (int k = 0; k < 30; ++k) {
        float f = (k + 1) / 30.0f;
        float az = -vers + 2 * vers * f;
        tr.tenir(10, 0, 0.5f, az, 200, 0, 0);
    }
}

// Lancer : 100 ms a 3 g avec 500 deg/s.
void pic_de_lancer(Trace &tr) { tr.tenir(100, 2.0f, 0, 2.2f, 300, 400, 0); }

void retournement_puis_retour()
{
    Trace tr;
    tr.a_plat(1000);
    retourner(tr, -1.0f);
    int64_t pose = tr.t;
    tr.a_l_envers(1500);
    assert(tr.gestes.empty()); // rien avant 1500 ms
    tr.a_l_envers(10);
    assert(tr.gestes.size() == 1);
    assert(tr.gestes[0].second == Input::FaceDown);
    assert(tr.gestes[0].first == pose + 1500);
    tr.a_l_envers(3000); // une seule fois
    assert(tr.gestes.size() == 1);

    retourner(tr, 1.0f);
    // La fin du retournement a deja az >= 0,8 g : on mesure depuis la pose.
    int64_t debout = tr.t;
    tr.a_plat(1000);
    assert(tr.compte(Input::FaceUp) == 1);
    assert(tr.compte(Input::FaceDown) == 1);
    assert(tr.gestes.size() == 2);
    assert(tr.gestes[1].second == Input::FaceUp);
    assert(tr.gestes[1].first <= debout + 500);
    // Les 3 ou 4 derniers echantillons de la rotation ont deja az >= 0,8 g
    // (az = 0,8 a l'arrondi flottant pres).
    assert(tr.gestes[1].first >= debout - 40 + 500);
    assert(tr.compte(Input::Throw) == 0 && tr.compte(Input::Shake) == 0);
}

// FaceUp tenu exactement 500 ms a plat, sans rotation prealable a +0,8.
void face_up_apres_500_ms()
{
    Trace tr;
    tr.a_l_envers(1510);
    assert(tr.compte(Input::FaceDown) == 1);
    int64_t debout = tr.t;
    tr.tenir(500, 0, 0, 1.0f);
    assert(tr.compte(Input::FaceUp) == 0);
    tr.tenir(10, 0, 0, 1.0f);
    assert(tr.compte(Input::FaceUp) == 1);
    assert(tr.gestes.back().first == debout + 500);
    // Un bref passage a l'endroit ne suffit pas.
    Trace tr2;
    tr2.a_l_envers(1510);
    tr2.a_plat(300);
    tr2.tenir(100, 0, 0, 0.5f);
    tr2.a_plat(300);
    assert(tr2.compte(Input::FaceUp) == 0);
}

void pose_a_l_envers_bref_rien()
{
    Trace tr;
    tr.a_plat(1000);
    retourner(tr, -1.0f);
    tr.a_l_envers(800);
    retourner(tr, 1.0f);
    tr.a_plat(3000);
    assert(tr.gestes.empty());

    // A l'envers mais agite (gyro) : pas immobile, le compteur repart.
    Trace tr2;
    tr2.a_l_envers(1000);
    tr2.tenir(50, 0, 0, -1.0f, 60, 0, 0);
    tr2.a_l_envers(1000);
    assert(tr2.gestes.empty());
    tr2.a_l_envers(600);
    assert(tr2.compte(Input::FaceDown) == 1);
}

// Un coup de secousse : 60 ms a 2,5 g puis 190 ms a 0,3 g (periode 250 ms).
void coup(Trace &tr, float g_gyro = 50)
{
    tr.tenir(60, 0, 0, 2.5f, g_gyro, 0, 0);
    tr.tenir(190, 0, 0, 0.3f, g_gyro, 0, 0);
}

void secousse_quatre_coups()
{
    Trace tr;
    tr.a_plat(500);
    for (int i = 0; i < 3; ++i)
        coup(tr);
    assert(tr.gestes.empty());
    int64_t quatrieme = tr.t;
    coup(tr);
    assert(tr.gestes.size() == 1);
    assert(tr.gestes[0].second == Input::Shake);
    assert(tr.gestes[0].first == quatrieme); // tout de suite
    tr.a_plat(3000);
    assert(tr.gestes.size() == 1);

    // Secousse avec forte rotation (chaque coup qualifie un lancer) : Shake seul.
    Trace tr2;
    tr2.a_plat(500);
    for (int i = 0; i < 4; ++i) {
        tr2.tenir(80, 0, 0, 2.8f, 500, 0, 0);
        tr2.tenir(170, 0, 0, 0.3f, 500, 0, 0);
    }
    tr2.a_plat(3000);
    assert(tr2.compte(Input::Shake) == 1);
    assert(tr2.compte(Input::Throw) == 0);
    assert(tr2.gestes.size() == 1);

    // Secousse prolongee : un seul Shake (le reste de la salve est ignore).
    Trace tr3;
    tr3.a_plat(500);
    for (int i = 0; i < 12; ++i)
        coup(tr3);
    tr3.a_plat(3000);
    assert(tr3.gestes.size() == 1 && tr3.compte(Input::Shake) == 1);

    // Trois coups seulement : rien.
    Trace tr4;
    tr4.a_plat(500);
    for (int i = 0; i < 3; ++i)
        coup(tr4);
    tr4.a_plat(3000);
    assert(tr4.gestes.empty());

    // Norme qui oscille autour du seuil (2,05 / 1,9 g) : un seul pic, pas de
    // rearmement sans repasser sous secousse_g - 0,3 g.
    Trace tr6;
    tr6.a_plat(500);
    for (int i = 0; i < 20; ++i) {
        tr6.tenir(10, 0, 0, 2.05f);
        tr6.tenir(10, 0, 0, 1.9f);
    }
    tr6.a_plat(3000);
    assert(tr6.gestes.empty());

    // Quatre coups trop lents (400 ms) : hors fenetre, rien.
    Trace tr5;
    tr5.a_plat(500);
    for (int i = 0; i < 4; ++i) {
        tr5.tenir(60, 0, 0, 2.5f, 50, 0, 0);
        tr5.tenir(340, 0, 0, 0.3f, 50, 0, 0);
    }
    tr5.a_plat(3000);
    assert(tr5.gestes.empty());
}

void lancer_un_pic_avec_rotation()
{
    Trace tr;
    tr.a_plat(500);
    int64_t debut = tr.t;
    pic_de_lancer(tr);
    tr.a_plat(890);
    assert(tr.gestes.empty()); // decide a la fin de la fenetre, pas avant
    tr.a_plat(2000);
    assert(tr.gestes.size() == 1);
    assert(tr.gestes[0].second == Input::Throw);
    // Candidat confirme a debut + 60 ms, decide 1000 ms apres.
    assert(tr.gestes[0].first == debut + 60 + 1000);

    // Pic trop court (40 ms) : rien.
    Trace tr2;
    tr2.a_plat(500);
    tr2.tenir(40, 2.0f, 0, 2.2f, 300, 400, 0);
    tr2.a_plat(3000);
    assert(tr2.gestes.empty());

    // Pic fort sans rotation : rien.
    Trace tr3;
    tr3.a_plat(500);
    tr3.tenir(100, 2.0f, 0, 2.2f, 50, 0, 0);
    tr3.a_plat(3000);
    assert(tr3.gestes.empty());
}

void lancer_puis_rattrapage_un_seul_geste()
{
    Trace tr;
    tr.a_plat(500);
    pic_de_lancer(tr);
    tr.tenir(300, 0, 0, 0.1f, 300, 0, 0); // vol (chute libre, rotation)
    tr.tenir(70, 0, 0, 2.0f, 80, 0, 0);    // rattrapage a 2 g, 400 ms apres
    tr.tenir(70, 0, 0, 2.6f, 400, 0, 0);   // rattrapage brutal, autre pic fort
    tr.a_plat(3000);
    assert(tr.gestes.size() == 1);
    assert(tr.compte(Input::Throw) == 1);
    assert(tr.compte(Input::Shake) == 0);
}

void pause_apres_geste()
{
    // Deux lancers a 1200 ms d'ecart : le second tombe dans la pause.
    Trace tr;
    tr.a_plat(500);
    pic_de_lancer(tr);
    tr.a_plat(1100);
    assert(tr.compte(Input::Throw) == 1);
    pic_de_lancer(tr);
    tr.a_plat(3000);
    assert(tr.gestes.size() == 1);
    // Apres la pause, un nouveau lancer compte.
    pic_de_lancer(tr);
    tr.a_plat(3000);
    assert(tr.compte(Input::Throw) == 2);

    // Secousse puis secousse dans la pause : un Shake.
    Trace tr2;
    tr2.a_plat(500);
    for (int i = 0; i < 4; ++i)
        coup(tr2);
    tr2.a_plat(400);
    for (int i = 0; i < 4; ++i)
        coup(tr2);
    tr2.a_plat(3000);
    assert(tr2.compte(Input::Shake) == 1);
}

void manipulation_normale_rien()
{
    Trace tr;
    tr.a_plat(1000);
    // Prise : 150 ms a 1,4 g, rotation moderee.
    tr.tenir(150, 0.3f, 0, 1.37f, 150, 80, 0);
    // Transport : leger balancement.
    for (int i = 0; i < 10; ++i) {
        tr.tenir(100, 0.2f, 0.1f, 1.1f, 60, 40, 20);
        tr.tenir(100, -0.2f, 0, 0.9f, -60, -40, 0);
    }
    // Pose : 150 ms a 1,4 g puis choc leger.
    tr.tenir(150, 0, 0.2f, 1.38f, 120, 0, 50);
    tr.tenir(30, 0, 0, 1.6f, 40, 0, 0);
    tr.a_plat(3000);
    // Tourner la board sur la table (rotation 250 deg/s, a plat).
    tr.tenir(500, 0, 0, 1.0f, 0, 0, 250);
    tr.a_plat(3000);
    assert(tr.gestes.empty());
}

void seuils_regles()
{
    Settings s = base();
    s.lancer_mg = 4000;
    Trace tr(s);
    tr.a_plat(500);
    pic_de_lancer(tr);
    tr.a_plat(3000);
    assert(tr.gestes.empty());

    // lancer_dps 600 : la rotation de 500 deg/s ne suffit plus.
    Settings s2 = base();
    s2.lancer_dps = 600;
    Trace tr2(s2);
    tr2.a_plat(500);
    pic_de_lancer(tr2);
    tr2.a_plat(3000);
    assert(tr2.gestes.empty());

    // lancer_ms 150 : le pic de 100 ms est trop court.
    Settings s3 = base();
    s3.lancer_ms = 150;
    Trace tr3(s3);
    tr3.a_plat(500);
    pic_de_lancer(tr3);
    tr3.a_plat(3000);
    assert(tr3.gestes.empty());

    // secousse_coups 6 : quatre coups ne suffisent plus, six oui.
    Settings s4 = base();
    s4.secousse_coups = 6;
    s4.secousse_fenetre_ms = 1500;
    Trace tr4(s4);
    tr4.a_plat(500);
    for (int i = 0; i < 4; ++i)
        coup(tr4);
    tr4.a_plat(3000);
    assert(tr4.gestes.empty());
    for (int i = 0; i < 6; ++i)
        coup(tr4);
    tr4.a_plat(3000);
    assert(tr4.gestes.size() == 1 && tr4.compte(Input::Shake) == 1);

    // secousse_mg 3000 : les coups a 2,5 g ne sont plus des pics.
    Settings s5 = base();
    s5.secousse_mg = 3000;
    Trace tr5(s5);
    tr5.a_plat(500);
    for (int i = 0; i < 6; ++i)
        coup(tr5);
    tr5.a_plat(3000);
    assert(tr5.gestes.empty());

    // retourne_ms 3000.
    Settings s6 = base();
    s6.retourne_ms = 3000;
    Trace tr6(s6);
    tr6.a_l_envers(3000);
    assert(tr6.gestes.empty());
    tr6.a_l_envers(10);
    assert(tr6.compte(Input::FaceDown) == 1);

    // secousse_fenetre_ms 500 : le lancer est decide 500 ms apres.
    Settings s7 = base();
    s7.secousse_fenetre_ms = 500;
    Trace tr7(s7);
    tr7.a_plat(500);
    int64_t debut = tr7.t;
    pic_de_lancer(tr7);
    tr7.a_plat(2000);
    assert(tr7.gestes.size() == 1 && tr7.gestes[0].first == debut + 60 + 500);

    // pause_gestes_ms 0 : deux lancers rapproches comptent tous les deux.
    Settings s8 = base();
    s8.pause_gestes_ms = 0;
    Trace tr8(s8);
    tr8.a_plat(500);
    pic_de_lancer(tr8);
    tr8.a_plat(1100);
    pic_de_lancer(tr8);
    tr8.a_plat(3000);
    assert(tr8.compte(Input::Throw) == 2);

    // set_settings en cours de route : vaut des l'echantillon suivant.
    Trace tr9;
    tr9.a_plat(500);
    tr9.det.set_settings(s);
    pic_de_lancer(tr9);
    tr9.a_plat(3000);
    assert(tr9.gestes.empty());
}

bool commence_par(const std::string &s, const std::string &p) { return s.compare(0, p.size(), p) == 0; }
bool finit_par(const std::string &s, const std::string &f)
{
    return s.size() >= f.size() && s.compare(s.size() - f.size(), f.size(), f) == 0;
}

void journal_une_ligne_par_salve()
{
    // journal 0 : muet.
    {
        Trace tr;
        tr.a_plat(500);
        pic_de_lancer(tr);
        tr.a_plat(3000);
        assert(tr.det.take_journal().empty());
    }
    Settings s = base();
    s.journal = 1;
    Trace tr(s);
    tr.a_plat(500);
    pic_de_lancer(tr);
    tr.a_plat(3000);
    auto j = tr.det.take_journal();
    assert(j.size() == 1);
    // Norme sqrt(2^2 + 2,2^2) = 2,973 g ; gyro sqrt(300^2 + 400^2) = 500.
    assert(j[0] == "salve pic_mg=2973 pic_dps=500 ms=90 coups=1 -> lancer sens=gauche grav=0.000,0.000,1.000 perdus=0");
    assert(tr.det.take_journal().empty()); // vide a la lecture

    for (int i = 0; i < 4; ++i)
        coup(tr);
    tr.a_plat(3000);
    j = tr.det.take_journal();
    assert(j.size() == 1);
    assert(commence_par(j[0], "salve pic_mg=2500 pic_dps=50 ms="));
    assert(finit_par(j[0], " coups=4 -> secousse grav=0.000,0.000,1.000 perdus=0"));

    tr.tenir(150, 0, 0, 1.4f, 150, 0, 0);
    tr.a_plat(3000);
    j = tr.det.take_journal();
    assert(j.size() == 1);
    assert(j[0] == "salve pic_mg=1400 pic_dps=150 ms=140 coups=0 -> rien grav=0.000,0.000,1.000 perdus=0");

    // grav = moyenne du calme qui termine la salve (board inclinee).
    tr.tenir(150, 0, 0, 1.4f, 150, 0, 0);
    tr.tenir(1000, 0.6f, 0, 0.8f);
    j = tr.det.take_journal();
    assert(j.size() == 1);
    assert(j[0] == "salve pic_mg=1400 pic_dps=150 ms=140 coups=0 -> rien grav=0.600,0.000,0.800 perdus=0");
    tr.a_plat(500);
    tr.det.take_journal();

    // Deux salves separees : deux lignes.
    tr.tenir(100, 0, 0, 1.6f);
    tr.a_plat(500);
    tr.tenir(100, 0, 0, 0.4f);
    tr.a_plat(500);
    j = tr.det.take_journal();
    assert(j.size() == 2);

    // journal 2 : les echantillons de la salve avant la ligne "salve".
    Settings s2 = base();
    s2.journal = 2;
    Trace tr2(s2);
    tr2.a_plat(500);
    tr2.tenir(150, 0, 0, 1.4f, 150, 0, 0);
    tr2.a_plat(1000);
    j = tr2.det.take_journal();
    // 15 echantillons agites + 30 de calme avant la fin de salve.
    assert(j.size() == 15 + 30 + 1);
    assert(j[0] == "S 500 0.000 0.000 1.400 150.000 0.000 0.000");
    assert(j[15] == "S 650 0.000 0.000 1.000 0.000 0.000 0.000");
    for (size_t i = 0; i + 1 < j.size(); ++i)
        assert(commence_par(j[i], "S "));
    assert(j.back() == "salve pic_mg=1400 pic_dps=150 ms=140 coups=0 -> rien grav=0.000,0.000,1.000 perdus=0");

    // journal 2 : salve longue plafonnee.
    Trace tr3(s2);
    tr3.a_plat(500);
    for (int i = 0; i < 40; ++i) {
        tr3.tenir(100, 0, 0, 1.6f);
        tr3.tenir(100, 0, 0, 0.4f);
    }
    tr3.a_plat(1000);
    j = tr3.det.take_journal();
    assert(j.size() == GestureDetector::kMaxSalve + 1);
    // 800 echantillons agites + 30 de calme, 300 gardes.
    assert(j.back() == "salve pic_mg=1600 pic_dps=0 ms=7990 coups=0 -> rien grav=0.000,0.000,1.000 perdus=530");
}

void journal_faces()
{
    // journal 1 : une ligne par transition, valeurs de l'echantillon de decision.
    Settings s = base();
    s.journal = 1;
    Trace tr(s);
    tr.a_l_envers(1510);
    tr.a_plat(510);
    assert(tr.compte(Input::FaceDown) == 1 && tr.compte(Input::FaceUp) == 1);
    auto j = tr.det.take_journal();
    assert(j.size() == 2);
    assert(j[0] == "face bas grav=0.000,0.000,-1.000");
    assert(j[1] == "face haut grav=0.000,0.000,1.000");

    // journal 0 : rien.
    Trace tr0;
    tr0.a_l_envers(1510);
    assert(tr0.det.take_journal().empty());

    // journal 2 : les echantillons du candidat (0 a 1500 ms) puis "face bas".
    Settings s2 = base();
    s2.journal = 2;
    Trace tr2(s2);
    tr2.a_l_envers(1510);
    j = tr2.det.take_journal();
    assert(j.size() == 151 + 1);
    assert(j[0] == "S 0 0.000 0.000 -1.000 0.000 0.000 0.000");
    assert(j[150] == "S 1500 0.000 0.000 -1.000 0.000 0.000 0.000");
    assert(j[151] == "face bas grav=0.000,0.000,-1.000 perdus=0");
    tr2.a_plat(510);
    j = tr2.det.take_journal();
    assert(j.size() == 1 && j[0] == "face haut grav=0.000,0.000,1.000");

    // Candidat abandonne : rien, et le candidat suivant repart de zero.
    Trace tr3(s2);
    tr3.a_l_envers(800);
    tr3.a_plat(500);
    assert(tr3.det.take_journal().empty());
    tr3.a_l_envers(1510);
    j = tr3.det.take_journal();
    assert(j.size() == 152);
    assert(j[0] == "S 1300 0.000 0.000 -1.000 0.000 0.000 0.000");

    // Plafond : retourne_ms 5000 -> 501 echantillons, 300 gardes.
    Settings s4 = s2;
    s4.retourne_ms = 5000;
    Trace tr4(s4);
    tr4.a_l_envers(5010);
    j = tr4.det.take_journal();
    assert(j.size() == GestureDetector::kMaxSalve + 1);
    assert(j.back() == "face bas grav=0.000,0.000,-1.000 perdus=201");
}

// Rejoue une trace reelle (lignes "S t ax ay az gx gy gz", '#' = commentaire,
// lignes vides et fins CRLF tolerees)
// sur un detecteur neuf aux reglages par defaut, puis laisse 2 s de calme
// (0, 0, az_calme) pour que les decisions en attente tombent.
Trace rejouer(const char *nom, float az_calme)
{
    std::string chemin = std::string(TRACES_DIR) + "/" + nom;
    std::ifstream f(chemin);
    if (!f) {
        std::fprintf(stderr, "trace introuvable : %s\n", chemin.c_str());
        std::abort();
    }
    Trace tr{Settings{}};
    std::string ligne;
    int64_t dernier = 0;
    int n = 0;
    while (std::getline(f, ligne)) {
        if (!ligne.empty() && ligne.back() == '\r')
            ligne.pop_back();
        auto debut = ligne.find_first_not_of(" \t");
        if (debut == std::string::npos || ligne[debut] == '#')
            continue;
        std::istringstream is(ligne);
        std::string tag;
        long long t;
        float v[6];
        is >> tag >> t;
        for (float &x : v)
            is >> x;
        assert(tag == "S" && !is.fail());
        auto r = tr.det.feed(Sample{t, v[0], v[1], v[2], v[3], v[4], v[5]});
        if (r)
            tr.gestes.emplace_back(t, *r);
        dernier = t;
        ++n;
    }
    assert(n > 0);
    tr.t = dernier + 10;
    tr.tenir(2000, 0, 0, az_calme);
    return tr;
}

void traces_reelles()
{
    for (const char *nom : {"lancer_1.txt", "lancer_2.txt"}) {
        Trace tr = rejouer(nom, 1.0f);
        if (tr.gestes.size() != 1 || tr.compte(Input::Throw) != 1)
            std::fprintf(stderr, "%s : %zu gestes, Throw=%d Shake=%d\n", nom, tr.gestes.size(),
                         tr.compte(Input::Throw), tr.compte(Input::Shake));
        assert(tr.gestes.size() == 1 && tr.compte(Input::Throw) == 1);
    }
    // Sens du lancer, ecran face a soi : 5 lancers de chaque cote.
    for (int i = 1; i <= 5; ++i) {
        for (bool droite : {false, true}) {
            std::string nom = std::string("lancer_") + (droite ? "droite_" : "gauche_") + std::to_string(i) + ".txt";
            Trace tr = rejouer(nom.c_str(), 1.0f);
            if (tr.gestes.size() != 1 || tr.compte(Input::Throw) != 1 || tr.det.lancer_droite() != droite)
                std::fprintf(stderr, "%s : %zu gestes, Throw=%d droite=%d\n", nom.c_str(), tr.gestes.size(),
                             tr.compte(Input::Throw), (int)tr.det.lancer_droite());
            assert(tr.gestes.size() == 1 && tr.compte(Input::Throw) == 1);
            assert(tr.det.lancer_droite() == droite);
        }
    }
    for (const char *nom : {"secousse_1.txt", "secousse_2.txt"}) {
        Trace tr = rejouer(nom, 1.0f);
        if (tr.gestes.size() != 1 || tr.compte(Input::Shake) != 1)
            std::fprintf(stderr, "%s : %zu gestes, Throw=%d Shake=%d\n", nom, tr.gestes.size(),
                         tr.compte(Input::Throw), tr.compte(Input::Shake));
        assert(tr.gestes.size() == 1 && tr.compte(Input::Shake) == 1);
    }
    {
        Trace tr = rejouer("retournement.txt", -1.0f);
        if (tr.gestes.size() != 1 || tr.compte(Input::FaceDown) != 1)
            std::fprintf(stderr, "retournement : %zu gestes\n", tr.gestes.size());
        assert(tr.gestes.size() == 1 && tr.compte(Input::FaceDown) == 1);
    }
    {
        Trace tr = rejouer("manipulation.txt", 1.0f);
        if (!tr.gestes.empty())
            std::fprintf(stderr, "manipulation : %zu gestes\n", tr.gestes.size());
        assert(tr.gestes.empty());
    }
}

} // namespace

int main()
{
    retournement_puis_retour();
    face_up_apres_500_ms();
    pose_a_l_envers_bref_rien();
    secousse_quatre_coups();
    lancer_un_pic_avec_rotation();
    lancer_puis_rattrapage_un_seul_geste();
    pause_apres_geste();
    manipulation_normale_rien();
    seuils_regles();
    journal_une_ligne_par_salve();
    journal_faces();
    traces_reelles();
    return 0;
}
