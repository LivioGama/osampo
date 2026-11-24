#pragma once
#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "logic/pup.h"
#include "logic/settings.h"

namespace pup {

// Echantillon de la centrale inertielle : accelerations en g, vitesses
// angulaires en deg/s, horodatage en ms (croissant).
struct Sample {
    int64_t t_ms;
    float ax, ay, az;
    float gx, gy, gz;
};

// Detecteur de gestes (spec G2-G6, G8), logique pure, ~100 Hz.
//
// Regles :
// - Orientation : ecran vers le haut = az ~ +1 g (supposition, Task 8).
//   "Quasi immobile" = |norme - 1 g| <= 0,15 g et gyro < 30 deg/s.
// - Face : az <= -0,8 g et quasi immobile, tenu sans interruption retourne_ms
//   -> FaceDown (une fois). Puis az >= +0,8 g tenu 500 ms -> FaceUp. Jamais de
//   FaceUp sans FaceDown prealable. Les faces ignorent les salves et la pause.
// - Salve : commence quand |norme - 1 g| > 0,5 g ou gyro > 100 deg/s, finit
//   apres 300 ms de calme (ni l'un ni l'autre) et une fois le lancer en attente
//   tranche.
// - Pic : la norme passe au-dessus de secousse_mg ; compte une fois, rearme
//   quand la norme repasse sous secousse_mg - 0,3 g.
// - Secousse : secousse_coups pics dans une fenetre glissante de
//   secousse_fenetre_ms -> Shake tout de suite.
// - Lancer : norme >= lancer_mg et gyro >= lancer_dps tenus ensemble au moins
//   lancer_ms -> candidat ; Throw quand secousse_fenetre_ms s'est ecoule apres
//   ce candidat sans secousse. Une secousse annule le candidat.
// - Sens du lancer : signe de ay a l'echantillon de norme maximale de la salve,
//   jusqu'au Throw. ay > 0 -> droite, sinon gauche. Calibre le 2026-11-20 sur
//   10 lancers reels avec la regle inverse ; le 2026-11-21, l'auteur constate que le
//   chiot part du mauvais cote a chaque lancer : regle et etiquettes des traces
//   inversees. Lu par lancer_droite() apres Throw.
// - Apres Throw ou Shake : ni Throw ni Shake pendant pause_gestes_ms, et les
//   pics restants de la salve sont ignores.
// - Journal (journal >= 1, spec G8) : voir take_journal.
// - set_settings vaut a partir de l'echantillon suivant.
// - Pas d'allocation par echantillon sauf pour le journal (journal >= 1).
class GestureDetector {
public:
    static constexpr size_t kMaxSalve = 300; // echantillons gardes en journal 2
    static constexpr int kMaxCoups = 20;     // borne de secousse_coups

    void set_settings(const Settings &s);
    std::optional<Input> feed(const Sample &s);
    // Sens du dernier Throw rendu (false : gauche, aussi avant tout lancer).
    bool lancer_droite() const { return lancer_droite_; }

    // Lignes de journal pretes a imprimer, videes a la lecture. L'appelant doit
    // les vider regulierement (sinon le tampon grossit tant que journal >= 1).
    // journal 0 : rien. journal >= 1 :
    // - une ligne par salve finie :
    //   "salve pic_mg=.. pic_dps=.. ms=.. coups=.. -> lancer sens=gauche|droite|secousse|rien grav=x,y,z perdus=N"
    //   ms = du debut au dernier echantillon agite ; coups = tous les pics ;
    //   grav = moyenne (g, 3 decimales) des echantillons calmes qui ont termine
    //   la salve ; perdus = echantillons de journal 2 au-dela du plafond (0 sinon).
    // - a chaque FaceDown / FaceUp : "face bas grav=x,y,z" / "face haut grav=x,y,z"
    //   (valeurs de l'echantillon de decision).
    // journal 2 en plus, echantillons "S t ax ay az gx gy gz" (t en ms, 3 decimales),
    // plafonnes aux kMaxSalve premiers :
    // - ceux de la salve, avant sa ligne "salve" ;
    // - ceux du candidat face bas (condition vraie jusqu'a FaceDown), avant la
    //   ligne "face bas", qui porte alors aussi " perdus=N". Un candidat abandonne
    //   n'imprime rien.
    std::vector<std::string> take_journal();

private:
    enum class Decision { Rien, Lancer, Secousse };

    std::optional<Input> feed_salve(const Sample &s, float norme, float gyro);
    std::optional<Input> feed_face(const Sample &s, float norme, float gyro);
    void fin_salve();
    void journal_face(const Sample &s, bool bas);
    void journal_echantillons(const std::vector<Sample> &v);

    Settings reglages_;

    // Face
    bool face_bas_ = false;
    int64_t bas_depuis_ = -1;  // condition "face bas" tenue depuis (-1 : non)
    int64_t haut_depuis_ = -1; // condition "face haut" tenue depuis

    // Salve
    bool en_salve_ = false;
    int64_t debut_salve_ = 0, dernier_agite_ = 0;
    float pic_g_ = 0, pic_dps_ = 0;
    float ay_pic_ = 0;            // ay a l'echantillon de norme maximale
    bool lancer_droite_ = false;
    int coups_ = 0;
    bool pic_arme_ = true;
    bool salve_close_ = false; // un geste a ete rendu : le reste est ignore
    Decision decision_ = Decision::Rien;

    // Pics recents (fenetre de secousse), anneau borne.
    std::array<int64_t, kMaxCoups> pics_{};
    int nb_pics_ = 0, tete_pics_ = 0;

    // Lancer
    int64_t fort_depuis_ = -1;  // norme et gyro au-dessus des seuils depuis
    bool fort_compte_ = false;  // cet episode fort a deja donne un candidat
    int64_t candidat_ = -1;     // candidat lancer en attente (-1 : aucun)

    int64_t pause_jusqu_ = INT64_MIN; // pas de Throw/Shake avant

    std::vector<std::string> journal_;
    std::vector<Sample> echantillons_;
    int perdus_ = 0;
    // Moyenne des echantillons calmes depuis le dernier agite (grav de la salve).
    float calme_somme_[3] = {0, 0, 0};
    int calme_n_ = 0;
    Sample dernier_{};
    // Journal 2 : echantillons du candidat face bas.
    std::vector<Sample> face_echantillons_;
    int face_perdus_ = 0;
};

} // namespace pup
