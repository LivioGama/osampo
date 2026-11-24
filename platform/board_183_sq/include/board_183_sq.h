#pragma once
#include <cstddef>
#include <cstdint>
#include <functional>
#include <string>

// Plateforme de la Waveshare ESP32-S3-Touch-LCD-1.83, classe `square`.
// Fonctions libres, ni classe abstraite ni virtual.
namespace board_183_sq {

void init();
void set_brightness(int percent);
void play_pcm(const int16_t *samples, size_t count);
// Joue un WAV PCM 16 kHz mono 16 bits (memes regles que play_pcm). `data` doit
// etre aligne sur 2 octets et rester valide pendant la lecture (aucune copie).
// Rend false, sans rien jouer, si le WAV est refuse.
bool play_wav(const uint8_t *data, size_t size);
// Gain du DAC en dB (0 = plein, pas de 0,5 dB) ; au demarrage -6,5 dB.
void set_volume_db(float db);
// Bouton BOOT (GPIO0, actif bas) : cb sur la boucle Slint a chaque appui
// (anti-rebond 40 ms). Rend false si la broche ne se configure pas.
bool on_boot_button(std::function<void()> cb);
bool rtc_read(int64_t &epoch_utc);
void rtc_write(int64_t epoch_utc);
std::string nvs_load(const char *key);
// Rend false si l'ecriture a echoue (l'erreur est journalisee).
bool nvs_save(const char *key, const std::string &value);
// Lignes recues par l'USB et par le Bluetooth, sur la boucle Slint.
void on_console_line(std::function<void(const std::string &)> cb);
// Demarre le Bluetooth (NimBLE, peripherique) : annonce `nom`, accepte un
// central a la fois ; chaque ligne ecrite sur la caracteristique, recollee,
// va au gestionnaire de on_console_line (SP5d2 B3-B5). Rend false si la pile
// ne demarre pas (journalise) : le chiot marche alors sans Bluetooth.
bool ble_start(const char *nom);
// Vrai tant qu'un central (le daemon) est connecte en Bluetooth.
bool ble_connected();
// Envoie `ligne` au daemon par notifications de la caracteristique ETAT
// (7a6d0003-5c3e-4b8e-9d2a-74616d6f7368), en morceaux de 20 octets au plus ; un
// '\n' final est ajoute s'il manque. Rend false sans central abonne, ou si une
// notification est abandonnee (pile saturee). Appelable depuis toute tache.
bool ble_send_line(const std::string &ligne);
// `cb` part sur la boucle Slint quand le daemon s'abonne a ETAT : la board y
// emet son etat aussitot (SPEC-001, Lien).
void on_ble_subscribed(std::function<void()> cb);
// Coupe proprement la connexion Bluetooth (avant extinction).
void ble_stop();
// Bouton PWR (SP5d2 B10, B17) : un appui long de 2 s appelle avant_extinction
// sur la boucle Slint (sauvegarde), puis eteint la board par l'AXP2101 ; un
// appui court la rallume (AXP2101 seul). Rend false si l'AXP2101 ne repond pas.
bool power_start(std::function<void()> avant_extinction);
// Tension batterie en mV, relue toutes les 10 s ; -1 si inconnue.
int battery_mv();
// Vrai sur secteur (VBUS present), relu chaque minute ; vrai avant la premiere lecture.
bool on_mains();

// Echantillon de la centrale inertielle QMI8658 : accelerations en g, vitesses
// angulaires en deg/s, horodatage esp_timer en ms.
struct ImuSample {
    int64_t t_ms;
    float ax, ay, az, gx, gy, gz;
};
// Demarre une tache qui lit l'IMU a ~100 Hz et appelle cb dans cette tache.
// Rend false (et journalise une fois) si le capteur ne repond pas (WHO_AM_I
// different de 0x05) : le chiot marche alors sans gestes (G11).
bool imu_start(std::function<void(const ImuSample &)> cb);

} // namespace board_183_sq
