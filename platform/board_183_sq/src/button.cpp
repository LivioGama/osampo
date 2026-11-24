// Bouton BOOT de la Waveshare 1.83 (GPIO0, rappel au +3,3 V, appui = niveau
// bas) : scrute toutes les 20 ms, un appui compte quand le niveau bas tient
// deux lectures (anti-rebond 40 ms), puis le rappel part sur la boucle Slint.
// Scrutation plutot qu'interruption : rien a faire en ISR, cout negligeable.
#include <functional>
#include <memory>

#include "board_183_sq.h"
#include "driver/gpio.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "slint.h"

static const char *TAG = "bouton";

namespace board_183_sq {
namespace {

constexpr gpio_num_t kBoot = GPIO_NUM_0;
constexpr int kPeriodeMs = 20;

void tache(void *arg)
{
    std::unique_ptr<std::function<void()>> cb(static_cast<std::function<void()> *>(arg));
    int bas = 0;       // lectures basses consecutives
    bool tenu = true;  // vrai tant que le bouton n'a pas ete relache : pas d'appui au demarrage
    for (;;) {
        vTaskDelay(pdMS_TO_TICKS(kPeriodeMs));
        if (gpio_get_level(kBoot) == 0) {
            if (++bas == 2 && !tenu) {
                tenu = true;
                std::function<void()> f = *cb;
                slint::invoke_from_event_loop([f] { f(); });
            }
        } else {
            bas = 0;
            tenu = false;
        }
    }
}

} // namespace

bool on_boot_button(std::function<void()> cb)
{
    gpio_config_t c = {};
    c.pin_bit_mask = 1ULL << kBoot;
    c.mode = GPIO_MODE_INPUT;
    c.pull_up_en = GPIO_PULLUP_ENABLE;
    c.pull_down_en = GPIO_PULLDOWN_DISABLE;
    c.intr_type = GPIO_INTR_DISABLE;
    if (gpio_config(&c) != ESP_OK) {
        ESP_LOGE(TAG, "GPIO0 non configurable, pas de bouton volume");
        return false;
    }
    auto *arg = new std::function<void()>(std::move(cb));
    if (xTaskCreate(tache, "bouton", 3072, arg, 3, nullptr) != pdPASS) {
        delete arg;
        ESP_LOGE(TAG, "tache bouton non creee");
        return false;
    }
    ESP_LOGI(TAG, "bouton BOOT (GPIO0) pret");
    return true;
}

} // namespace board_183_sq
