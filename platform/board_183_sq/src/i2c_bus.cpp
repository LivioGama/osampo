#include "i2c_bus.h"
#include "esp_check.h"
#include "esp_log.h"
#include "pins.h"

static const char *TAG = "i2c_bus";

namespace board_183_sq {

i2c_master_bus_handle_t i2c_bus()
{
    static i2c_master_bus_handle_t bus = nullptr;
    if (bus) return bus;
    i2c_master_bus_config_t cfg = {};
    cfg.i2c_port = I2C_NUM_1; // comme le BSP Waveshare
    cfg.sda_io_num = I2C_SDA;
    cfg.scl_io_num = I2C_SCL;
    cfg.clk_source = I2C_CLK_SRC_DEFAULT;
    cfg.glitch_ignore_cnt = 7;
    cfg.flags.enable_internal_pullup = true;
    ESP_ERROR_CHECK(i2c_new_master_bus(&cfg, &bus));
    ESP_LOGI(TAG, "bus I2C pret (%d kHz)", I2C_HZ / 1000);
    return bus;
}

} // namespace board_183_sq
