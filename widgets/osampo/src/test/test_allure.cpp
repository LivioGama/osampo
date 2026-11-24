// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <map>
#include <sstream>
#include <string>

#include "logic/allure.h"

using pup::Allure;
using pup::Podometre;

namespace {

constexpr int64_t kPeriodeMs = 10; // ~100 Hz, comme la capture reelle

// Bruit deterministe (+-amp), pour ne pas dependre de rand().
float bruit(uint32_t &etat, float amp)
{
    etat = etat * 1664525u + 1013904223u;
    return amp * ((etat >> 8) / float(1u << 24) * 2.0f - 1.0f);
}

struct Banc {
    Podometre p;
    int64_t t = 0;
    uint32_t graine = 1;
    float phase = 0;

    // Marche simulee comme la capture : le bras (et la board) se balance a la
    // foulee (gyro_dps de rotation), l'acceleration bat au pas (acc_g), plus du bruit.
    void marcher(int64_t ms, float foulee_ms, float acc_g, float gyro_dps, float bruit_g = 0.03f)
    {
        for (int64_t fin = t + ms; t < fin; t += kPeriodeMs) {
            phase += kPeriodeMs / foulee_ms;
            const float w = 2.0f * float(M_PI) * phase;
            p.feed(t, bruit(graine, bruit_g), 0.3f + bruit(graine, bruit_g),
                   0.95f + acc_g * std::sin(2 * w) + bruit(graine, bruit_g), gyro_dps * std::sin(w) + bruit(graine, 10),
                   0.4f * gyro_dps * std::cos(w) + bruit(graine, 10), bruit(graine, 10));
        }
    }
    // Mouvement sans rythme (bruit seulement).
    void agiter(int64_t ms, float acc_g, float gyro_dps)
    {
        for (int64_t fin = t + ms; t < fin; t += kPeriodeMs)
            p.feed(t, bruit(graine, acc_g), bruit(graine, acc_g), 1.0f + bruit(graine, acc_g),
                   bruit(graine, gyro_dps), bruit(graine, gyro_dps), bruit(graine, gyro_dps));
    }
    void immobile(int64_t ms) { agiter(ms, 0.005f, 0.5f); }
};

bool proche(float a, float b, float tol) { return std::fabs(a - b) <= tol; }

// Rejoue une trace (format "S t ax ay az gx gy gz") ; rend l'allure a chaque
// seconde ecoulee depuis le premier echantillon.
std::map<int, Allure> rejouer(const char *nom)
{
    std::string chemin = std::string(TRACES_DIR) + "/" + nom;
    std::ifstream f(chemin);
    if (!f) {
        std::fprintf(stderr, "trace introuvable : %s\n", chemin.c_str());
        std::abort();
    }
    Podometre p;
    std::map<int, Allure> par_s;
    std::string ligne;
    long long t0 = -1;
    while (std::getline(f, ligne)) {
        auto debut = ligne.find_first_not_of(" \t");
        if (debut == std::string::npos || ligne[debut] != 'S')
            continue;
        std::istringstream is(ligne);
        std::string tag;
        long long t;
        float v[6];
        is >> tag >> t;
        for (float &x : v)
            is >> x;
        if (t0 < 0)
            t0 = t;
        p.feed(t, v[0], v[1], v[2], v[3], v[4], v[5]);
        par_s[int((t - t0) / 1000)] = p.allure();
    }
    return par_s;
}

} // namespace

int main()
{
    // Immobile : arret, image a 150 ms.
    {
        Banc b;
        b.immobile(10000);
        assert(b.p.allure() == Allure::Arret);
        assert(b.p.cadence() == 0 && b.p.vitesse_kmh() == 0);
        assert(b.p.duree_image_ms(8) == 150);
    }
    // Marche (foulee 1,05 s = 1,9 pas/s) : pas, ~4,8 km/h, une image ~130 ms.
    {
        Banc b;
        b.marcher(12000, 1050, 0.15f, 180);
        assert(b.p.allure() == Allure::Pas);
        assert(proche(b.p.cadence(), 1.9f, 0.1f));
        assert(b.p.vitesse_kmh() > 4.0f && b.p.vitesse_kmh() < 5.5f);
        assert(b.p.duree_image_ms(8) == 130);
        // la fenetre (4 s) et la tenue (1,5 s) d'abord
        Banc c;
        c.marcher(4500, 1050, 0.15f, 180);
        assert(c.p.allure() == Allure::Arret);
    }
    // Marche rapide (foulee 0,83 s = 2,4 pas/s) : trot.
    {
        Banc b;
        b.marcher(12000, 830, 0.18f, 150);
        assert(b.p.allure() == Allure::Trot);
    }
    // Course en main : forte acceleration, peu reguliere.
    {
        Banc b;
        b.agiter(12000, 0.6f, 120);
        assert(b.p.allure() == Allure::Course);
        assert(b.p.cadence() == Podometre::kCadenceCourse && b.p.vitesse_kmh() > 9.0f);
        assert(b.p.duree_image_ms(6) == 120);
    }
    // Prendre la board en main (acceleration et rotation tres fortes) : pas une course.
    {
        Banc b;
        b.agiter(5000, 0.6f, 500);
        assert(b.p.allure() != Allure::Course);
    }
    // Board manipulee sur place (petites rotations sans rythme) : arret.
    {
        Banc b;
        b.agiter(10000, 0.06f, 30);
        assert(b.p.allure() == Allure::Arret);
    }
    // Enchainement : marche, un trou de rythme (on garde la marche), course, arret.
    {
        Banc b;
        b.marcher(10000, 1050, 0.15f, 180);
        assert(b.p.allure() == Allure::Pas);
        b.agiter(3000, 0.1f, 120); // rythme brouille un instant
        assert(b.p.allure() == Allure::Pas);
        b.agiter(2000, 0.6f, 150);
        assert(b.p.allure() == Allure::Pas); // course pas encore tenue
        b.agiter(5000, 0.6f, 150);
        assert(b.p.allure() == Allure::Course);
        b.immobile(5000);
        assert(b.p.allure() == Allure::Arret);
        const auto j = b.p.take_journal();
        assert(j.size() == 3);
        assert(j[0].rfind("allure pas cadence=", 0) == 0);
        assert(j[1].rfind("allure course ", 0) == 0);
        assert(j[2].rfind("allure arret ", 0) == 0);
        assert(b.p.take_journal().empty());
    }
    // Sortie de course sans rythme net : retour au pas, pas de course figee.
    {
        Banc b;
        b.agiter(8000, 0.6f, 150);
        assert(b.p.allure() == Allure::Course);
        b.agiter(6000, 0.15f, 120);
        assert(b.p.allure() == Allure::Pas);
    }
    // Bilan d'une promenade : pas, temps par allure, distance.
    {
        Banc b;
        b.immobile(5000);
        b.p.remettre_bilan(); // depart de la promenade
        b.marcher(60000, 1050, 0.15f, 180);
        b.immobile(10000);
        const pup::Bilan bl = b.p.bilan();
        // ~62 s de marche reconnue a 1,9 pas/s (la fenetre de 4 s retarde depart et arret)
        assert(bl.pas >= 105 && bl.pas <= 125);
        const int64_t total = bl.ms[0] + bl.ms[1] + bl.ms[2] + bl.ms[3];
        assert(total >= 69000 && total <= 71000);
        assert(bl.ms[int(Allure::Pas)] > 50000);
        assert(bl.metres > 60 && bl.metres < 90);
        assert(bl.texte().rfind("pas=", 0) == 0 && bl.texte().find(" distance=0.") != std::string::npos);
        b.p.remettre_bilan();
        assert(b.p.bilan().pas == 0 && b.p.bilan().metres == 0);
        pup::Bilan v;
        v.pas = 1234;
        v.ms[1] = 750000;
        v.ms[2] = 65000;
        v.metres = 1420;
        assert(v.texte() == "pas=1234 marche=12:30 trot=1:05 course=0:00 arret=0:00 distance=1.42km");
    }
    // Trace reelle, board en main : immobile, marche, marche rapide, course, posee.
    {
        auto a = rejouer("marche_main_1.txt");
        for (int s : {2, 5, 8})
            assert(a[s] == Allure::Arret);
        for (int s = 0; s < 16; ++s)
            assert(a[s] != Allure::Course); // la prise en main n'est pas une course
        for (int s = 18; s <= 30; ++s)
            assert(a[s] == Allure::Pas);
        for (int s = 33; s <= 45; ++s)
            assert(a[s] == Allure::Trot);
        for (int s = 49; s <= 62; ++s)
            assert(a[s] == Allure::Course);
        for (int s = 66; s <= 71; ++s)
            assert(a[s] == Allure::Arret);
    }
    // Traces des gestes (salves courtes, trous entre elles) : jamais de course.
    for (const char *nom : {"manipulation.txt", "secousse_1.txt", "secousse_2.txt", "lancer_1.txt",
                            "lancer_2.txt", "retournement.txt", "lancer_droite_1.txt", "lancer_gauche_1.txt"}) {
        for (const auto &[s, al] : rejouer(nom)) {
            if (al == Allure::Course)
                std::fprintf(stderr, "%s : course a %d s\n", nom, s);
            assert(al != Allure::Course);
        }
    }
    std::puts("test_allure OK");
    return 0;
}
