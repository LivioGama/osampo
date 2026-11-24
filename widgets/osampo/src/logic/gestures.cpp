#include "logic/gestures.h"

#include <cmath>
#include <cstdio>

namespace pup {

namespace {

constexpr float kSalveEcartG = 0.5f;     // |norme - 1 g| au-dela : agite
constexpr float kSalveDps = 100.0f;      // gyro au-dela : agite
constexpr int64_t kCalmeFinMs = 300;     // calme qui termine une salve
constexpr float kImmobileEcartG = 0.15f; // quasi immobile : |norme - 1 g| <= 0,15
constexpr float kImmobileDps = 30.0f;    // et gyro < 30 deg/s
constexpr float kFaceG = 0.8f;           // az <= -0,8 (bas) ou >= +0,8 (haut)
constexpr int64_t kFaceHautMs = 500;     // retour a l'endroit tenu 500 ms
constexpr float kRearmeG = 0.3f;         // hysteresis de rearmement d'un pic

} // namespace

void GestureDetector::set_settings(const Settings &s)
{
    reglages_ = s;
    if (reglages_.journal >= 2) {
        echantillons_.reserve(kMaxSalve);
        face_echantillons_.reserve(kMaxSalve);
    }
}

std::optional<Input> GestureDetector::feed(const Sample &s)
{
    float norme = std::sqrt(s.ax * s.ax + s.ay * s.ay + s.az * s.az);
    float gyro = std::sqrt(s.gx * s.gx + s.gy * s.gy + s.gz * s.gz);
    std::optional<Input> geste = feed_salve(s, norme, gyro);
    // Les compteurs de face avancent toujours ; si un geste sort deja sur cet
    // echantillon, la transition de face attend l'echantillon suivant (la
    // condition, toujours tenue, la rendra alors).
    std::optional<Input> face = feed_face(s, norme, gyro);
    if (geste)
        return geste;
    if (face) {
        face_bas_ = *face == Input::FaceDown;
        if (reglages_.journal >= 1)
            journal_face(s, face_bas_);
        face_echantillons_.clear();
        face_perdus_ = 0;
        return face;
    }
    return std::nullopt;
}

std::optional<Input> GestureDetector::feed_face(const Sample &s, float norme, float gyro)
{
    bool immobile = std::fabs(norme - 1.0f) <= kImmobileEcartG && gyro < kImmobileDps;
    bool bas = s.az <= -kFaceG && immobile;
    bool haut = s.az >= kFaceG;

    bas_depuis_ = bas ? (bas_depuis_ < 0 ? s.t_ms : bas_depuis_) : -1;
    if (bas && !face_bas_) {
        if (reglages_.journal >= 2) {
            if (face_echantillons_.size() < kMaxSalve)
                face_echantillons_.push_back(s);
            else
                ++face_perdus_;
        }
    } else if (!face_echantillons_.empty() || face_perdus_) {
        face_echantillons_.clear(); // candidat abandonne : rien
        face_perdus_ = 0;
    }
    haut_depuis_ = haut ? (haut_depuis_ < 0 ? s.t_ms : haut_depuis_) : -1;

    if (!face_bas_ && bas_depuis_ >= 0 && s.t_ms - bas_depuis_ >= reglages_.retourne_ms)
        return Input::FaceDown;
    if (face_bas_ && haut_depuis_ >= 0 && s.t_ms - haut_depuis_ >= kFaceHautMs)
        return Input::FaceUp;
    return std::nullopt;
}

std::optional<Input> GestureDetector::feed_salve(const Sample &s, float norme, float gyro)
{
    const int64_t t = s.t_ms;
    const float secousse_g = reglages_.secousse_mg / 1000.0f;
    const float lancer_g = reglages_.lancer_mg / 1000.0f;
    const bool agite = std::fabs(norme - 1.0f) > kSalveEcartG || gyro > kSalveDps;

    if (!en_salve_) {
        if (!agite)
            return std::nullopt;
        en_salve_ = true;
        debut_salve_ = t;
        pic_g_ = 0;
        ay_pic_ = 0;
        pic_dps_ = 0;
        coups_ = 0;
        pic_arme_ = true;
        salve_close_ = false;
        decision_ = Decision::Rien;
        nb_pics_ = 0;
        fort_depuis_ = -1;
        fort_compte_ = false;
        candidat_ = -1;
        echantillons_.clear();
        perdus_ = 0;
        calme_n_ = 0;
    }

    if (reglages_.journal >= 2) {
        if (echantillons_.size() < kMaxSalve)
            echantillons_.push_back(s);
        else
            ++perdus_;
    }
    dernier_ = s;
    if (!agite) {
        calme_somme_[0] += s.ax;
        calme_somme_[1] += s.ay;
        calme_somme_[2] += s.az;
        ++calme_n_;
    }
    if (agite) {
        calme_somme_[0] = calme_somme_[1] = calme_somme_[2] = 0;
        calme_n_ = 0;
        dernier_agite_ = t;
        if (norme > pic_g_) {
            pic_g_ = norme;
            if (!salve_close_)
                ay_pic_ = s.ay;
        }
        if (gyro > pic_dps_)
            pic_dps_ = gyro;
    }

    const bool en_pause = t < pause_jusqu_;
    const bool actif = !salve_close_ && !en_pause; // pics pris en compte
    std::optional<Input> geste;

    // Pics de secousse.
    if (pic_arme_ && norme >= secousse_g) {
        pic_arme_ = false;
        ++coups_;
        if (actif) {
            pics_[tete_pics_] = t;
            tete_pics_ = (tete_pics_ + 1) % kMaxCoups;
            if (nb_pics_ < kMaxCoups)
                ++nb_pics_;
            int n = reglages_.secousse_coups;
            if (n < 1)
                n = 1;
            if (n > kMaxCoups)
                n = kMaxCoups;
            if (nb_pics_ >= n) {
                // n-ieme pic le plus recent (le plus recent est lui-meme).
                int64_t ancien = pics_[(tete_pics_ - n + kMaxCoups) % kMaxCoups];
                if (t - ancien <= reglages_.secousse_fenetre_ms)
                    geste = Input::Shake;
            }
        }
    } else if (!pic_arme_ && norme < secousse_g - kRearmeG) {
        pic_arme_ = true;
    }

    // Episode fort (candidat lancer).
    if (norme >= lancer_g && gyro >= reglages_.lancer_dps) {
        if (fort_depuis_ < 0) {
            fort_depuis_ = t;
            fort_compte_ = false;
        }
        if (!fort_compte_ && t - fort_depuis_ >= reglages_.lancer_ms) {
            fort_compte_ = true;
            if (actif && !geste && candidat_ < 0)
                candidat_ = t;
        }
    } else {
        fort_depuis_ = -1;
    }

    if (geste) {
        decision_ = Decision::Secousse;
        candidat_ = -1;
    } else if (candidat_ >= 0 && t - candidat_ >= reglages_.secousse_fenetre_ms) {
        geste = Input::Throw;
        lancer_droite_ = ay_pic_ > 0; // inverse le 2026-11-21 (l'auteur : le chiot partait de l autre cote)
        decision_ = Decision::Lancer;
        candidat_ = -1;
    }
    if (geste) {
        salve_close_ = true;
        nb_pics_ = 0;
        pause_jusqu_ = t + reglages_.pause_gestes_ms;
    }

    if (!agite && t - dernier_agite_ >= kCalmeFinMs && candidat_ < 0)
        fin_salve();
    return geste;
}

void GestureDetector::fin_salve()
{
    en_salve_ = false;
    if (reglages_.journal < 1) {
        echantillons_.clear();
        return;
    }
    char ligne[160];
    if (reglages_.journal >= 2)
        journal_echantillons(echantillons_);
    echantillons_.clear();
    float g[3] = {dernier_.ax, dernier_.ay, dernier_.az};
    if (calme_n_ > 0)
        for (int i = 0; i < 3; ++i)
            g[i] = calme_somme_[i] / calme_n_;
    const char *dec = decision_ == Decision::Lancer     ? (lancer_droite_ ? "lancer sens=droite" : "lancer sens=gauche")
                      : decision_ == Decision::Secousse ? "secousse"
                                                        : "rien";
    std::snprintf(ligne, sizeof ligne,
                  "salve pic_mg=%ld pic_dps=%ld ms=%lld coups=%d -> %s grav=%.3f,%.3f,%.3f perdus=%d",
                  std::lround(pic_g_ * 1000.0f), std::lround(pic_dps_),
                  static_cast<long long>(dernier_agite_ - debut_salve_), coups_, dec, g[0], g[1], g[2],
                  reglages_.journal >= 2 ? perdus_ : 0);
    journal_.emplace_back(ligne);
    perdus_ = 0;
}

void GestureDetector::journal_echantillons(const std::vector<Sample> &v)
{
    char ligne[160];
    for (const Sample &e : v) {
        std::snprintf(ligne, sizeof ligne, "S %lld %.3f %.3f %.3f %.3f %.3f %.3f",
                      static_cast<long long>(e.t_ms), e.ax, e.ay, e.az, e.gx, e.gy, e.gz);
        journal_.emplace_back(ligne);
    }
}

void GestureDetector::journal_face(const Sample &s, bool bas)
{
    char ligne[96];
    if (bas && reglages_.journal >= 2) {
        journal_echantillons(face_echantillons_);
        std::snprintf(ligne, sizeof ligne, "face bas grav=%.3f,%.3f,%.3f perdus=%d", s.ax, s.ay, s.az,
                      face_perdus_);
    } else {
        std::snprintf(ligne, sizeof ligne, "face %s grav=%.3f,%.3f,%.3f", bas ? "bas" : "haut", s.ax, s.ay,
                      s.az);
    }
    journal_.emplace_back(ligne);
}

std::vector<std::string> GestureDetector::take_journal()
{
    std::vector<std::string> r;
    r.swap(journal_);
    return r;
}

} // namespace pup
