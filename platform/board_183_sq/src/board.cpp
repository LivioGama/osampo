#include "board_183_sq.h"
#include "audio.h"
#include "backlight.h"
#include "console.h"
#include "rtc.h"
#include "storage.h"
#include "display.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "i2c_bus.h"
#include "imu.h"
#include "pins.h"
#include "touch.h"
#include "wav.h"
#include "slint-esp.h"
#include "esp_lcd_panel_interface.h"

static const char *TAG = "board_183_sq";

namespace board_183_sq {
namespace {

// Compteur de fps : on enveloppe draw_bitmap du panneau. Slint envoie les
// zones modifiees par bandes de lignes croissantes ; une bande dont y_start
// ne depasse pas la precedente ouvre une nouvelle image. Journal toutes les 10 s.
// Ces compteurs n'ont qu'un seul appelant : la tache de rendu Slint (pas de verrou).
esp_err_t (*g_real_draw)(esp_lcd_panel_t *, int, int, int, int, const void *) = nullptr;
int g_last_y = 1 << 30;
uint32_t g_frames = 0;
int64_t g_t0_us = 0;

esp_err_t counting_draw(esp_lcd_panel_t *panel, int xs, int ys, int xe, int ye, const void *data)
{
    if (ys <= g_last_y) g_frames++;
    g_last_y = ys;
    const int64_t now = esp_timer_get_time();
    if (now - g_t0_us >= 10 * 1000 * 1000) {
        ESP_LOGI(TAG, "fps %.1f (%u images en %.1f s)", g_frames * 1e6 / (now - g_t0_us),
                 (unsigned)g_frames, (now - g_t0_us) / 1e6);
        g_frames = 0;
        g_t0_us = now;
    }
    return g_real_draw(panel, xs, ys, xe, ye, data);
}

} // namespace

void init()
{
    storage_init();
    console_init();
    backlight_init(); // eteint
    esp_lcd_panel_handle_t panel = display_init();
    esp_lcd_touch_handle_t touch = touch_init();
    audio_init();
    rtc_init();

    g_real_draw = panel->draw_bitmap;
    g_t0_us = esp_timer_get_time();
    panel->draw_bitmap = counting_draw;

    SlintPlatformConfiguration<slint::platform::Rgb565Pixel> cfg;
    cfg.size = slint::PhysicalSize({W, H});
    cfg.panel_handle = panel;
    cfg.touch_handle = touch;
    cfg.byte_swap = true; // ST7789 en SPI : RGB565 gros-boutiste
    slint_esp_init(cfg);
    set_brightness(100);
    ESP_LOGI(TAG, "init ok");
}

void set_brightness(int percent) { backlight_set(percent); }
void play_pcm(const int16_t *samples, size_t count) { audio_play(samples, count); }

bool play_wav(const uint8_t *data, size_t size)
{
    Pcm pcm{};
    if (!wav_parse(data, size, pcm)) return false;
    // Les echantillons pointent dans data : un tampon impair donnerait un
    // acces int16 desaligne.
    if (reinterpret_cast<uintptr_t>(pcm.samples) % alignof(int16_t) != 0) return false;
    play_pcm(pcm.samples, pcm.count);
    return true;
}
bool rtc_read(int64_t &epoch_utc) { return rtc_get(epoch_utc); }
void rtc_write(int64_t epoch_utc) { rtc_set(epoch_utc); }
std::string nvs_load(const char *key) { return storage_load(key); }
bool nvs_save(const char *key, const std::string &value) { return storage_save(key, value); }
void on_console_line(std::function<void(const std::string &)> cb) { console_set_handler(std::move(cb)); }
bool imu_start(std::function<void(const ImuSample &)> cb) { return imu_begin(std::move(cb)); }

} // namespace board_183_sq
