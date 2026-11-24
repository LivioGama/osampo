#include "socket_link.h"

// Les tests reposent sur assert : on le garde actif meme sous -DNDEBUG (Release).
#undef NDEBUG
#include <atomic>
#include <cassert>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <mutex>
#include <string>
#include <sys/socket.h>
#include <sys/un.h>
#include <thread>
#include <unistd.h>
#include <vector>

namespace {

std::string chemin_temporaire(const char *nom)
{
    std::string p = "/tmp/sp3_link_" + std::string(nom) + ".sock";
    ::unlink(p.c_str());
    return p;
}

// Faux daemon : accepte une connexion, lit le hello, puis ecrit exactement les
// octets qu on lui donne. C est ainsi qu on controle le decoupage.
class FauxDaemon {
public:
    // garde_ms : combien de temps la connexion reste ouverte et muette apres
    // le dernier paquet. Une garde longue simule le demarrage a froid.
    FauxDaemon(std::string path, std::vector<std::string> paquets, int garde_ms = 200)
        : path_(std::move(path)), paquets_(std::move(paquets)), garde_ms_(garde_ms)
    {
        fd_ = ::socket(AF_UNIX, SOCK_STREAM, 0);
        assert(fd_ >= 0);
        sockaddr_un a{};
        a.sun_family = AF_UNIX;
        std::strncpy(a.sun_path, path_.c_str(), sizeof(a.sun_path) - 1);
        assert(::bind(fd_, reinterpret_cast<sockaddr *>(&a), sizeof(a)) == 0);
        assert(::listen(fd_, 1) == 0);
        thread_ = std::thread(&FauxDaemon::run, this);
    }

    ~FauxDaemon()
    {
        arret_.store(true);
        ::shutdown(fd_, SHUT_RDWR);
        ::close(fd_);
        if (thread_.joinable()) {
            thread_.join();
        }
        ::unlink(path_.c_str());
    }

    std::string hello_recu()
    {
        std::lock_guard<std::mutex> g(m_);
        return hello_;
    }

private:
    void run()
    {
        const int c = ::accept(fd_, nullptr, nullptr);
        if (c < 0) {
            return;
        }
        char brut[256];
        const ssize_t n = ::recv(c, brut, sizeof(brut), 0);
        if (n > 0) {
            std::lock_guard<std::mutex> g(m_);
            hello_.assign(brut, static_cast<size_t>(n));
        }
        for (const auto &p : paquets_) {
            if (arret_.load()) {
                break;
            }
            ::send(c, p.data(), p.size(), MSG_NOSIGNAL);
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
        }
        for (int t = 0; t < garde_ms_ && !arret_.load(); t += 10) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        ::close(c);
    }

    std::string path_;
    std::vector<std::string> paquets_;
    int garde_ms_;
    int fd_ = -1;
    std::thread thread_;
    std::atomic<bool> arret_{false};
    std::mutex m_;
    std::string hello_;
};

struct Collecte {
    std::mutex m;
    std::vector<float> valeurs;
    std::vector<std::string> libelles;

    void ajouter(const JsonLine &l)
    {
        std::lock_guard<std::mutex> g(m);
        valeurs.push_back(l.number("value", -1.0f));
        libelles.push_back(l.text("label", "ABSENT"));
    }

    size_t taille()
    {
        std::lock_guard<std::mutex> g(m);
        return valeurs.size();
    }
};

void attendre_au_moins(Collecte &c, size_t n, const char *quoi)
{
    for (int i = 0; i < 100 && c.taille() < n; ++i) {
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    if (c.taille() < n) {
        std::fprintf(stderr, "echec: %s, %zu trames sur %zu attendues\n", quoi, c.taille(), n);
        std::abort();
    }
}

void une_trame_coupee_en_deux_paquets()
{
    const std::string p = chemin_temporaire("coupe");
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n{\"value\":4", "2,\"label\":\"a\"}\n"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "trame coupee en deux paquets");
    l.stop();
    assert(c.valeurs[0] == 42.0f);
    assert(c.libelles[0] == "a");
}

void deux_trames_dans_un_paquet()
{
    const std::string p = chemin_temporaire("deux");
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n{\"value\":1}\n{\"value\":2}\n"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 2, "deux trames dans un paquet");
    l.stop();
    assert(c.valeurs[0] == 1.0f);
    assert(c.valeurs[1] == 2.0f);
}

void ligne_sans_saut_final_ignoree()
{
    const std::string p = chemin_temporaire("sansfin");
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n{\"value\":7}\n{\"value\":9}"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "ligne sans saut final");
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    l.stop();
    assert(c.taille() == 1 && "une ligne sans saut de ligne final ne doit pas etre remise");
}

void json_invalide_ignore_sans_casser_le_flux()
{
    const std::string p = chemin_temporaire("invalide");
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n", "pas du json\n", "{\"value\":5}\n"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "json invalide puis trame valide");
    l.stop();
    assert(c.valeurs[0] == 5.0f && "la trame valide qui suit doit passer");
}

void le_hello_porte_l_identifiant()
{
    const std::string p = chemin_temporaire("hello");
    FauxDaemon d(p, {"{\"ack\":\"demo3\"}\n{\"value\":1}\n"});
    Collecte c;
    SocketLink l(p, "demo3", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "hello");
    l.stop();
    assert(d.hello_recu() == "{\"hello\":\"demo3\"}\n");
}

void la_cadence_annoncee_est_reprise()
{
    const std::string p = chemin_temporaire("cadence");
    FauxDaemon d(p, {"{\"ack\":\"w\",\"cadence_ms\":5000,\"stale_s\":90}\n{\"value\":1}\n"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    assert(l.cadence_ms() == 500 && "avant tout ack, le defaut du protocole");
    l.start();
    attendre_au_moins(c, 1, "cadence annoncee");
    assert(l.cadence_ms() == 5000 && "la cadence de l ack doit etre reprise");
    assert(l.stale_s() == 90 && "le seuil de l ack doit etre repris");
    l.stop();
}

void un_refus_ne_produit_aucune_trame()
{
    const std::string p = chemin_temporaire("refus");
    FauxDaemon d(p, {"{\"error\":\"manifeste introuvable\"}\n{\"value\":1}\n"});
    Collecte c;
    SocketLink l(p, "inconnu", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    std::this_thread::sleep_for(std::chrono::milliseconds(400));
    l.stop();
    assert(c.taille() == 0 && "apres un refus, aucune trame ne doit remonter");
}

void sans_daemon_rien_ne_remonte_et_rien_ne_plante()
{
    const std::string p = chemin_temporaire("absent");
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    std::this_thread::sleep_for(std::chrono::milliseconds(400));
    assert(!l.connected());
    assert(l.ms_since_last_frame() == -1);
    assert(c.taille() == 0);
    l.stop();
}

// Ruling C1 : le pair est connecte et muet (demarrage a froid : ack, puis
// aucune trame). stop() doit rendre la main sans attendre le pair.
void stop_rend_la_main_face_a_un_pair_muet()
{
    const std::string p = chemin_temporaire("muet");
    FauxDaemon d(p, {"{\"ack\":\"w\",\"cadence_ms\":500,\"stale_s\":30}\n"}, 5000);
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    for (int i = 0; i < 100 && !l.connected(); ++i) {
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    assert(l.connected() && "l ack doit marquer la connexion");
    const auto t0 = std::chrono::steady_clock::now();
    l.stop();
    const auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                        std::chrono::steady_clock::now() - t0)
                        .count();
    std::fprintf(stderr, "stop face a un pair muet: %lld ms\n", static_cast<long long>(ms));
    assert(ms < 300 && "stop() doit interrompre un recv bloque");
    assert(c.taille() == 0);
}

// Faux daemon qui refuse chaque hello et compte les connexions.
class FauxDaemonRefus {
public:
    explicit FauxDaemonRefus(std::string path) : path_(std::move(path))
    {
        fd_ = ::socket(AF_UNIX, SOCK_STREAM, 0);
        assert(fd_ >= 0);
        sockaddr_un a{};
        a.sun_family = AF_UNIX;
        std::strncpy(a.sun_path, path_.c_str(), sizeof(a.sun_path) - 1);
        assert(::bind(fd_, reinterpret_cast<sockaddr *>(&a), sizeof(a)) == 0);
        assert(::listen(fd_, 8) == 0);
        thread_ = std::thread(&FauxDaemonRefus::run, this);
    }

    ~FauxDaemonRefus()
    {
        arret_.store(true);
        ::shutdown(fd_, SHUT_RDWR);
        ::close(fd_);
        if (thread_.joinable()) {
            thread_.join();
        }
        ::unlink(path_.c_str());
    }

    int connexions() const { return connexions_.load(); }

private:
    void run()
    {
        while (!arret_.load()) {
            const int c = ::accept(fd_, nullptr, nullptr);
            if (c < 0) {
                return;
            }
            connexions_.fetch_add(1);
            char brut[256];
            (void)::recv(c, brut, sizeof(brut), 0);
            const std::string refus = "{\"error\":\"manifeste introuvable\"}\n";
            ::send(c, refus.data(), refus.size(), MSG_NOSIGNAL);
            ::close(c);
        }
    }

    std::string path_;
    int fd_ = -1;
    std::thread thread_;
    std::atomic<bool> arret_{false};
    std::atomic<int> connexions_{0};
};

// Ruling C8 : apres un refus, le client attend 1 s avant de retenter. Sur une
// fenetre de 2,5 s : connexions a t=0, 1 s, 2 s, soit 3 au plus.
void un_refus_ne_produit_pas_de_boucle_serree()
{
    const std::string p = chemin_temporaire("cadence_refus");
    FauxDaemonRefus d(p);
    Collecte c;
    SocketLink l(p, "inconnu", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    std::this_thread::sleep_for(std::chrono::milliseconds(2500));
    const auto t0 = std::chrono::steady_clock::now();
    l.stop();
    const auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                        std::chrono::steady_clock::now() - t0)
                        .count();
    const int n = d.connexions();
    std::fprintf(stderr, "refus: %d connexions en 2,5 s, stop en %lld ms\n", n,
                 static_cast<long long>(ms));
    assert(n >= 2 && "le client doit retenter apres un refus");
    assert(n <= 3 && "au plus une tentative par seconde apres un refus");
    assert(ms < 300 && "l attente de reconnexion doit ceder a stop()");
    assert(c.taille() == 0);
}

// Ruling C9 : une ligne de plus de 4096 octets est jetee entiere. Sa queue,
// qui ressemble a une trame valide, ne doit pas etre decodee.
void ligne_trop_longue_jetee_jusqu_au_saut_suivant()
{
    const std::string p = chemin_temporaire("longue");
    // 5000 octets de bourrage puis une queue qui, seule, serait une trame.
    const std::string longue = "{\"pad\":\"" + std::string(5000, 'x') + "\"}{\"value\":666}\n";
    // La queue arrive dans un paquet separe, apres que le tampon a deborde.
    const std::string tete = longue.substr(0, 4500);
    const std::string queue = longue.substr(4500);
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n", tete, queue, "{\"value\":3}\n"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "trame valide apres une ligne trop longue");
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    l.stop();
    assert(c.taille() == 1 && "seule la trame valide doit remonter");
    assert(c.valeurs[0] == 3.0f);
}

// Variante : la queue de la ligne trop longue est elle-meme un objet complet
// sur sa propre ligne logique, decoupe pour tomber pile apres le debordement.
void queue_de_ligne_trop_longue_jamais_prise_pour_une_trame()
{
    const std::string p = chemin_temporaire("queue");
    const std::string bourrage(4200, 'y');
    // Paquet 1 : 4200 octets sans saut (debordement). Paquet 2 : une queue qui
    // est un JSON valide termine par \n. Paquet 3 : la vraie trame.
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n", bourrage, "{\"value\":666}\n", "{\"value\":4}\n"});
    Collecte c;
    SocketLink l(p, "w", [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "trame valide apres une queue trompeuse");
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    l.stop();
    assert(c.taille() == 1 && "la queue d une ligne trop longue ne doit pas remonter");
    assert(c.valeurs[0] == 4.0f);
}

// Fix round 1 : un identifiant qui contient " et \ doit donner un hello JSON
// valide, relu a l identique. Une concatenation le casserait.
void le_hello_echappe_l_identifiant()
{
    const std::string p = chemin_temporaire("echappe");
    const std::string id = "a\"b\\c\",\"x\":\"y";
    FauxDaemon d(p, {"{\"ack\":\"w\"}\n{\"value\":1}\n"});
    Collecte c;
    SocketLink l(p, id, [&c](const JsonLine &j) { c.ajouter(j); });
    l.start();
    attendre_au_moins(c, 1, "hello echappe");
    l.stop();
    const std::string h = d.hello_recu();
    assert(!h.empty() && h.back() == '\n' && "le hello finit par un saut de ligne");
    JsonLine relu;
    assert(relu.parse(h.data(), h.size() - 1) && "le hello doit etre du JSON valide");
    assert(relu.text("hello", "ABSENT") == id && "l identifiant doit revenir a l identique");
    assert(!relu.has("x") && "aucune cle injectee");
}

struct Cas {
    const char *nom;
    void (*fn)();
};

const Cas kCas[] = {
    {"une_trame_coupee_en_deux_paquets", une_trame_coupee_en_deux_paquets},
    {"deux_trames_dans_un_paquet", deux_trames_dans_un_paquet},
    {"ligne_sans_saut_final_ignoree", ligne_sans_saut_final_ignoree},
    {"json_invalide_ignore_sans_casser_le_flux", json_invalide_ignore_sans_casser_le_flux},
    {"le_hello_porte_l_identifiant", le_hello_porte_l_identifiant},
    {"la_cadence_annoncee_est_reprise", la_cadence_annoncee_est_reprise},
    {"un_refus_ne_produit_aucune_trame", un_refus_ne_produit_aucune_trame},
    {"sans_daemon_rien_ne_remonte_et_rien_ne_plante", sans_daemon_rien_ne_remonte_et_rien_ne_plante},
    {"stop_rend_la_main_face_a_un_pair_muet", stop_rend_la_main_face_a_un_pair_muet},
    {"un_refus_ne_produit_pas_de_boucle_serree", un_refus_ne_produit_pas_de_boucle_serree},
    {"ligne_trop_longue_jetee_jusqu_au_saut_suivant", ligne_trop_longue_jetee_jusqu_au_saut_suivant},
    {"queue_de_ligne_trop_longue_jamais_prise_pour_une_trame", queue_de_ligne_trop_longue_jamais_prise_pour_une_trame},
    {"le_hello_echappe_l_identifiant", le_hello_echappe_l_identifiant},
};

}  // namespace

// Sans argument : tous les cas. Avec un nom : ce cas seul, pour isoler un blocage.
int main(int argc, char **argv)
{
    // Chien de garde : un cas bloque ne doit jamais bloquer la suite. SIGALRM
    // tue le processus, ctest le compte en echec.
    ::alarm(30);
    int joues = 0;
    for (const Cas &c : kCas) {
        if (argc > 1 && std::strcmp(argv[1], c.nom) != 0) {
            continue;
        }
        c.fn();
        ++joues;
    }
    if (joues == 0) {
        std::fprintf(stderr, "cas inconnu: %s\n", argv[1]);
        return 2;
    }
    std::printf("test_socket_link: %d cas OK\n", joues);
    return 0;
}
