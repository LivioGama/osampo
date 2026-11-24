#include "backlight.h"
#include "driver/ledc.h"
#include "esp_log.h"
#include "pins.h"

static const char *TAG = "backlight";

namespace board_183_sq {

void backlight_init()
{
    ledc_timer_config_t t = {};
    t.speed_mode = LEDC_LOW_SPEED_MODE;
    t.duty_resolution = LEDC_TIMER_10_BIT;
    t.timer_num = LEDC_TIMER_0;
    t.freq_hz = 5000;
    t.clk_cfg = LEDC_AUTO_CLK;
    ESP_ERROR_CHECK(ledc_timer_config(&t));
    ledc_channel_config_t c = {};
    c.gpio_num = LCD_BL;
    c.speed_mode = LEDC_LOW_SPEED_MODE;
    c.channel = LEDC_CHANNEL_0;
    c.timer_sel = LEDC_TIMER_0;
    c.duty = 0;
    c.hpoint = 0;
    ESP_ERROR_CHECK(ledc_channel_config(&c));
}

void backlight_set(int percent)
{
    if (percent < 0) percent = 0;
    if (percent > 100) percent = 100;
    const uint32_t duty = (uint32_t)percent * 1023 / 100;
    esp_err_t e = ledc_set_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_0, duty);
    if (e == ESP_OK) e = ledc_update_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_0);
    if (e != ESP_OK) {
        ESP_LOGE(TAG, "luminosite %d%% : %s", percent, esp_err_to_name(e));
        return;
    }
    ESP_LOGI(TAG, "luminosite %d%% (duty %u/1023)", percent, (unsigned)duty);
}

} // namespace board_183_sq
