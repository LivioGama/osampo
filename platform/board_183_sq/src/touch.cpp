#include "touch.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_touch_cst816s.h"
#include "esp_log.h"
#include "i2c_bus.h"
#include "pins.h"

static const char *TAG = "touch";

namespace board_183_sq {

esp_lcd_touch_handle_t touch_init()
{
    esp_lcd_panel_io_handle_t tp_io = nullptr;
    // ESP_LCD_TOUCH_IO_I2C_CST816S_CONFIG() ne compile pas en C++ (ordre des
    // designateurs) : champs remplis a la main, memes valeurs.
    esp_lcd_panel_io_i2c_config_t io_cfg = {};
    io_cfg.dev_addr = ESP_LCD_TOUCH_IO_I2C_CST816S_ADDRESS;
    io_cfg.scl_speed_hz = I2C_HZ;
    io_cfg.control_phase_bytes = 1;
    io_cfg.lcd_cmd_bits = 8;
    io_cfg.lcd_param_bits = 0;
    io_cfg.flags.disable_control_phase = 1;
    ESP_ERROR_CHECK(esp_lcd_new_panel_io_i2c(i2c_bus(), &io_cfg, &tp_io));
    esp_lcd_touch_config_t tcfg = {};
    tcfg.x_max = W;
    tcfg.y_max = H;
    tcfg.rst_gpio_num = TP_RST;
    tcfg.int_gpio_num = TP_INT;
    esp_lcd_touch_handle_t touch = nullptr;
    esp_err_t e = esp_lcd_touch_new_i2c_cst816s(tp_io, &tcfg, &touch);
    if (e != ESP_OK) {
        ESP_LOGE(TAG, "touch CST816 : %s, on continue sans", esp_err_to_name(e));
        esp_lcd_panel_io_del(tp_io);
        return nullptr;
    }
    ESP_LOGI(TAG, "touch CST816 pret");
    return touch;
}

} // namespace board_183_sq
