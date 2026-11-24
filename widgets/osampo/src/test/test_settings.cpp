// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>

#include "json_line.h"
#include "logic/settings.h"
#include "logic/rappel.h"
#include <cstdio>
#include <fstream>
#include <sstream>
#include <string>

#ifndef CFG_DEFAUT_JSON
#error "CFG_DEFAUT_JSON doit pointer vers cfg_defaut.json"
#endif

static std::string lit_fichier(const char *chemin)
{
    std::ifstream f(chemin);
    assert(f.good());
    std::stringstream ss;
    ss << f.rdbuf();
    std::string s = ss.str();
    if (!s.empty() && s.back() == '\n') s.pop_back();
    return s;
}

static std::vector<std::string> applique(pup::Settings &s, const std::string &ligne)
{
    JsonLine j;
    bool ok = j.parse(ligne.data(), ligne.size());
    assert(ok);
    return s.apply(j);
}

int main()
{
    {   // defauts_egaux_au_fichier
        pup::Settings s;
        std::string attendu = lit_fichier(CFG_DEFAUT_JSON);
        assert(attendu.find('\n') == std::string::npos);
        assert(s.to_line() == attendu);
    }
    {   // aller_retour
        pup::Settings a;
        a.report_min = 25;
        a.relances_s[0] = 30; a.relances_s[1] = 90; a.nb_relances = 2;
        a.calme_debut = 0; a.calme_fin = 1439;
        a.lum_nuit = 5; a.lancer_mg = 16000; a.journal = 2;
        a.k_lancer = '"'; a.k_secousse = '\\'; a.k_retourner = 'x';
        pup::Settings b;
        auto err = applique(b, a.to_line());
        assert(err.empty());
        assert(b.to_line() == a.to_line());
        assert(b.nb_relances == 2 && b.relances_s[1] == 90);
        assert(b.k_lancer == '"' && b.k_secousse == '\\');
    }
    {   // rappels_aller_retour
        using pup::Category;
        auto txt = pup::reminders_to_text(pup::default_reminders());
        assert(txt == "repas,1200,127;sortie,1230,127;sport,1800,21;repas,1930,127;dodo,2145,127");
        auto lu = pup::reminders_from_text(txt);
        assert(lu && lu->size() == 5);
        assert((*lu)[4].cat == Category::Dodo && (*lu)[4].minute == 21 * 60 + 45 && (*lu)[4].days == 127);
        assert(pup::reminders_to_text(*lu) == txt);
    }
    {   // rappels_invalides_refuses
        for (const char *t : {"repas,2500,127", "chat,1200,1", "", "repas,1200,0", "repas,1200,128",
                              "repas,120,127", "repas,1200,127;", "repas,1200"})
            assert(!pup::reminders_from_text(t));
        std::string dix_sept;
        for (int i = 0; i < 17; ++i) dix_sept += std::string(i ? ";" : "") + "repas,1200,127";
        assert(!pup::reminders_from_text(dix_sept));
        pup::Settings s;
        auto err = applique(s, "{\"cfg\":1,\"rappels\":\"chat,1200,1\"}");
        assert(err.size() == 1 && err[0].rfind("rappels:", 0) == 0);
        assert(pup::reminders_to_text(s.rappels) == pup::reminders_to_text(pup::default_reminders()));
        err = applique(s, "{\"cfg\":1,\"rappels\":\"sortie,1800,31\"}");
        assert(err.empty() && s.rappels.size() == 1 && s.rappels[0].cat == pup::Category::Sortie);
    }
    {   // promenade_et_batterie_bornees
        pup::Settings s;
        auto err = applique(s, "{\"cfg\":1,\"courte_min\":7,\"seuil_reclame\":40,\"retour_min\":3,\"batterie_faible_mv\":3500}");
        assert(err.empty() && s.courte_min == 7 && s.seuil_reclame == 40 && s.retour_min == 3 && s.batterie_faible_mv == 3500);
        err = applique(s, "{\"cfg\":1,\"seuil_reclame\":101,\"batterie_faible_mv\":2000}");
        assert(err.size() == 2 && s.seuil_reclame == 40 && s.batterie_faible_mv == 3500);
    }
    {   // patte_avant_aboie_min : defaut 2, bornes 1..60, place apres calme_validation_min
        pup::Settings s;
        assert(s.patte_avant_aboie_min == 2);
        const std::string l = s.to_line();
        assert(l.find("\"calme_validation_min\":10,\"patte_avant_aboie_min\":2,") != std::string::npos);
        auto err = applique(s, "{\"cfg\":1,\"patte_avant_aboie_min\":5}");
        assert(err.empty() && s.patte_avant_aboie_min == 5);
        err = applique(s, "{\"patte_avant_aboie_min\":1}");
        assert(err.empty() && s.patte_avant_aboie_min == 1);
        err = applique(s, "{\"patte_avant_aboie_min\":60}");
        assert(err.empty() && s.patte_avant_aboie_min == 60);
        err = applique(s, "{\"patte_avant_aboie_min\":0}");
        assert(err.size() == 1 && err[0] == "patte_avant_aboie_min: hors bornes 1..60" && s.patte_avant_aboie_min == 60);
        err = applique(s, "{\"patte_avant_aboie_min\":61}");
        assert(err.size() == 1 && s.patte_avant_aboie_min == 60);
        err = applique(s, "{\"patte_avant_aboie_min\":2.5}");
        assert(err.size() == 1 && err[0] == "patte_avant_aboie_min: entier attendu" && s.patte_avant_aboie_min == 60);
        err = applique(s, "{\"gain\":3}"); // cle absente : valeur gardee
        assert(err.empty() && s.patte_avant_aboie_min == 60);
    }
    {   // cle_hors_bornes_ignoree
        pup::Settings s;
        auto err = applique(s, "{\"cfg\":1,\"report_min\":0,\"gain\":30}");
        assert(s.report_min == 10);
        assert(s.gain == 30);
        assert(err.size() == 1 && err[0].find("report_min") != std::string::npos);
        // rattrapage_min 0 couperait tous les rappels : refuse.
        err = applique(s, "{\"cfg\":1,\"rattrapage_min\":0}");
        assert(s.rattrapage_min == 60);
        assert(err.size() == 1 && err[0].find("rattrapage_min") != std::string::npos);
        // mauvais type : invalide aussi
        err = applique(s, "{\"cfg\":1,\"lum_jour\":\"abc\"}");
        assert(s.lum_jour == 100);
        assert(err.size() == 1 && err[0].find("lum_jour") != std::string::npos);
    }
    {   // relances_non_croissantes_refusees
        pup::Settings s;
        auto err = applique(s, "{\"cfg\":1,\"relance1_s\":100,\"relance2_s\":100,\"relance3_s\":200,\"gain\":5}");
        assert(s.relances_s[0] == 60 && s.relances_s[1] == 180 && s.relances_s[2] == 420);
        assert(s.nb_relances == 3 && s.gain == 5);
        assert(err.size() == 1 && err[0].find("relances") != std::string::npos);
        // zero au milieu refuse
        err = applique(s, "{\"relance1_s\":60,\"relance2_s\":0,\"relance3_s\":200}");
        assert(err.size() == 1 && s.nb_relances == 3);
        // une seule relance valide
        err = applique(s, "{\"relance1_s\":45,\"relance2_s\":0,\"relance3_s\":0}");
        assert(err.empty() && s.nb_relances == 1 && s.relances_s[0] == 45);
        // hors bornes
        err = applique(s, "{\"relance1_s\":5}");
        assert(err.size() == 1 && s.relances_s[0] == 45);
    }
    {   // touches_identiques_refusees
        pup::Settings s;
        auto err = applique(s, "{\"k_lancer\":\"a\",\"k_secousse\":\"a\",\"k_retourner\":\"b\"}");
        assert(s.k_lancer == 't' && s.k_secousse == 's' && s.k_retourner == 'f');
        assert(err.size() == 1 && err[0].find("touches") != std::string::npos);
        err = applique(s, "{\"k_lancer\":\"ab\"}");
        assert(err.size() == 1 && s.k_lancer == 't');
        err = applique(s, "{\"k_lancer\":\" \"}");
        assert(err.size() == 1 && s.k_lancer == 't');
        err = applique(s, "{\"k_lancer\":\"s\"}"); // collision avec la valeur courante
        assert(err.size() == 1 && s.k_lancer == 't');
    }
    {   // cle_absente_inchangee
        pup::Settings s, ref;
        auto err = applique(s, "{\"cfg\":1}");
        assert(err.empty() && s.to_line() == ref.to_line());
    }
    {   // in_range
        assert(pup::in_range(1380, 1320, 420));
        assert(!pup::in_range(420, 1320, 420));
        assert(!pup::in_range(600, 600, 600));
        assert(pup::in_range(100, 1320, 420));
        assert(pup::in_range(600, 500, 700));
    }
    std::puts("test_settings ok");
    return 0;
}
