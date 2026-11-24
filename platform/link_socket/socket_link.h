#pragma once
#include "json_line.h"
#include <atomic>
#include <condition_variable>
#include <cstdint>
#include <functional>
#include <mutex>
#include <string>
#include <thread>

// Client du socket du daemon.
//
// Tient la connexion, envoie la poignee de main, decoupe le flux en lignes et
// remet chaque ligne decodee au sink. Se reconnecte tout seul.
//
// Aucune dependance a Slint : l appelant est responsable de repasser par la
// boucle d evenements avant de toucher une propriete de vue.
class SocketLink {
public:
    // Le sink tourne sur le thread du lien : n y jamais appeler stop(), n y jamais
    // detruire le SocketLink, n en laisser jamais s echapper une exception.
    using LineSink = std::function<void(const JsonLine &)>;

    SocketLink(std::string socket_path, std::string widget_id, LineSink sink);
    ~SocketLink();

    SocketLink(const SocketLink &) = delete;
    SocketLink &operator=(const SocketLink &) = delete;

    void start();
    void stop();

    bool connected() const { return connected_.load(); }

    // -1 tant qu aucune trame n est arrivee. Sert au chien de garde qui decide
    // de l etat online, cote appelant.
    int64_t ms_since_last_frame() const;

    // Valeurs annoncees par le daemon dans son ack. Avant tout ack, ce sont les
    // defauts du protocole. Le delai d inactivite s en deduit : un widget
    // cadence a 5 s serait hors ligne en permanence avec un seuil en dur.
    int cadence_ms() const { return cadence_ms_.load(); }
    int stale_s() const { return stale_s_.load(); }

private:
    void run();
    // Rend le descripteur connecte, ou -1. Envoie la poignee de main et
    // consomme la reponse ack ou error.
    int connect_and_greet();
    void pump(int fd);
    // Traite une ligne complete. Rend false si la connexion doit etre lachee
    // (refus du daemon).
    bool handle_line(const char *ligne, size_t longueur, bool &ack_vu);
    // Attend le delai de reconnexion, ou moins si stop() est appele.
    void wait_before_retry();
    // Publie ou retire le descripteur courant, sous fd_m_ : stop() ne doit
    // jamais fermer un descripteur deja rendu au systeme puis reattribue.
    void publish_fd(int fd);
    void close_fd(int fd);

    std::string path_;
    std::string widget_id_;
    LineSink sink_;

    std::thread thread_;
    std::atomic<bool> running_{false};

    // Descripteur de la connexion en cours, -1 sinon. stop() fait un
    // shutdown dessus pour debloquer un recv en attente : un pair connecte et
    // muet (demarrage a froid) ne doit pas retenir l arret.
    std::atomic<int> fd_{-1};
    std::mutex fd_m_;

    // Reveille l attente de reconnexion quand stop() est appele.
    std::mutex attente_m_;
    std::condition_variable attente_cv_;

    std::atomic<bool> connected_{false};
    std::atomic<int64_t> last_frame_ms_{-1};
    std::atomic<int> cadence_ms_{500};
    std::atomic<int> stale_s_{30};
};
