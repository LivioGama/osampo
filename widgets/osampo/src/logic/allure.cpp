#include "logic/allure.h"

#include <algorithm>
#include <cmath>
#include <cstdio>

namespace pup {

namespace {

constexpr size_t kCapacite = 512;  // > kFenetreMs a 112 Hz
constexpr int64_t kTrouMs = 200;   // trou dans les echantillons : fenetre videe

} // namespace

const char *allure_nom(Allure a)
{
    switch (a) {
    case Allure::Arret: return "arret";
    case Allure::Pas: return "pas";
    case Allure::Trot: return "trot";
    case Allure::Course: return "course";
    }
    return "arret";
}

std::string Bilan::texte() const
{
    auto mmss = [](int64_t ms) {
        const int64_t s = ms / 1000;
        char b[24];
        std::snprintf(b, sizeof b, "%lld:%02lld", (long long)(s / 60), (long long)(s % 60));
        return std::string(b);
    };
    char l[160];
    std::snprintf(l, sizeof l, "pas=%lld marche=%s trot=%s course=%s arret=%s distance=%.2fkm", (long long)pas,
                  mmss(ms[int(Allure::Pas)]).c_str(), mmss(ms[int(Allure::Trot)]).c_str(),
                  mmss(ms[int(Allure::Course)]).c_str(), mmss(ms[int(Allure::Arret)]).c_str(), metres / 1000.0f);
    return l;
}

float Podometre::cadence() const
{
    switch (allure_) {
    case Allure::Arret: return 0.0f;
    case Allure::Course:
        // course reguliere : sa foulee ; sinon une cadence de course typique
        return rythme_ >= kRythmeMin && foulee_ms_ > 0 ? std::max(2000.0f / foulee_ms_, kCadenceTrot)
                                                        : kCadenceCourse;
    default: return foulee_ms_ > 0 ? 2000.0f / foulee_ms_ : 0.0f;
    }
}

float Podometre::vitesse_kmh() const
{
    const float c = cadence();
    if (c <= 0)
        return 0.0f;
    float longueur = 0.45f + 0.13f * c;
    if (allure_ == Allure::Course)
        longueur *= 1.3f;
    return c * longueur * 3.6f;
}

int Podometre::duree_image_ms(int images) const
{
    const float c = cadence();
    if (c <= 0 || images <= 0)
        return 150;
    const float d = std::clamp(2000.0f / c / images, 70.0f, 200.0f);
    return int(std::lround(d / 10.0f)) * 10;
}

std::vector<std::string> Podometre::take_journal()
{
    std::vector<std::string> out;
    out.swap(journal_);
    return out;
}

Allure Podometre::brute() const
{
    if (acc_sd_ < kCalmeG && gyr_sd_ < kCalmeDps)
        return Allure::Arret;
    if (acc_sd_ >= kCourseG && gyr_sd_ < kCourseMaxDps)
        return Allure::Course;
    if (rythme_ >= kRythmeMin && foulee_ms_ > 0)
        return 2000.0f / foulee_ms_ >= kCadenceTrot ? Allure::Trot : Allure::Pas;
    // mouvement sans rythme net (en sortant de la course : retour au pas)
    if (allure_ == Allure::Pas || allure_ == Allure::Trot)
        return allure_;
    return gyr_sd_ >= kBougeDps ? Allure::Pas : Allure::Arret;
}

void Podometre::evaluer(int64_t t)
{
    const size_t n = n_;
    const auto at = [&](size_t i) -> const Ech & { // i = 0 : le plus ancien
        return ech_[(tete_ + kCapacite - n + i) % kCapacite];
    };
    const float dt = float(at(n - 1).t - at(0).t) / float(n - 1); // ms par echantillon
    if (dt <= 0)
        return;

    // Centrage et variance par axe.
    float moy[6] = {}, var[6] = {};
    for (size_t i = 0; i < n; ++i)
        for (int j = 0; j < 6; ++j)
            moy[j] += at(i).v[j];
    for (float &m : moy)
        m /= float(n);
    for (size_t i = 0; i < n; ++i)
        for (int j = 0; j < 6; ++j) {
            const float d = at(i).v[j] - moy[j];
            var[j] += d * d;
        }
    for (float &v : var)
        v /= float(n);
    acc_sd_ = std::sqrt(var[0] + var[1] + var[2]);
    gyr_sd_ = std::sqrt(var[3] + var[4] + var[5]);
    // Les deux familles n'ont pas la meme unite : chaque axe est normalise, puis
    // pese par sa part de variance dans sa famille.
    float poids[6], poids_tot = 0;
    const float va = var[0] + var[1] + var[2], vg = var[3] + var[4] + var[5];
    for (int j = 0; j < 6; ++j) {
        const float fam = j < 3 ? va : vg;
        poids[j] = fam > 0 && var[j] > 0 ? var[j] / fam : 0;
        poids_tot += poids[j];
    }

    // Autocorrelation ponderee sur la plage des foulees.
    const int lo = std::max(1, int(std::ceil(kFouleeMinMs / dt)));
    const int hi = std::min(int(n) - 1, int(kFouleeMaxMs / dt));
    float meilleur = -2;
    int lag_max = 0;
    for (int L = lo; L <= hi; ++L) {
        float r = 0;
        for (int j = 0; j < 6; ++j) {
            if (poids[j] <= 0)
                continue;
            float s = 0;
            for (size_t i = 0; i + L < n; ++i)
                s += (at(i).v[j] - moy[j]) * (at(i + L).v[j] - moy[j]);
            r += poids[j] * (s / float(n - L)) / var[j];
        }
        r = poids_tot > 0 ? r / poids_tot : 0;
        if (r > meilleur) {
            meilleur = r;
            lag_max = L;
        }
    }
    rythme_ = std::max(0.0f, meilleur);
    foulee_ms_ = lag_max > 0 ? int(std::lround(lag_max * dt)) : 0;

    const Allure avant = allure_;
    const Allure b = brute();
    if (b != candidate_) {
        candidate_ = b;
        candidate_depuis_ = t;
    }
    if (b != allure_ && t - candidate_depuis_ >= (b == Allure::Course ? kTenirCourseMs : kTenirMs))
        allure_ = b;
    if (allure_ != avant) {
        char l[128];
        std::snprintf(l, sizeof l, "allure %s cadence=%.2f foulee=%d rythme=%.2f acc=%.2f gyro=%.0f vitesse=%.1f",
                      allure_nom(allure_), cadence(), foulee_ms_, rythme_, acc_sd_, gyr_sd_, vitesse_kmh());
        journal_.emplace_back(l);
    }
}

void Podometre::feed(int64_t t_ms, float ax, float ay, float az, float gx, float gy, float gz)
{
    if (ech_.empty())
        ech_.resize(kCapacite);
    if (t_ >= 0 && t_ms - t_ > kTrouMs) {
        n_ = 0; // trou : on repart d'une fenetre vide (l'allure reste)
        derniere_eval_ = -1;
    }
    // Bilan : l'allure en cours jusqu'a cet echantillon.
    if (t_ >= 0) {
        const int64_t d = std::clamp<int64_t>(t_ms - t_, 0, 50);
        bilan_.ms[int(allure_)] += d;
        bilan_.metres += vitesse_kmh() / 3.6f * d / 1000.0f;
        pas_frac_ += cadence() * d / 1000.0f;
        const int64_t entiers = int64_t(pas_frac_);
        bilan_.pas += entiers;
        pas_frac_ -= float(entiers);
    }
    t_ = t_ms;

    ech_[tete_] = Ech{t_ms, {ax, ay, az, gx, gy, gz}};
    tete_ = (tete_ + 1) % kCapacite;
    n_ = std::min(n_ + 1, kCapacite);
    // Fenetre : on oublie les echantillons plus vieux que kFenetreMs.
    while (n_ > 1 && t_ms - ech_[(tete_ + kCapacite - n_) % kCapacite].t > kFenetreMs)
        --n_;

    const auto &plus_vieux = ech_[(tete_ + kCapacite - n_) % kCapacite];
    if (t_ms - plus_vieux.t < kFenetreMs - 100)
        return; // fenetre pas encore pleine
    if (derniere_eval_ >= 0 && t_ms - derniere_eval_ < kPasEvalMs)
        return;
    derniere_eval_ = t_ms;
    evaluer(t_ms);
}

} // namespace pup
