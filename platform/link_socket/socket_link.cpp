#include "socket_link.h"

#include <cJSON.h>
#include <chrono>
#include <csignal>
#include <cstdio>
#include <cstring>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <unistd.h>

namespace {

// Longueur maximale d une ligne, protocole. Au-dela, la ligne est jetee :
// un flux corrompu ne doit pas faire croitre le tampon sans borne.
constexpr size_t kLigneMax = 4096;

// Delai entre deux tentatives de connexion, apres toute sortie de pump()
// comme apres un echec de connect. Volontairement constant : la degressivite
// est un raccourci assume, voir le spec.
constexpr int kReconnexionMs = 1000;

// Borne connect et l envoi du hello. Sur AF_UNIX, connect bloque quand la file
// d attente du daemon est pleine ; SO_SNDTIMEO le borne.
constexpr int kEnvoiMaxMs = 500;

// Construit {"hello":"<id>"}\n par cJSON : un identifiant qui contient un guillemet ou une barre oblique inverse
// serait, par concatenation, du JSON invalide ou une cle injectee.
bool build_hello(const std::string &id, std::string &sortie)
{
    cJSON *o = cJSON_CreateObject();
    if (o == nullptr) {
        return false;
    }
    if (cJSON_AddStringToObject(o, "hello", id.c_str()) == nullptr) {
        cJSON_Delete(o);
        return false;
    }
    char *texte = cJSON_PrintUnformatted(o);
    cJSON_Delete(o);
    if (texte == nullptr) {
        return false;
    }
    sortie.assign(texte);
    sortie.push_back('\n');
    cJSON_free(texte);
    return true;
}

int64_t maintenant_ms()
{
    using namespace std::chrono;
    return duration_cast<milliseconds>(steady_clock::now().time_since_epoch()).count();
}

}  // namespace

SocketLink::SocketLink(std::string socket_path, std::string widget_id, LineSink sink)
    : path_(std::move(socket_path)), widget_id_(std::move(widget_id)), sink_(std::move(sink))
{
    // Le client ecrit sa poignee de main. Si le daemon meurt entre connect et
    // write, SIGPIPE tuerait le processus : on l ignore une fois pour toutes.
    std::signal(SIGPIPE, SIG_IGN);
}

SocketLink::~SocketLink() { stop(); }

void SocketLink::start()
{
    if (running_.exchange(true)) {
        return;
    }
    thread_ = std::thread(&SocketLink::run, this);
}

void SocketLink::stop()
{
    {
        // Sous attente_m_ : l attente de reconnexion ne peut pas manquer le
        // reveil entre son test du predicat et sa mise en sommeil.
        std::lock_guard<std::mutex> g(attente_m_);
        if (!running_.exchange(false)) {
            return;
        }
    }
    attente_cv_.notify_all();
    {
        // Debloque un recv en attente. Pas de SHUT_WR seul : c est la fin de
        // lecture qui fait fermer le daemon, et ici on veut tout couper.
        std::lock_guard<std::mutex> g(fd_m_);
        const int fd = fd_.load();
        if (fd >= 0) {
            ::shutdown(fd, SHUT_RDWR);
        }
    }
    if (thread_.joinable()) {
        thread_.join();
    }
}

int64_t SocketLink::ms_since_last_frame() const
{
    const int64_t t = last_frame_ms_.load();
    if (t < 0) {
        return -1;
    }
    return maintenant_ms() - t;
}

void SocketLink::publish_fd(int fd)
{
    std::lock_guard<std::mutex> g(fd_m_);
    fd_.store(fd);
}

void SocketLink::close_fd(int fd)
{
    std::lock_guard<std::mutex> g(fd_m_);
    fd_.store(-1);
    ::close(fd);
}

int SocketLink::connect_and_greet()
{
    const int fd = ::socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        return -1;
    }
    // Publie des la creation : stop() peut ainsi couper un connect ou un send
    // en cours. Un stop() arrive juste avant la publication est rattrape par
    // le test de running_ qui suit.
    publish_fd(fd);
    if (!running_.load()) {
        close_fd(fd);
        return -1;
    }

    timeval envoi{};
    envoi.tv_sec = kEnvoiMaxMs / 1000;
    envoi.tv_usec = (kEnvoiMaxMs % 1000) * 1000;
    ::setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &envoi, sizeof(envoi));

    sockaddr_un adresse{};
    adresse.sun_family = AF_UNIX;
    if (path_.size() >= sizeof(adresse.sun_path)) {
        close_fd(fd);
        return -1;
    }
    std::strncpy(adresse.sun_path, path_.c_str(), sizeof(adresse.sun_path) - 1);

    if (::connect(fd, reinterpret_cast<sockaddr *>(&adresse), sizeof(adresse)) != 0) {
        close_fd(fd);
        return -1;
    }

    std::string hello;
    if (!build_hello(widget_id_, hello)) {
        close_fd(fd);
        return -1;
    }
    if (::send(fd, hello.data(), hello.size(), MSG_NOSIGNAL) != static_cast<ssize_t>(hello.size())) {
        close_fd(fd);
        return -1;
    }
    return fd;
}

bool SocketLink::handle_line(const char *ligne, size_t longueur, bool &ack_vu)
{
    if (longueur == 0) {
        return true;  // ligne vide : ignoree
    }

    JsonLine decodee;
    if (!decodee.parse(ligne, longueur)) {
        return true;  // JSON invalide : ignore, le flux continue
    }

    if (!ack_vu) {
        // la premiere ligne est la reponse a la poignee de main
        ack_vu = true;
        if (decodee.has("error")) {
            std::fprintf(stderr, "link_socket: refus du daemon: %.*s\n",
                         static_cast<int>(longueur), ligne);
            return false;  // refus : inutile d insister sur cette connexion
        }
        if (decodee.has("ack")) {
            // le daemon annonce sa cadence et son seuil : on s y plie,
            // plutot que de figer des nombres cote widget.
            cadence_ms_.store(decodee.integer("cadence_ms", cadence_ms_.load()));
            stale_s_.store(decodee.integer("stale_s", stale_s_.load()));
            connected_.store(true);
            return true;
        }
    }

    last_frame_ms_.store(maintenant_ms());
    sink_(decodee);
    return true;
}

void SocketLink::pump(int fd)
{
    std::string tampon;
    char brut[1024];
    bool ack_vu = false;
    // Vrai apres un debordement : on jette tout jusqu au prochain \n, sans quoi
    // la queue de la ligne trop longue serait prise pour une trame.
    bool jeter = false;

    while (running_.load()) {
        const ssize_t n = ::recv(fd, brut, sizeof(brut), 0);
        if (n <= 0) {
            return;  // ferme, en erreur, ou coupe par stop() : on ressort
        }

        size_t i = 0;
        const size_t total = static_cast<size_t>(n);
        while (i < total) {
            const void *nl = std::memchr(brut + i, '\n', total - i);
            const size_t fin = nl ? static_cast<size_t>(static_cast<const char *>(nl) - brut) : total;

            if (!jeter) {
                tampon.append(brut + i, fin - i);
                if (tampon.size() > kLigneMax) {
                    std::fprintf(stderr, "link_socket: ligne de plus de %zu octets jetee\n", kLigneMax);
                    tampon.clear();
                    jeter = true;
                }
            }
            if (nl == nullptr) {
                break;  // ligne incomplete : la suite viendra au prochain recv
            }

            if (!jeter && !handle_line(tampon.data(), tampon.size(), ack_vu)) {
                return;
            }
            tampon.clear();
            jeter = false;
            i = fin + 1;
        }
    }
}

void SocketLink::wait_before_retry()
{
    std::unique_lock<std::mutex> g(attente_m_);
    attente_cv_.wait_for(g, std::chrono::milliseconds(kReconnexionMs),
                         [this] { return !running_.load(); });
}

void SocketLink::run()
{
    while (running_.load()) {
        const int fd = connect_and_greet();
        if (fd >= 0) {
            pump(fd);
            close_fd(fd);
        }
        connected_.store(false);
        // Apres toute sortie, refus compris : jamais de boucle serree
        // connect, hello, error.
        wait_before_retry();
    }
}
