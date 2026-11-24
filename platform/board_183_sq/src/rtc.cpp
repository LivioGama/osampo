#include "rtc.h"
#include "driver/i2c_master.h"
#include "esp_log.h"
#include "i2c_bus.h"
#include "pcf85063.h"
#include "pins.h"

static const char *TAG = "rtc";

namespace board_183_sq {
namespace {

constexpr uint8_t PCF_ADDR = 0x51;
constexpr uint8_t REG_CTRL1 = 0x00;
constexpr uint8_t REG_SECONDS = 0x04;
constexpr uint8_t CTRL1_12_24 = 1 << 1; // 0 = 24 h
constexpr uint8_t CTRL1_STOP = 1 << 5;  // 1 = oscillateur/horloge arretes
constexpr int TIMEOUT_MS = 100;

i2c_master_dev_handle_t g_dev = nullptr;

// Appele une fois par rtc_init (avant tout autre appel).
void add_dev()
{
    i2c_master_dev_handle_t &h = g_dev;
    i2c_device_config_t cfg = {};
    cfg.dev_addr_length = I2C_ADDR_BIT_LEN_7;
    cfg.device_address = PCF_ADDR;
    cfg.scl_speed_hz = I2C_HZ;
    if (i2c_master_bus_add_device(i2c_bus(), &cfg, &h) != ESP_OK) {
        ESP_LOGE(TAG, "ajout du PCF85063 impossible");
        h = nullptr;
    }
}

} // namespace

void rtc_init() { add_dev(); }

bool rtc_get(int64_t &epoch_utc)
{
    auto d = g_dev;
    if (!d) return false;
    const uint8_t reg = REG_SECONDS;
    uint8_t regs[7] = {};
    if (i2c_master_transmit_receive(d, &reg, 1, regs, sizeof regs, TIMEOUT_MS) != ESP_OK) {
        ESP_LOGW(TAG, "lecture I2C en echec");
        return false;
    }
    return pcf_to_epoch(regs, epoch_utc);
}

void rtc_set(int64_t epoch_utc)
{
    auto d = g_dev;
    if (!d) return;
    const uint8_t creg = REG_CTRL1;
    uint8_t c1 = 0;
    if (i2c_master_transmit_receive(d, &creg, 1, &c1, 1, TIMEOUT_MS) != ESP_OK) {
        ESP_LOGW(TAG, "lecture Control_1 en echec");
        return;
    }
    const uint8_t want = c1 & ~(CTRL1_STOP | CTRL1_12_24);
    if (want != c1) {
        const uint8_t w[2] = {REG_CTRL1, want};
        if (i2c_master_transmit(d, w, 2, TIMEOUT_MS) != ESP_OK) ESP_LOGW(TAG, "ecriture Control_1 en echec");
    }
    uint8_t buf[8];
    buf[0] = REG_SECONDS;
    epoch_to_pcf(epoch_utc, buf + 1);
    if (i2c_master_transmit(d, buf, sizeof buf, TIMEOUT_MS) != ESP_OK) ESP_LOGW(TAG, "ecriture de l'heure en echec");
}

} // namespace board_183_sq
