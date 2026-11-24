#pragma once
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "data.h"
#include "logic/clock.h"
#include "logic/lien_pc.h"
#include "logic/needs.h"
#include "logic/pup.h"
#include "logic/reminders.h"
#include "logic/settings.h"

namespace pup {

struct ViewModel {
    std::string state;    // "repos" "dort" "reclame" "rapporte" "content" "tournis" "boude" "bain" "os" "promenade"
    std::string reason;   // "" "rappel" "promenade" "claude"
    std::string reminder; // "" "sortie" "sport" "repas" "dodo"
    bool night = false;   // horloge reglee et dans la plage nuit des reglages (choix Claude)
    bool clock_ok = false;
    bool en_ligne = false; // donnees du PC utilisables (usable), quel que soit le lien (SP5d2 B11) ;
                          // vrai aussi pendant stale_s apres le demarrage (pas d'icone le temps de se connecter)
    int bouger = 0, jouer = 0, dormir = 0; // niveaux 0..2 (journal ; plus affiches depuis SP5e)
    int heure = -1, minute = -1;           // heure locale affichee, -1 sans horloge
    bool laisse = false;          // icone laisse (SP5h) : tout sauf Dort
    bool bulle_dodo = false;      // reclamation de la promenade du soir
    bool batterie_faible = false; // tension sous [ecran] batterie_faible_mv
    // Planche a jouer : "repos" "dort" "aboie" "laisse" "patte" "content"
    // "roulade" "rapporte" "tournis" "boude" "bain" "os" "promenade" ; "laisse"
    // aussi pour un rappel sortie, sport ou dodo. Content vient d'une caresse ->
    // "roulade", sinon (rappel valide) -> "content" (R-y).
    std::string anim;
};

// Cable horloge, rappels, besoins et machine a etats du chiot. Pas de Slint.
//
// - Donnees utilisables : online && age effectif <= stale_s, avec age effectif =
//   age de la derniere frame + temps ecoule depuis sa reception. Sinon la
//   jauge est perimee et claude_wait(false).
// - online vient UNIQUEMENT de on_link ; DataFrame::online est ignore.
// - Sans horloge (B1) : ni rappel, ni calme, ni besoin.
class Controller {
public:
    explicit Controller(int stale_s, std::vector<Reminder> list = default_reminders());
    void on_frame(const DataFrame &d, int64_t mono_s);
    void on_link(bool online);
    void on_battery(int mv); // -1 : inconnue ou secteur, jamais faible
    void on_input(Input in, int64_t mono_s);
    Cue tick(int64_t mono_s); // appele chaque seconde
    ViewModel view() const;
    std::string save() const; // besoins, rappels sonnes, dernier Dormir, promenade ; l'horloge se resynchronise
    bool load(const std::string &s);
    // Propage aux rappels, au chiot et aux besoins ; ne touche pas l'etat en cours.
    // Une liste de rappels differente remplace celle du planificateur.
    void set_settings(const Settings &s);
    const Settings &settings() const;

    // Commande du PC (SPEC-001) : memes regles que le geste equivalent sur la board ; une commande
    // ignoree par le chiot (id non reclame, chiot endormi, en promenade...) ne produit rien.
    void on_command(const Commande &c, int64_t mono_s);
    // Evenements a emettre vers le PC depuis le dernier appel, dans l'ordre ; vide ensuite.
    std::vector<Evenement> take_events();
    // Champs de la ligne d'etat envoyee au PC.
    EtatBoard etat_board() const;

private:
    bool usable() const;
    void avancer(int64_t mono_s);
    std::optional<int64_t> today(int64_t mono_s) const;
    void entree(Input in, int64_t mono_s, const char *source);
    void recolter(const char *source);

    int stale_s_;
    Settings cfg_;
    Clock clock_;
    ReminderScheduler sched_;
    Needs needs_;
    Pup pup_;
    DataFrame frame_;
    bool online_ = false;
    int battery_mv_ = -1;
    // Rappels sonnes lus par load, reappliques quand la liste [[rappel]] arrive
    // apres l'etat (demarrage : liste compilee, etat, puis config) ; une fois.
    std::optional<std::string> sched_attente_;
    std::optional<int64_t> dormir_day_; // dernier jour ou FaceDown a donne Dormir
    bool content_by_stroke_ = false;  // l'etat Content courant vient d'un Stroke
    int64_t mono_ = 0;
    std::vector<Evenement> events_;
    int64_t debut_ = -1;              // premier temps mono vu (grace de demarrage de l'icone de lien)
    int64_t synced_epoch_ = 0;        // derniere valeur transmise a clock_
    int32_t synced_tz_ = 0;
    int64_t frame_mono_ = 0;          // mono de reception de frame_
};

} // namespace pup
