#pragma once
#include <array>
#include <cstdint>
#include <string>
#include <vector>

// Allure du maitre pendant la promenade (SP5k), mesuree par l'IMU de la board,
// tenue en main ou dans la poche. Logique pure, ~100 Hz.
namespace pup {

enum class Allure { Arret, Pas, Trot, Course };
const char *allure_nom(Allure a); // "arret", "pas", "trot", "course"

// Bilan d'une promenade (depuis remettre_bilan) : pas, temps passe a chaque allure,
// distance estimee (somme de la vitesse estimee). Sert a recaler les seuils sans cable.
struct Bilan {
    int64_t pas = 0;
    int64_t ms[4] = {0, 0, 0, 0}; // par Allure
    float metres = 0;
    // "pas=1234 marche=12:30 trot=1:05 course=0:20 arret=3:10 distance=1.42km"
    std::string texte() const;
};

// Regles (calees le 2026-11-23 sur une marche reelle, board en main) :
// - Fenetre glissante de kFenetreMs, evaluee toutes les kPasEvalMs. Chaque axe
//   (acceleration x, y, z, rotation x, y, z) est centre sur sa moyenne : pas de
//   dependance a l'orientation.
// - Energie : ecart-type de l'acceleration (g, norme des 3 axes) et de la
//   rotation (deg/s).
// - Foulee (deux pas) : maximum, entre kFouleeMinMs et kFouleeMaxMs, de
//   l'autocorrelation des 6 axes, chaque axe pese par sa variance (le
//   balancement du bras ou du corps, au rythme de la foulee, porte le signal ;
//   compter des pics sur la norme de l'acceleration ratait les pas, board en main).
// - Allure brute : calme (acceleration < kCalmeG et rotation < kCalmeDps) ->
//   Arret ; acceleration >= kCourseG et rotation < kCourseMaxDps -> Course (en main, la course est peu
//   reguliere mais secoue fort) ; foulee nette (correlation >= kRythmeMin) ->
//   Pas, ou Trot si cadence >= kCadenceTrot ; sinon mouvement sans rythme
//   net : l'allure en cours si on marche ou trotte deja (le rythme faiblit souvent un
//   instant en marchant), Pas si la rotation depasse kBougeDps, sinon Arret
//   (on manipule la board sur place).
// - Changement d'allure : l'allure brute doit tenir kTenirMs (kTenirCourseMs
//   vers Course).
// - Cadence (pas/s) = 2 / foulee ; en course sans foulee nette : kCadenceCourse.
// - Vitesse (estimation, pas de GPS) : cadence x longueur du pas ; longueur =
//   0,45 + 0,13 x cadence m, x 1,3 en course.
//
// ponytail: une seule capture reelle (main, 2026-11-23) ; poche et vraie course
// dehors non mesurees. Le bilan de promenade sert a recaler.
class Podometre {
public:
    static constexpr int kFenetreMs = 4000;
    static constexpr int kPasEvalMs = 500;
    static constexpr int kFouleeMinMs = 750; // en dessous : un seul pas (cadence doublee)
    static constexpr int kFouleeMaxMs = 1600;
    static constexpr float kCalmeG = 0.06f;
    static constexpr float kCalmeDps = 40.0f;
    static constexpr float kCourseG = 0.42f;
    static constexpr float kRythmeMin = 0.35f;
    static constexpr float kCadenceTrot = 2.3f;
    static constexpr float kCadenceCourse = 2.8f;
    static constexpr int64_t kTenirMs = 1500;
    static constexpr int64_t kTenirCourseMs = 2500; // prendre la board en main secoue fort
    static constexpr float kBougeDps = 80.0f;
    static constexpr float kCourseMaxDps = 400.0f; // au-dela : on prend la board en main (490 mesures), pas une course (300 au plus)

    void feed(int64_t t_ms, float ax, float ay, float az, float gx, float gy, float gz);

    Allure allure() const { return allure_; }
    float cadence() const;     // pas par seconde, 0 a l'arret
    float vitesse_kmh() const; // 0 a l'arret
    // Dernieres mesures de la fenetre (journal, tests).
    float energie_g() const { return acc_sd_; }
    float energie_dps() const { return gyr_sd_; }
    float rythme() const { return rythme_; }
    int foulee_ms() const { return foulee_ms_; }

    // Duree d'une image de la marche du chiot : un cycle de ses pattes par
    // foulee du maitre (deux pas), sur `images` images, bornee a 70..200 ms ;
    // 150 ms a l'arret. Arrondie a 10 ms (moins de mises a jour de la vue).
    int duree_image_ms(int images) const;

    Bilan bilan() const { return bilan_; }
    void remettre_bilan() { bilan_ = Bilan{}; pas_frac_ = 0; } // debut d'une promenade

    // Lignes de journal (changement d'allure), videes a la lecture :
    // "allure pas cadence=1.90 foulee=1050 rythme=0.72 acc=0.18 gyro=175 vitesse=4.6"
    std::vector<std::string> take_journal();

private:
    struct Ech {
        int64_t t;
        float v[6];
    };
    void evaluer(int64_t t);
    Allure brute() const;

    std::vector<Ech> ech_; // anneau de la fenetre
    size_t tete_ = 0, n_ = 0;
    int64_t t_ = -1, derniere_eval_ = -1;

    float acc_sd_ = 0, gyr_sd_ = 0, rythme_ = 0;
    int foulee_ms_ = 0;

    Allure allure_ = Allure::Arret;
    Allure candidate_ = Allure::Arret;
    int64_t candidate_depuis_ = 0;
    Bilan bilan_;
    float pas_frac_ = 0;
    std::vector<std::string> journal_;
};

} // namespace pup
