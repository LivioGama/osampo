#include <cctype>
#include "app.h"
#include "data.h"
#include "decode.h"
#include "fake_source.h"
#include "logic/controller.h"
#include "logic/volume.h"
#include "socket_link.h"
#include "sound_desktop.h"
#include "widget.h"

#include <algorithm>  // max
#include <chrono>
#include <cstdio>
#include <cstdlib>   // getenv
#include <cstring>
#include <filesystem>
#include <fstream>
#include <memory>
#include <mutex>
#include <slint.h>
#include <string>
#include <unistd.h>  // getuid

namespace {

// Identifiant du widget, annonce au daemon dans la poignee de main.
constexpr const char *kWidgetId = "osampo";

// Plancher du delai d inactivite. Le delai vaut 3 fois la cadence annoncee par
// le daemon, sans descendre sous cette valeur.
constexpr int64_t kDelaiPlancherMs = 1000;

// Peremption des donnees, en secondes : stale_s de widget.toml.
constexpr int kStaleS = 30;

std::string chemin_du_socket()
{
    const char *xdg = std::getenv("XDG_RUNTIME_DIR");
    if (xdg && *xdg) {
        return std::string(xdg) + "/osampo.sock";
    }
    return "/tmp/osampo-" + std::to_string(getuid()) + "/osampo.sock";
}

// $XDG_STATE_HOME/osampo/state.txt, repli ~/.local/state/osampo/state.txt.
std::filesystem::path chemin_de_l_etat()
{
    const char *xdg = std::getenv("XDG_STATE_HOME");
    std::filesystem::path base;
    if (xdg && *xdg) {
        base = xdg;
    } else {
        const char *home = std::getenv("HOME");
        base = std::filesystem::path(home ? home : ".") / ".local" / "state";
    }
    return base / "osampo" / "state.txt";
}

void charger(pup::Controller &c, const std::filesystem::path &p)
{
    std::ifstream in(p);
    std::string ligne;
    if (in && std::getline(in, ligne)) {
        c.load(ligne); // un etat illisible est ignore
    }
}

// Ecriture atomique : un fichier temporaire, puis rename. Une coupure en
// pleine ecriture laisse l ancien etat intact.
void ecrire(const std::string &s, const std::filesystem::path &p)
{
    std::error_code ec;
    std::filesystem::create_directories(p.parent_path(), ec);
    std::filesystem::path tmp = p;
    tmp += ".tmp";
    bool ok = false;
    {
        std::ofstream out(tmp, std::ios::trunc);
        out << s << '\n';
        out.flush();
        ok = static_cast<bool>(out);
    }
    if (ok) {
        std::filesystem::rename(tmp, p, ec);
    }
    if (!ok || ec) {
        std::fprintf(stderr, "etat non ecrit: %s\n", p.c_str());
    }
}

int64_t mono_s()
{
    return std::chrono::duration_cast<std::chrono::seconds>(
               std::chrono::steady_clock::now().time_since_epoch())
        .count();
}

// --heure HH:MM
bool lire_heure(const char *s, int &hh, int &mm)
{
    char fin = 0;
    return std::sscanf(s, "%d:%d%c", &hh, &mm, &fin) == 2 && hh >= 0 && hh < 24 && mm >= 0 &&
           mm < 60;
}

int usage(const char *prog)
{
    std::fprintf(stderr, "usage: %s [--fake [--heure HH:MM]]\n", prog);
    return 2;
}

}  // namespace

int main(int argc, char **argv)
{
    // Le repli factice est EXPLICITE. Le deduire de l absence du socket rendrait
    // tout test faussement vert : un daemon qui plante laisse son fichier en
    // place, et une panne se verrait alors comme une animation normale.
    bool fake = false;
    bool heure = false;
    int hh = 0, mm = 0;
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--fake") == 0) {
            fake = true;
        } else if (std::strcmp(argv[i], "--heure") == 0 && i + 1 < argc &&
                   lire_heure(argv[i + 1], hh, mm)) {
            heure = true;
            ++i;
        } else {
            return usage(argv[0]);
        }
    }
    if (heure && !fake) {
        return usage(argv[0]);
    }

    auto square = ViewSquare::create();
    slint::ComponentWeakHandle<ViewSquare> weak_square(square);

    // Le controleur et l etat du lien ne sont touches que depuis la boucle
    // d evenements Slint.
    const std::filesystem::path chemin_etat = chemin_de_l_etat();
    auto ctrl = std::make_shared<pup::Controller>(kStaleS);
    charger(*ctrl, chemin_etat);
    auto en_ligne = std::make_shared<bool>(false);
    auto sauve = std::make_shared<std::string>(ctrl->save());

    auto rafraichir = [weak_square, ctrl, en_ligne] {
        if (auto v = weak_square.lock()) {
            apply(**v, ctrl->view(), *en_ligne);
        }
    };
    // Tick, son, puis ecriture de l etat s il a change (changement de jour,
    // besoin satisfait).
    auto battre = [ctrl, sauve, chemin_etat, rafraichir] {
        play(ctrl->tick(mono_s()));
        const std::string s = ctrl->save();
        if (s != *sauve) {
            ecrire(s, chemin_etat);
            *sauve = s;
        }
        rafraichir();
    };
    auto lien_change = [ctrl, en_ligne, rafraichir](bool vivant) {
        *en_ligne = vivant;
        ctrl->on_link(vivant);
        rafraichir();
    };
    // Appele depuis un thread source : la trame passe par la boucle.
    auto pousser = [ctrl, rafraichir](DataFrame d) {
        slint::invoke_from_event_loop([ctrl, rafraichir, d] {
            ctrl->on_frame(d, mono_s());
            rafraichir();
        });
    };

    // Un geste agit tout de suite, et son cue (Jappe) part sans attendre la
    // seconde suivante.
    auto geste = [ctrl, battre](pup::Input in) {
        ctrl->on_input(in, mono_s());
        battre();
    };
    auto face_down = std::make_shared<bool>(false);
    square->on_tapped([geste] { geste(pup::Input::Tap); });
    square->on_long_pressed([geste] { geste(pup::Input::LongPress); });
    square->on_stroked([geste] { geste(pup::Input::Stroke); });
    square->on_anim_done([geste] { geste(pup::Input::AnimDone); });
    square->on_brossed([geste] { geste(pup::Input::Brosse); });
    square->on_os_donne([geste] { geste(pup::Input::Os); });
    square->on_laisse_touchee([geste] { geste(pup::Input::Laisse); });
    auto pile_faible = std::make_shared<bool>(false);
    // Desktop seulement : gestes de l IMU simules au clavier.
    // Lancer : la touche en minuscule vers la gauche, en majuscule vers la droite.
    square->on_key([ctrl, geste, face_down, square, pile_faible](slint::SharedString k) {
        const std::string_view t(k.data(), k.size());
        const pup::Settings &st = ctrl->settings();
        if (t.size() != 1) {
            return;
        }
        const char lancer_d = static_cast<char>(std::toupper(static_cast<unsigned char>(st.k_lancer)));
        if (t[0] == st.k_lancer) {
            square->set_vers_la_droite(false);
            geste(pup::Input::Throw);
        } else if (t[0] == st.k_secousse) {
            geste(pup::Input::Shake);
        } else if (t[0] == st.k_retourner) {
            *face_down = !*face_down;
            geste(*face_down ? pup::Input::FaceDown : pup::Input::FaceUp);
        } else if (t[0] == 'l') {
            geste(pup::Input::Laisse); // desktop : la laisse au clavier (SP5h)
        } else if (t[0] == 'b') {
            // desktop : batterie fictive juste sous le seuil [ecran] batterie_faible_mv
            // (sans seuil, rien ne s'affiche) ; la vue suit au battement suivant.
            *pile_faible = !*pile_faible;
            const int seuil = ctrl->settings().batterie_faible_mv;
            ctrl->on_battery(*pile_faible && seuil > 0 ? seuil - 1 : -1);
        } else if (t[0] == 'v') {
            // Desktop : la touche v remplace le bouton BOOT de la board (SP5e).
            const int n = pup::volume_suivant(square->get_volume());
            set_volume(n);
            square->set_volume(n);
            play(pup::Cue::Wouf);
        } else if (t[0] == lancer_d) {
            square->set_vers_la_droite(true);
            geste(pup::Input::Throw);
        }
    });

    slint::Timer battement;
    battement.start(slint::TimerMode::Repeated, std::chrono::seconds(1), battre);

    rafraichir();
    square->show();

    if (fake) {
        lien_change(true);
        FakeSource source([pousser](const DataFrame &d) { pousser(d); },
                          heure ? FakeSource::offset_to(hh, mm) : 0);
        source.start();
        slint::run_event_loop();
        source.stop();
        ecrire(ctrl->save(), chemin_etat);
        return 0;
    }

    std::mutex garde;
    DataFrame courante;
    pup::Settings miroir;  // protege par garde
    SocketLink lien(chemin_du_socket(), kWidgetId,
                    [&garde, &courante, &miroir, pousser, ctrl, rafraichir](const JsonLine &ligne) {
                        // Le sink tourne sur le thread du lien : une exception
                        // qui s en echapperait terminerait le processus.
                        try {
                            // Une ligne cfg n est jamais une trame : elle est
                            // appliquee dans la boucle Slint (ni decode, ni pousser).
                            if (ligne.has("cfg")) {
                                // JsonLine n est pas copiable : on applique ici sur le
                                // miroir des reglages (seules les lignes cfg les
                                // modifient), puis la copie passe a la boucle.
                                pup::Settings st;
                                {
                                    std::lock_guard<std::mutex> g(garde);
                                    for (const std::string &err : miroir.apply(ligne)) {
                                        std::fprintf(stderr, "cfg: %s\n", err.c_str());
                                    }
                                    st = miroir;
                                }
                                slint::invoke_from_event_loop([ctrl, rafraichir, st] {
                                    ctrl->set_settings(st);
                                    rafraichir();
                                });
                                return;
                            }
                            DataFrame copie;
                            {
                                std::lock_guard<std::mutex> g(garde);
                                decode(ligne, courante);
                                copie = courante;
                            }
                            pousser(copie);
                        } catch (const std::exception &e) {
                            std::fprintf(stderr, "trame ignoree: %s\n", e.what());
                        } catch (...) {
                            std::fprintf(stderr, "trame ignoree\n");
                        }
                    });
    lien.start();

    // Chien de garde : sans trame depuis le delai d inactivite, on passe hors
    // ligne. Il tourne sur la boucle d evenements, comme le controleur.
    slint::Timer chien;
    chien.start(slint::TimerMode::Repeated, std::chrono::milliseconds(250),
                [&lien, en_ligne, lien_change] {
                    // delai = 3 x la cadence annoncee, jamais sous le plancher.
                    const int64_t delai =
                        std::max<int64_t>(kDelaiPlancherMs, 3LL * lien.cadence_ms());
                    const int64_t depuis = lien.ms_since_last_frame();
                    const bool vivant = lien.connected() && depuis >= 0 && depuis < delai;
                    if (vivant != *en_ligne) {
                        lien_change(vivant);
                    }
                });

    slint::run_event_loop();
    lien.stop();
    ecrire(ctrl->save(), chemin_etat);
    return 0;
}
