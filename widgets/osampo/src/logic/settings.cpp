#include "logic/settings.h"

#include <cmath>

namespace pup {

namespace {

// Valeur sentinelle : une cle presente mais d un autre type rend le repli.
constexpr float SENTINELLE = -1.0e9f;

enum class Lecture { Absent, Invalide, Ok };

Lecture lit_entier(const JsonLine &l, const char *cle, int &v)
{
    if (!l.has(cle)) return Lecture::Absent;
    float f = l.number(cle, SENTINELLE);
    if (f == SENTINELLE || f != std::floor(f) || f < -1.0e8f || f > 1.0e8f)
        return Lecture::Invalide;
    v = static_cast<int>(f);
    return Lecture::Ok;
}

struct Borne {
    const char *cle;
    int Settings::*champ;
    int min, max;
};

const Borne BORNES[] = {
    {"report_min", &Settings::report_min, 1, 120},
    {"rattrapage_min", &Settings::rattrapage_min, 1, 720},
    {"calme_validation_min", &Settings::calme_validation_min, 1, 120},
    {"patte_avant_aboie_min", &Settings::patte_avant_aboie_min, 1, 60},
    {"calme_debut", &Settings::calme_debut, 0, 1439},
    {"calme_fin", &Settings::calme_fin, 0, 1439},
    {"nuit_debut", &Settings::nuit_debut, 0, 1439},
    {"nuit_fin", &Settings::nuit_fin, 0, 1439},
    {"lum_jour", &Settings::lum_jour, 5, 100},
    {"lum_nuit", &Settings::lum_nuit, 5, 100},
    {"baisse_jour", &Settings::baisse_jour, 0, 100},
    {"gain", &Settings::gain, 0, 100},
    {"lancer_mg", &Settings::lancer_mg, 500, 16000},
    {"lancer_dps", &Settings::lancer_dps, 50, 4000},
    {"lancer_ms", &Settings::lancer_ms, 10, 1000},
    {"secousse_mg", &Settings::secousse_mg, 500, 16000},
    {"secousse_coups", &Settings::secousse_coups, 2, 20},
    {"secousse_fenetre_ms", &Settings::secousse_fenetre_ms, 200, 5000},
    {"retourne_ms", &Settings::retourne_ms, 200, 10000},
    {"pause_gestes_ms", &Settings::pause_gestes_ms, 0, 10000},
    {"journal", &Settings::journal, 0, 2},
    {"courte_min", &Settings::courte_min, 1, 120},
    {"seuil_reclame", &Settings::seuil_reclame, 0, 100},
    {"retour_min", &Settings::retour_min, 1, 60},
};

// Ordre du fil pour les cles entieres, relances et touches intercalees a part.
void ajoute_entier(std::string &s, const char *cle, int v)
{
    s += ",\"";
    s += cle;
    s += "\":";
    s += std::to_string(v);
}

void ajoute_touche(std::string &s, const char *cle, char c)
{
    s += ",\"";
    s += cle;
    s += "\":\"";
    if (c == '"' || c == '\\') s += '\\';
    s += c;
    s += '"';
}

int valeur(const Settings &s, const char *cle)
{
    for (const Borne &b : BORNES)
        if (std::string(b.cle) == cle) return s.*(b.champ);
    return 0;
}

} // namespace

std::vector<std::string> Settings::apply(const JsonLine &ligne)
{
    std::vector<std::string> erreurs;

    for (const Borne &b : BORNES) {
        int v = 0;
        Lecture r = lit_entier(ligne, b.cle, v);
        if (r == Lecture::Absent) continue;
        if (r == Lecture::Invalide) {
            erreurs.push_back(std::string(b.cle) + ": entier attendu");
        } else if (v < b.min || v > b.max) {
            erreurs.push_back(std::string(b.cle) + ": hors bornes " + std::to_string(b.min) +
                              ".." + std::to_string(b.max));
        } else {
            this->*(b.champ) = v;
        }
    }

    // Relances : bloc. 0 = absente pour relance2_s / relance3_s.
    {
        static const char *CLES[3] = {"relance1_s", "relance2_s", "relance3_s"};
        int eff[3];
        for (int i = 0; i < 3; ++i) eff[i] = i < nb_relances ? relances_s[i] : 0;
        bool present = false, mauvais_type = false;
        for (int i = 0; i < 3; ++i) {
            int v = 0;
            Lecture r = lit_entier(ligne, CLES[i], v);
            if (r == Lecture::Absent) continue;
            present = true;
            if (r == Lecture::Invalide) mauvais_type = true;
            else eff[i] = v;
        }
        if (present) {
            bool ok = !mauvais_type && eff[0] != 0;
            int n = 0;
            while (ok && n < 3 && eff[n] != 0) {
                if (eff[n] < 10 || eff[n] > 3600) ok = false;
                if (n > 0 && eff[n] <= eff[n - 1]) ok = false;
                ++n;
            }
            for (int i = n; ok && i < 3; ++i)
                if (eff[i] != 0) ok = false; // zero au milieu
            if (ok) {
                nb_relances = n;
                for (int i = 0; i < 3; ++i) relances_s[i] = i < n ? eff[i] : 0;
            } else {
                erreurs.push_back("relances: 1 a 3 valeurs de 10 a 3600, strictement croissantes");
            }
        }
    }

    // Touches : bloc, un caractere ASCII imprimable, trois distinctes.
    {
        static const char *CLES[3] = {"k_lancer", "k_secousse", "k_retourner"};
        char eff[3] = {k_lancer, k_secousse, k_retourner};
        bool present = false, ok = true;
        for (int i = 0; i < 3; ++i) {
            if (!ligne.has(CLES[i])) continue;
            present = true;
            std::string t = ligne.text(CLES[i], "\x01");
            if (t.size() != 1 || t[0] < 0x21 || t[0] > 0x7E) ok = false;
            else eff[i] = t[0];
        }
        if (present) {
            if (eff[0] == eff[1] || eff[0] == eff[2] || eff[1] == eff[2]) ok = false;
            if (ok) {
                k_lancer = eff[0];
                k_secousse = eff[1];
                k_retourner = eff[2];
            } else {
                erreurs.push_back("touches: trois caracteres imprimables distincts");
            }
        }
    }
    // Batterie faible : 0 (pas de repere) ou une tension plausible.
    {
        int v = 0;
        Lecture r = lit_entier(ligne, "batterie_faible_mv", v);
        if (r == Lecture::Ok && (v == 0 || (v >= 3000 && v <= 4200))) batterie_faible_mv = v;
        else if (r != Lecture::Absent) erreurs.push_back("batterie_faible_mv: 0 ou 3000..4200");
    }

    if (ligne.has("rappels")) {
        auto l = reminders_from_text(ligne.text("rappels", ""));
        if (l) rappels = *l;
        else erreurs.push_back("rappels: liste invalide");
    }
    return erreurs;
}

std::string Settings::to_line() const
{
    std::string s = "{\"cfg\":1";
    ajoute_entier(s, "report_min", report_min);
    ajoute_entier(s, "rattrapage_min", rattrapage_min);
    for (int i = 0; i < 3; ++i) {
        std::string cle = "relance" + std::to_string(i + 1) + "_s";
        ajoute_entier(s, cle.c_str(), i < nb_relances ? relances_s[i] : 0);
    }
    static const char *SUITE[] = {
        "calme_validation_min", "patte_avant_aboie_min", "calme_debut", "calme_fin", "nuit_debut", "nuit_fin",
        "lum_jour", "lum_nuit", "baisse_jour", "gain", "lancer_mg", "lancer_dps",
        "lancer_ms", "secousse_mg", "secousse_coups", "secousse_fenetre_ms",
        "retourne_ms", "pause_gestes_ms", "journal", "courte_min", "seuil_reclame", "retour_min"};
    for (const char *cle : SUITE) ajoute_entier(s, cle, valeur(*this, cle));
    ajoute_entier(s, "batterie_faible_mv", batterie_faible_mv);
    ajoute_touche(s, "k_lancer", k_lancer);
    ajoute_touche(s, "k_secousse", k_secousse);
    ajoute_touche(s, "k_retourner", k_retourner);
    s += ",\"rappels\":\"" + reminders_to_text(rappels) + '"';
    s += '}';
    return s;
}

bool in_range(int minute, int debut, int fin)
{
    if (debut == fin) return false;
    if (debut < fin) return minute >= debut && minute < fin;
    return minute >= debut || minute < fin;
}

} // namespace pup
