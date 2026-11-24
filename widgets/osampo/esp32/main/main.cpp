// osampo sur la Waveshare ESP32-S3-Touch-LCD-1.83, autonome.
//
// Meme cablage que desktop/main.cpp : un Controller tique chaque seconde, son
// cue part sur le haut-parleur, la vue suit view(). Differences propres a la
// board :
//  - l'heure vient du RTC au demarrage (decalage tz_s en NVS), puis des lignes
//    JSON recues sur la console USB (sync_time.py, plus tard le daemon) ;
//  - l'etat persiste en NVS (cle "etat"), ecrit des qu'il change (C9) ;
//  - la vue n'est jamais grisee (C1) ; la luminosite suit la nuit (C11).
// Le controleur et la trame courante ne sont touches que depuis la boucle
// Slint : la console y livre ses lignes, les timers et gestes y tournent.

#include <atomic>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <mutex>
#include <optional>
#include <string>

#include "board_183_sq.h"
#include "data.h"
#include "decode.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "json_line.h"
#include "logic/controller.h"
#include "logic/allure.h"
#include "logic/gestures.h"
#include "logic/lien_pc.h"
#include "logic/volume.h"
#include "view_square.h"
#include "widget.h"

// Sons embarques (EMBED_FILES).
extern const uint8_t wouf_start[] asm("_binary_wouf_wav_start");
extern const uint8_t wouf_end[] asm("_binary_wouf_wav_end");
extern const uint8_t aboie_start[] asm("_binary_aboie_wav_start");
extern const uint8_t aboie_end[] asm("_binary_aboie_wav_end");
extern const uint8_t gemit_start[] asm("_binary_gemit_wav_start");
extern const uint8_t gemit_end[] asm("_binary_gemit_wav_end");
extern const uint8_t jappe_start[] asm("_binary_jappe_wav_start");
extern const uint8_t jappe_end[] asm("_binary_jappe_wav_end");

namespace {

const char *TAG = "chiot";

// Peremption des donnees, en secondes : stale_s de widget.toml.
constexpr int kStaleS = 30;
constexpr int kJournalS = 10;


struct Son {
    const char *nom;
    const uint8_t *start;
    const uint8_t *end;
    uint8_t *buf = nullptr; // copie alignee (malloc), nullptr si absente
    size_t size = 0;
};

// Indexe par pup::Cue (None exclu).
Son g_sons[] = {
    {"wouf", wouf_start, wouf_end},
    {"aboie", aboie_start, aboie_end},
    {"gemit", gemit_start, gemit_end},
    {"jappe", jappe_start, jappe_end},
};

// EMBED_FILES ne garantit aucun alignement du symbole : chaque WAV est copie
// une fois dans un tampon du tas (aligne sur 8 octets), lu en place ensuite.
void charger_sons()
{
    for (Son &s : g_sons) {
        const size_t n = static_cast<size_t>(s.end - s.start);
        s.buf = static_cast<uint8_t *>(malloc(n));
        if (!s.buf) {
            ESP_LOGE(TAG, "son %s : %u octets, malloc a echoue", s.nom, (unsigned)n);
            continue;
        }
        memcpy(s.buf, s.start, n);
        s.size = n;
    }
}

// Niveau de volume (SP5e), lu en NVS au demarrage, change au bouton BOOT ;
// lu et ecrit sur la boucle Slint seulement. 0 = aucun son.
int g_volume = pup::kVolumeDefaut;
// Promenade en cours, pose par la boucle Slint (rafraichir), lue par la tache IMU
// pour le bilan de promenade du podometre (SP5k).
std::atomic<bool> g_en_promenade{false};

void jouer(pup::Cue c)
{
    if (g_volume == 0) return;
    int i = 0;
    switch (c) {
    case pup::Cue::Wouf: i = 0; break;
    case pup::Cue::Aboie: i = 1; break;
    case pup::Cue::Gemit: i = 2; break;
    case pup::Cue::Jappe: i = 3; break;
    case pup::Cue::None: return;
    }
    const Son &s = g_sons[i];
    if (!s.buf) return;
    if (!board_183_sq::play_wav(s.buf, s.size)) ESP_LOGE(TAG, "son %s : wav refuse", s.nom);
}

int64_t mono_s() { return esp_timer_get_time() / 1000000; }

// tz_s ecrit en decimal par nous-memes ; une valeur illisible est ignoree.
bool lire_tz(const std::string &s, long &tz)
{
    if (s.empty()) return false;
    char *fin = nullptr;
    tz = strtol(s.c_str(), &fin, 10);
    return fin && *fin == '\0' && tz > -24 * 3600 && tz < 24 * 3600;
}

struct Etat {
    std::shared_ptr<pup::Controller> ctrl;
    DataFrame courante;     // cles absentes = valeur precedente (decode)
    bool lien = false;      // on_link(true) deja donne
    std::string sauve;      // derniere chaine ecrite en NVS (C9)
    int lum = -1;           // derniere luminosite appliquee
    int64_t journal_s = 0;  // dernier journal periodique
    unsigned invalides = 0; // lignes JSON jetees depuis le demarrage
    // Heure courante de la board (D11) : dernier epoch ecrit ou lu au RTC,
    // avance du temps monotone ecoule depuis. Le controleur n'expose pas la
    // sienne ; celle-ci a la meme origine (RTC au demarrage, puis la console).
    bool heure_ok = false;
    int64_t heure_epoch = 0;
    int64_t heure_mono = 0;
    std::string cfg;        // derniere ligne cfg persistee en NVS (cache)
    std::string tz;         // tz_s stocke en NVS (cache, evite une lecture par ligne)
    int64_t epoch_recu = 0; // dernier epoch recu par la console
    int64_t echec_epoch = 0; // epoch du dernier reglage rate (RTC ou NVS "tz")
    pup::Emetteur emetteur;  // quand emettre la ligne d'etat vers le PC (SPEC-001)
};

// Reglages du detecteur de gestes, copies depuis la boucle Slint vers la tache
// IMU (le detecteur ne vit que dans celle-ci).
struct ReglagesGestes {
    std::mutex m;
    pup::Settings st;
    bool change = false;

    void poser(const pup::Settings &s)
    {
        std::lock_guard<std::mutex> g(m);
        st = s;
        change = true;
    }
};

} // namespace

extern "C" void app_main(void)
{
    board_183_sq::init();
    charger_sons();

    auto view = ViewSquare::create();
    slint::ComponentWeakHandle<ViewSquare> weak(view);

    auto e = std::make_shared<Etat>();
    e->ctrl = std::make_shared<pup::Controller>(kStaleS);
    g_volume = pup::volume_lire(board_183_sq::nvs_load("volume"));
    if (g_volume > 0) board_183_sq::set_volume_db(pup::volume_db(g_volume));
    view->set_volume(g_volume);
    {
        const std::string bilan = board_183_sq::nvs_load("bilan");
        if (!bilan.empty()) printf("derniere promenade %s\n", bilan.c_str());
    }
    const std::string lu = board_183_sq::nvs_load("etat");
    if (!lu.empty() && !e->ctrl->load(lu)) ESP_LOGW(TAG, "etat NVS illisible, ignore");
    e->sauve = e->ctrl->save();

    // Reglages : la ligne cfg persistee en NVS, appliquee avant la premiere
    // vue. Illisible ou partielle : les cles valides passent, le reste garde
    // ses valeurs par defaut.
    e->cfg = board_183_sq::nvs_load("cfg");
    if (!e->cfg.empty()) {
        JsonLine l;
        if (l.parse(e->cfg.data(), e->cfg.size()) && l.has("cfg")) {
            pup::Settings st;
            for (const std::string &err : st.apply(l)) ESP_LOGW(TAG, "cfg NVS : %s", err.c_str());
            e->ctrl->set_settings(st);
        } else {
            ESP_LOGW(TAG, "cfg NVS illisible, reglages par defaut");
        }
    }

    // Le detecteur part avec les reglages charges de la NVS.
    auto gestes = std::make_shared<ReglagesGestes>();
    gestes->poser(e->ctrl->settings());

    // Decode une ligne JSON dans la trame courante et la passe au controleur.
    // Sert aux lignes de la console et a la trame synthetique du demarrage.
    auto pousser = [e](const JsonLine &ligne) {
        decode(ligne, e->courante);
        e->ctrl->on_frame(e->courante, mono_s());
    };

    auto rafraichir = [weak, e] {
        const pup::ViewModel vm = e->ctrl->view();
        g_en_promenade = vm.state == "promenade";
        // Nuit lum_nuit, jour lum_jour, sans horloge lum_jour (C11).
        const pup::Settings &st = e->ctrl->settings();
        const int lum = vm.night && vm.clock_ok ? st.lum_nuit : st.lum_jour;
        if (lum != e->lum) {
            board_183_sq::set_brightness(lum);
            e->lum = lum;
        }
        if (auto v = weak.lock()) {
            apply(**v, vm, true); // jamais grisee (C1)
            (*v)->set_bluetooth(board_183_sq::ble_connected());
        }
    };

    // Lien PC (SPEC-001) : evenements du controleur puis ligne d'etat, vers le daemon par BLE.
    // Les evenements sont toujours vides du controleur ; sans abonne ils sont perdus (le PC
    // relit l'etat a l'abonnement). Un etat non parti est reessaye a la seconde suivante.
    auto vider_lien = [e] {
        for (const pup::Evenement &ev : e->ctrl->take_events())
            board_183_sq::ble_send_line(pup::ligne_evenement(ev));
        if (e->emetteur.a_emettre(e->ctrl->etat_board(), mono_s())) {
            if (!board_183_sq::ble_send_line(pup::ligne_etat(e->ctrl->etat_board())))
                e->emetteur.forcer();
        }
    };

    // Tick, son, ecriture de l'etat s'il a change (C9), puis la vue.
    auto battre = [e, rafraichir, vider_lien] {
        // Batterie faible (SP5h P8) : rien sur secteur ; battery_mv relu toutes les 10 s.
        e->ctrl->on_battery(board_183_sq::on_mains() ? -1 : board_183_sq::battery_mv());
        jouer(e->ctrl->tick(mono_s()));
        const std::string s = e->ctrl->save();
        if (s != e->sauve) {
            // Un echec est journalise par la plateforme ; on garde l'ancienne
            // chaine pour reessayer a la seconde suivante.
            if (board_183_sq::nvs_save("etat", s)) e->sauve = s;
        }
        vider_lien();
        rafraichir();
    };

    // Heure du RTC : trame synthetique {epoch, tz_s}, sans on_link (le lien
    // ne vient que de la console).
    {
        int64_t epoch = 0;
        long tz = 0;
        const bool rtc_ok = board_183_sq::rtc_read(epoch);
        e->tz = board_183_sq::nvs_load("tz");
        const bool tz_ok = lire_tz(e->tz, tz);
        if (rtc_ok) {
            e->heure_ok = true;
            e->heure_epoch = epoch;
            e->heure_mono = mono_s();
        }
        if (rtc_ok && tz_ok) {
            char buf[64];
            const int n = snprintf(buf, sizeof buf, "{\"epoch\":%lld,\"tz_s\":%ld}", (long long)epoch, tz);
            JsonLine ligne;
            if (n > 0 && ligne.parse(buf, (size_t)n)) {
                pousser(ligne);
                ESP_LOGI(TAG, "heure du RTC : epoch=%lld tz_s=%ld", (long long)epoch, tz);
            }
        } else {
            ESP_LOGW(TAG, "pas d'heure au demarrage (rtc=%d tz=%d)", (int)rtc_ok, (int)tz_ok);
        }
    }

    // Console : seules les lignes commencant par '{' sont des trames.
    board_183_sq::on_console_line([e, gestes, pousser, rafraichir, battre](const std::string &txt) {
        if (txt.empty() || txt[0] != '{') return;
        // Commande du PC : au controleur, pas une trame. Inconnue ou incomplete : ignoree.
        if (pup::est_ligne_commande(txt)) {
            if (const auto c = pup::lire_commande(txt)) {
                e->ctrl->on_command(*c, mono_s());
                battre(); // reaction et evenement partent sans attendre la seconde
            } else {
                ESP_LOGW(TAG, "commande inconnue ou incomplete ignoree");
            }
            return;
        }
        JsonLine ligne;
        if (!ligne.parse(txt.data(), txt.size())) {
            ++e->invalides; // compte dans le journal periodique (deux emetteurs melanges ?)
            ESP_LOGW(TAG, "ligne JSON invalide ignoree");
            return;
        }
        // Une ligne cfg n'est jamais une trame : ni decode, ni on_link.
        if (ligne.has("cfg")) {
            pup::Settings st = e->ctrl->settings();
            for (const std::string &err : st.apply(ligne)) ESP_LOGW(TAG, "cfg : %s", err.c_str());
            e->ctrl->set_settings(st);
            gestes->poser(st);
            const std::string nouvelle = st.to_line();
            if (nouvelle != e->cfg) {
                if (board_183_sq::nvs_save("cfg", nouvelle)) {
                    e->cfg = nouvelle;
                    printf("config appliquee\n");
                } else {
                    ESP_LOGE(TAG, "nvs_save(\"cfg\") a echoue");
                }
            }
            rafraichir();
            return;
        }
        if (!e->lien) {
            e->lien = true;
            e->ctrl->on_link(true);
        }
        const bool avec_heure = ligne.has("epoch") && ligne.has("tz_s");
        if (avec_heure) {
            // Memes bornes que lire_tz au demarrage, verifiees avant tout
            // effet : une heure invalide est ignoree (ligne non poussee).
            DataFrame essai = e->courante;
            decode(ligne, essai);
            if (essai.epoch <= 0 || essai.tz_s <= -24 * 3600 || essai.tz_s >= 24 * 3600) {
                ESP_LOGW(TAG, "heure invalide ignoree (epoch=%lld tz_s=%ld)", (long long)essai.epoch,
                         (long)essai.tz_s);
                rafraichir();
                return;
            }
        }
        pousser(ligne);
        if (avec_heure) {
            const int64_t epoch = e->courante.epoch;
            const std::string tz = std::to_string(e->courante.tz_s);
            // D11 : le daemon releve l'heure toutes les secondes mais renvoie la
            // trame toutes les 500 ms ; un epoch deja recu est un vieux releve,
            // pas une heure courante. Seul le passage d'un epoch recu a un autre
            // signale un releve frais (moins de 500 ms) ; le premier recu apres
            // le demarrage peut dater d'une seconde et ne sert que si la board n'a
            // aucune heure. Un releve frais n'ecrit le RTC (et n'accuse) que
            // s'il s'ecarte de plus de 2 s de l'heure de la board ; un tz_s
            // different aussi. "force":1 (sync_time.py) ecrit et accuse toujours.
            const bool force = ligne.integer("force", 0) == 1;
            const bool frais = e->epoch_recu != 0 && epoch != e->epoch_recu;
            e->epoch_recu = epoch;
            int64_t ecart = 0;
            if (e->heure_ok) ecart = epoch - (e->heure_epoch + (mono_s() - e->heure_mono));
            const bool a_regler =
                force || !e->heure_ok || e->tz != tz || (frais && (ecart > 2 || ecart < -2));
            // M5 : un reglage rate (RTC mort, NVS pleine) n'est retente qu'a un
            // nouvel epoch, pas a chaque ligne : un ESP_LOGE par epoch au plus.
            const bool deja_rate = !force && epoch == e->echec_epoch;
            if (!a_regler || deja_rate) {
                rafraichir();
                return;
            }
            board_183_sq::rtc_write(epoch);
            if (e->tz != tz) {
                if (board_183_sq::nvs_save("tz", tz)) {
                    e->tz = tz;
                } else {
                    e->echec_epoch = epoch;
                    ESP_LOGE(TAG, "nvs_save(\"tz\") a echoue (tz_s=%s)", tz.c_str());
                }
            }
            // Relecture apres mise a l'heure (C7) : l'accuse n'est donne que
            // si le RTC rend l'heure envoyee, a 2 s pres.
            int64_t relu = 0;
            if (board_183_sq::rtc_read(relu) && relu >= epoch - 2 && relu <= epoch + 2) {
                e->heure_ok = true;
                e->heure_epoch = epoch;
                e->heure_mono = mono_s();
                printf("heure reglee epoch=%lld\n", (long long)epoch);
            } else {
                e->echec_epoch = epoch;
                ESP_LOGE(TAG, "RTC incoherent apres ecriture (envoye=%lld relu=%lld)", (long long)epoch,
                         (long long)relu);
            }
        }
        rafraichir();
    });

    // Bluetooth (SP5d2) : memes lignes que la console, meme gestionnaire.
    board_183_sq::ble_start("osampo");
    // Le daemon s'abonne a ETAT : l'etat part aussitot (SPEC-001, Lien).
    board_183_sq::on_ble_subscribed([e, vider_lien] {
        e->emetteur.forcer();
        vider_lien();
    });
    // Bouton PWR : l'etat est sauve avant l'extinction (SP5d2, Review Focus 5).
    board_183_sq::power_start([e] {
        const std::string s = e->ctrl->save();
        if (board_183_sq::nvs_save("etat", s)) e->sauve = s;
    });

    // Bouton BOOT : volume suivant (coupe -> 1 -> 2 -> 3), persiste ; un wouf
    // au nouveau niveau pour l'entendre, l'icone haut-parleur s'affiche (SP5e).
    board_183_sq::on_boot_button([weak] {
        g_volume = pup::volume_suivant(g_volume);
        if (!board_183_sq::nvs_save("volume", pup::volume_ecrire(g_volume))) ESP_LOGE(TAG, "nvs_save(\"volume\") a echoue");
        if (g_volume > 0) board_183_sq::set_volume_db(pup::volume_db(g_volume));
        ESP_LOGI(TAG, "volume %d", g_volume);
        if (auto v = weak.lock()) (*v)->set_volume(g_volume);
        jouer(pup::Cue::Wouf);
    });

    // Un geste agit tout de suite, et son cue part sans attendre la seconde.
    auto geste = [e, battre](pup::Input in) {
        e->ctrl->on_input(in, mono_s());
        battre();
    };
    view->on_tapped([geste] { geste(pup::Input::Tap); });
    view->on_long_pressed([geste] { geste(pup::Input::LongPress); });
    view->on_stroked([geste] { geste(pup::Input::Stroke); });
    view->on_anim_done([geste] { geste(pup::Input::AnimDone); });
    view->on_brossed([geste] { geste(pup::Input::Brosse); });
    view->on_os_donne([geste] { geste(pup::Input::Os); });
    view->on_laisse_touchee([geste] { geste(pup::Input::Laisse); });

    // Gestes de l'IMU (G1, G2) : le detecteur tourne dans la tache du capteur,
    // son journal y est imprime ; seul l'Input passe a la boucle Slint, ou il
    // suit le meme chemin que les gestes tactiles. Capteur absent : le chiot
    // marche sans gestes (G11, journalise par la plateforme).
    {
        auto det = std::make_shared<pup::GestureDetector>();
        // Allure du maitre (SP5k) : le podometre suit la meme tache ; la vue
        // n'est mise a jour que quand l'allure ou la duree d'image change.
        auto podo = std::make_shared<pup::Podometre>();
        auto vu = std::make_shared<std::pair<pup::Allure, int>>(pup::Allure::Arret, 0);
        board_183_sq::imu_start([det, podo, vu, gestes, geste, view](const board_183_sq::ImuSample &m) {
            {
                std::lock_guard<std::mutex> g(gestes->m);
                if (gestes->change) {
                    det->set_settings(gestes->st);
                    gestes->change = false;
                }
            }
            const pup::Sample s{m.t_ms, m.ax, m.ay, m.az, m.gx, m.gy, m.gz};
            const std::optional<pup::Input> in = det->feed(s);
            for (const std::string &l : det->take_journal()) printf("%s\n", l.c_str());
            // Bilan de promenade : remis a zero au depart, journalise et garde en NVS a la fin.
            static bool en_promenade = false;
            if (g_en_promenade != en_promenade) {
                en_promenade = g_en_promenade;
                if (en_promenade) {
                    podo->remettre_bilan();
                } else {
                    const std::string b = podo->bilan().texte();
                    printf("bilan promenade %s\n", b.c_str());
                    if (!board_183_sq::nvs_save("bilan", b)) ESP_LOGE(TAG, "nvs_save(\"bilan\") a echoue");
                }
            }
            podo->feed(m.t_ms, m.ax, m.ay, m.az, m.gx, m.gy, m.gz);
            for (const std::string &l : podo->take_journal()) printf("%s\n", l.c_str());
            // En laisse, le chiot marche par defaut (arret ou rien de mesure : pas) ;
            // il ne trotte ou ne court que si le maitre accelere (choix de l'auteur).
            const pup::Allure al = podo->allure() == pup::Allure::Arret ? pup::Allure::Pas : podo->allure();
            const int ms = podo->duree_image_ms(al == pup::Allure::Course ? 6 : 8);
            if (al != vu->first || ms != vu->second) {
                *vu = {al, ms};
                slint::invoke_from_event_loop([view, al, ms] {
                    view->set_allure(pup::allure_nom(al));
                    view->set_marche_image(ms);
                });
            }
            if (in) {
                const pup::Input v = *in;
                const bool droite = det->lancer_droite();
                slint::invoke_from_event_loop([geste, v, droite, view] {
                    if (v == pup::Input::Throw)
                        view->set_vers_la_droite(droite);
                    geste(v);
                });
            }
        });
    }

    slint::Timer battement;
    battement.start(slint::TimerMode::Repeated, std::chrono::seconds(1), [e, battre] {
        battre();
        const int64_t t = mono_s();
        if (t - e->journal_s >= kJournalS) {
            e->journal_s = t;
            const pup::ViewModel vm = e->ctrl->view();
            ESP_LOGI(TAG, "t=%lld etat=%s anim=%s nuit=%d horloge=%d en_ligne=%d besoins=%d/%d/%d vol=%d vbat=%d invalides=%u tas=%u (interne %u)",
                     (long long)t, vm.state.c_str(), vm.anim.c_str(), (int)vm.night, (int)vm.clock_ok, (int)vm.en_ligne,
                     vm.bouger, vm.jouer, vm.dormir, g_volume, board_183_sq::battery_mv(), (unsigned)e->invalides, (unsigned)esp_get_free_heap_size(),
                     (unsigned)heap_caps_get_free_size(MALLOC_CAP_INTERNAL));
        }
    });

    rafraichir();
    printf("chiot lance\n");
    view->run();
}
