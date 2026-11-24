// Bouton PWR et batterie (SP5d2 B10, B15, B17) : pilote minimal de l'AXP2101.
// Appui long (2 s) sur PWRON -> sauvegarde puis extinction du PMIC. L'appui
// court qui rallume est gere par l'AXP2101 seul (board eteinte).
#include <atomic>
#include <functional>
#include <memory>

#include "axp2101.h"
#include "board_183_sq.h"
#include "driver/i2c_master.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include "i2c_bus.h"
#include "pins.h"
#include "slint.h"
#include "vbat_log.h"

static const char *TAG = "alim";

namespace board_183_sq {
namespace {

constexpr int kTimeoutMs = 100;
constexpr int kScrutationMs = 100;
i2c_master_dev_handle_t g_axp = nullptr;
std::atomic<int> g_vbat{-1};

bool lire(uint8_t reg, uint8_t &val)
{
    return g_axp && i2c_master_transmit_receive(g_axp, &reg, 1, &val, 1, kTimeoutMs) == ESP_OK;
}

bool ecrire(uint8_t reg, uint8_t val)
{
    const uint8_t buf[2] = {reg, val};
    return g_axp && i2c_master_transmit(g_axp, buf, 2, kTimeoutMs) == ESP_OK;
}

bool mettre_bit(uint8_t reg, uint8_t masque)
{
    uint8_t v = 0;
    return lire(reg, v) && ecrire(reg, v | masque);
}

int lire_vbat()
{
    uint8_t h = 0, l = 0;
    if (!lire(kAxpVbatH, h) || !lire(kAxpVbatL, l)) return -1;
    return axp_vbat_mv(h, l);
}

struct Tache {
    std::function<void()> avant;
};

void eteindre(const std::function<void()> &avant)
{
    ESP_LOGI(TAG, "extinction (appui long)");
    // La sauvegarde tourne sur la boucle Slint, comme le reste de l'etat ;
    // on attend sa fin (au plus 2 s) avant de couper.
    SemaphoreHandle_t fini = xSemaphoreCreateBinary();
    slint::invoke_from_event_loop([avant, fini] {
        if (avant) avant();
        xSemaphoreGive(fini);
    });
    if (xSemaphoreTake(fini, pdMS_TO_TICKS(2000)) != pdTRUE) ESP_LOGW(TAG, "sauvegarde non terminee, extinction quand meme");
    ble_stop();
    vTaskDelay(pdMS_TO_TICKS(100)); // laisse partir le journal et la deconnexion
    mettre_bit(kAxpCommon, 0x01);
    // Toujours la si VBUS maintient la board : on reessaie au prochain appui.
    ESP_LOGW(TAG, "extinction refusee par l'AXP2101");
}

// Journal de decharge (SP5h tache 1) : sur batterie, une tension toutes les
// 5 min dans la NVS (720 points = 60 h ; toutes les minutes, la reecriture de
// ~3,6 Ko usait la petite partition NVS, relecture finale I5) ; il repart de zero au debut de chaque decharge (passage secteur
// -> batterie) et il est imprime au demarrage, pour poser le seuil de batterie faible.
VbatLog g_decharge;
std::atomic<bool> g_sur_secteur{true}; // relu par on_mains, depuis la boucle Slint

void noter_decharge(bool point)
{
    uint8_t st = 0;
    if (!lire(kAxpStatus1, st)) return;
    const bool secteur = (st >> 5) & 1;
    if (!secteur) {
        if (g_sur_secteur.load()) g_decharge = VbatLog(); // nouvelle decharge
        const int mv = g_vbat.load();
        if (point && mv > 0) {
            g_decharge.push(static_cast<uint16_t>(mv));
            nvs_save("vbat_log", g_decharge.encode());
        }
    }
    g_sur_secteur = secteur;
}

// Estimation de charge de l'AXP2101 (registre 0xA4 d'apres la fiche technique,
// non verifie) : seulement journalisee, pour juger si elle est utilisable.
int lire_charge_estimee()
{
    uint8_t v = 0;
    return lire(0xA4, v) ? v : -1;
}

void scruter(void *arg)
{
    std::unique_ptr<Tache> t(static_cast<Tache *>(arg));
    int n = 0;
    for (;;) {
        uint8_t sts = 0;
        if (lire(kAxpIntSts2, sts) && sts) {
            ecrire(kAxpIntSts2, sts); // efface (1 = effacer)
            if (axp_appui_long(sts)) eteindre(t->avant);
        }
        if (++n % (10000 / kScrutationMs) == 1) g_vbat = lire_vbat(); // toutes les 10 s
        if (n % (60000 / kScrutationMs) == 0) // secteur relu chaque minute, point toutes les 5 min
            noter_decharge(n % (300000 / kScrutationMs) == 0);
        vTaskDelay(pdMS_TO_TICKS(kScrutationMs));
    }
}

} // namespace

bool power_start(std::function<void()> avant_extinction)
{
    i2c_device_config_t cfg = {};
    cfg.dev_addr_length = I2C_ADDR_BIT_LEN_7;
    cfg.device_address = kAxpAddr;
    cfg.scl_speed_hz = I2C_HZ;
    if (i2c_master_bus_add_device(i2c_bus(), &cfg, &g_axp) != ESP_OK) {
        g_axp = nullptr;
        ESP_LOGE(TAG, "ajout de l'AXP2101 impossible");
        return false;
    }
    uint8_t id = 0, st = 0, on = 0, lvl = 0;
    if (!lire(kAxpChipIdReg, id) || id != kAxpChipId) {
        ESP_LOGE(TAG, "AXP2101 absent (id=0x%02x) : ni bouton PWR ni batterie", id);
        return false;
    }
    lire(kAxpStatus1, st);
    lire(kAxpPwronStatus, on);
    // Appui long a 2 s (B17) ; IRQ d'appui court et long activees, statut efface.
    if (lire(kAxpOffOnLevel, lvl)) ecrire(kAxpOffOnLevel, axp_avec_appui_long(lvl, kAxpIrqLevel2s));
    mettre_bit(kAxpIntEn2, kAxpPkeyLong | kAxpPkeyShort);
    ecrire(kAxpIntSts2, 0xFF);
    mettre_bit(kAxpAdcCtrl, 0x01);
    vTaskDelay(pdMS_TO_TICKS(20));
    g_vbat = lire_vbat();
    ESP_LOGI(TAG, "AXP2101 ok : batterie=%d vbus=%d demarrage=0x%02x vbat=%d mV niveaux=0x%02x", (st >> 3) & 1,
             (st >> 5) & 1, on, g_vbat.load(), lvl);
    {
        const std::string journal = nvs_load("vbat_log");
        VbatLog lu;
        if (lu.decode(journal)) g_decharge = lu;
        ESP_LOGI(TAG, "vbat_log n=%u charge_estimee=%d mv=%s", (unsigned)lu.values().size(), lire_charge_estimee(),
                 journal.c_str());
    }
    xTaskCreate(scruter, "alim", 4096, new Tache{std::move(avant_extinction)}, 2, nullptr);
    return true;
}

int battery_mv() { return g_vbat.load(); }
bool on_mains() { return g_sur_secteur.load(); }

} // namespace board_183_sq
