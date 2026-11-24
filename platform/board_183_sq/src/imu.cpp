// Pilote QMI8658 (centrale inertielle 6 axes) de la Waveshare ESP32-S3-Touch-LCD-1.83.
//
// Registres d'apres la fiche QMI8658A (QST), section "Register Map" :
//   WHO_AM_I (0x00) = 0x05 ; CTRL1 (0x02) interface ; CTRL2 (0x03) accelerometre
//   (aFS bits 6:4, aODR bits 3:0) ; CTRL3 (0x04) gyroscope (gFS bits 6:4, gODR
//   bits 3:0) ; CTRL5 (0x06) filtres passe-bas ; CTRL7 (0x08) activation
//   (aEN bit 0, gEN bit 1) ; AX_L (0x35) .. GZ_H (0x40) donnees en
//   complement a deux, petit-boutiste ; RESET (0x60) reset logiciel (0xB0).
// Une tache dediee lit les 12 octets AX_L..GZ_H en une transaction a ~100 Hz.

#include "imu.h"

#include "driver/i2c_master.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "i2c_bus.h"
#include "pins.h"

static const char *TAG = "imu";

namespace board_183_sq {
namespace {

constexpr uint8_t REG_WHO_AM_I = 0x00;
constexpr uint8_t WHO_AM_I_QMI8658 = 0x05;
constexpr uint8_t REG_CTRL1 = 0x02;
constexpr uint8_t REG_CTRL2 = 0x03;
constexpr uint8_t REG_CTRL3 = 0x04;
constexpr uint8_t REG_CTRL5 = 0x06;
constexpr uint8_t REG_CTRL7 = 0x08;
constexpr uint8_t REG_AX_L = 0x35;
constexpr uint8_t REG_RESET = 0x60;
constexpr uint8_t RESET_CMD = 0xB0;

// CTRL1 : ADDR_AI (bit 6) = 1, adresse auto-incrementee en lecture multiple ;
// BE (bit 5) = 0, petit-boutiste ; SensorDisable (bit 0) = 0, oscillateur interne actif.
constexpr uint8_t CTRL1_VAL = 0x40;
// CTRL2 : aFS = 010 (+-8 g), aODR = 0110. En mode 6 axes (accel + gyro actifs),
// la cadence suit celle du gyroscope : 0110 = 112,1 Hz, la plus proche au-dessus
// de 100 Hz (0111 = 56,05 Hz). Table "Accelerometer/Gyroscope ODR" de la fiche.
constexpr uint8_t CTRL2_VAL = (0b010 << 4) | 0b0110;
// CTRL3 : gFS = 111 (+-2048 deg/s), gODR = 0110 (112,1 Hz).
constexpr uint8_t CTRL3_VAL = (0b111 << 4) | 0b0110;
// CTRL5 : filtres passe-bas desactives (valeur de reset).
constexpr uint8_t CTRL5_VAL = 0x00;
// CTRL7 : aEN | gEN.
constexpr uint8_t CTRL7_VAL = 0x03;

// Sensibilites (fiche, tables "Accelerometer/Gyroscope specifications") :
// +-8 g -> 4096 LSB/g ; +-2048 deg/s -> 16 LSB/(deg/s).
constexpr float LSB_PAR_G = 4096.0f;
constexpr float LSB_PAR_DPS = 16.0f;

constexpr int TIMEOUT_MS = 50;
constexpr int PERIODE_MS = 10; // ~100 Hz cote tache (capteur a 112,1 Hz)
constexpr int PILE = 6144;     // detecteur, journal (std::string) et printf

i2c_master_dev_handle_t g_dev = nullptr;
std::function<void(const ImuSample &)> g_cb;

bool ecrire(uint8_t reg, uint8_t val)
{
    const uint8_t w[2] = {reg, val};
    return i2c_master_transmit(g_dev, w, 2, TIMEOUT_MS) == ESP_OK;
}

bool lire(uint8_t reg, uint8_t *buf, size_t n)
{
    return i2c_master_transmit_receive(g_dev, &reg, 1, buf, n, TIMEOUT_MS) == ESP_OK;
}

int16_t mot(const uint8_t *p) { return static_cast<int16_t>(p[0] | (p[1] << 8)); }

void tache(void *)
{
    TickType_t reveil = xTaskGetTickCount();
    const TickType_t periode = pdMS_TO_TICKS(PERIODE_MS) > 0 ? pdMS_TO_TICKS(PERIODE_MS) : 1;
    int echecs = 0; // erreurs I2C consecutives, journalisees une fois par serie
    for (;;) {
        vTaskDelayUntil(&reveil, periode);
        uint8_t d[12];
        if (!lire(REG_AX_L, d, sizeof d)) {
            if (echecs++ == 0) ESP_LOGW(TAG, "lecture I2C en echec");
            continue;
        }
        if (echecs > 0) {
            ESP_LOGW(TAG, "lecture I2C revenue apres %d echecs", echecs);
            echecs = 0;
        }
        ImuSample s;
        s.t_ms = esp_timer_get_time() / 1000;
        s.ax = mot(d + 0) / LSB_PAR_G;
        s.ay = mot(d + 2) / LSB_PAR_G;
        s.az = mot(d + 4) / LSB_PAR_G;
        s.gx = mot(d + 6) / LSB_PAR_DPS;
        s.gy = mot(d + 8) / LSB_PAR_DPS;
        s.gz = mot(d + 10) / LSB_PAR_DPS;
        g_cb(s);
    }
}

} // namespace

bool imu_begin(std::function<void(const ImuSample &)> cb)
{
    if (g_dev) {
        ESP_LOGW(TAG, "imu_start deja appele");
        return false;
    }
    i2c_device_config_t cfg = {};
    cfg.dev_addr_length = I2C_ADDR_BIT_LEN_7;
    cfg.device_address = QMI8658_ADDR;
    cfg.scl_speed_hz = I2C_HZ;
    if (i2c_master_bus_add_device(i2c_bus(), &cfg, &g_dev) != ESP_OK) {
        ESP_LOGE(TAG, "ajout du QMI8658 impossible, pas de gestes");
        g_dev = nullptr;
        return false;
    }
    uint8_t id = 0;
    if (!lire(REG_WHO_AM_I, &id, 1) || id != WHO_AM_I_QMI8658) {
        ESP_LOGE(TAG, "IMU absente (0x%02x : WHO_AM_I=0x%02x, attendu 0x%02x), pas de gestes", QMI8658_ADDR,
                 id, WHO_AM_I_QMI8658);
        i2c_master_bus_rm_device(g_dev);
        g_dev = nullptr;
        return false;
    }
    // Reset logiciel, puis configuration complete (CTRL7 en dernier).
    if (!ecrire(REG_RESET, RESET_CMD)) ESP_LOGW(TAG, "reset logiciel en echec");
    vTaskDelay(pdMS_TO_TICKS(20));
    const bool ok = ecrire(REG_CTRL1, CTRL1_VAL) && ecrire(REG_CTRL2, CTRL2_VAL) && ecrire(REG_CTRL3, CTRL3_VAL) &&
                    ecrire(REG_CTRL5, CTRL5_VAL) && ecrire(REG_CTRL7, CTRL7_VAL);
    if (!ok) {
        ESP_LOGE(TAG, "configuration du QMI8658 en echec, pas de gestes");
        i2c_master_bus_rm_device(g_dev);
        g_dev = nullptr;
        return false;
    }
    g_cb = std::move(cb);
    if (xTaskCreate(tache, "imu", PILE, nullptr, 4, nullptr) != pdPASS) {
        ESP_LOGE(TAG, "xTaskCreate a echoue, pas de gestes");
        return false;
    }
    ESP_LOGI(TAG, "QMI8658 pret (0x%02x, +-8 g, +-2048 deg/s, 112 Hz)", QMI8658_ADDR);
    return true;
}

} // namespace board_183_sq
