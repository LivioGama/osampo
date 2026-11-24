#include "audio.h"
#include "driver/gpio.h"
#include "driver/i2c_master.h"
#include "driver/i2s_std.h"
#include "esp_check.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"
#include "es8311.h"
#include "i2c_bus.h"
#include "pins.h"

static const char *TAG = "audio";

namespace board_183_sq {
namespace {

constexpr uint32_t RATE = 16000;
constexpr uint32_t MCLK_MULT = 256; // MCLK = 4,096 MHz
constexpr float VOLUME_DB = -6.5f;  // volume DAC au demarrage (C10, ancien 70 %), aucun gain logiciel ; reg 0x32 = 0xB2
constexpr int DMA_DESC = 4;
constexpr int DMA_FRAMES = 240;     // 4 x 240 trames = 60 ms en file DMA
constexpr size_t CHUNK = 256;       // trames par ecriture I2S
constexpr int PA_SETTLE_MS = 20;    // apres allumage de l'ampli (anti-claquement)
// Latence d'interruption d'un son : ~76 ms (file DMA 4 x 240 trames a 16 kHz + tampon d'ecriture).
constexpr int PA_HOLD_MS = 100;     // > file DMA : la fin du son sort avant extinction

struct Req {
    const int16_t *samples;
    size_t count;
};

i2c_master_dev_handle_t g_codec = nullptr;
i2s_chan_handle_t g_tx = nullptr;
QueueHandle_t g_queue = nullptr;

esp_err_t wr(uint8_t reg, uint8_t val)
{
    const uint8_t buf[2] = {reg, val};
    esp_err_t e = i2c_master_transmit(g_codec, buf, sizeof buf, 100);
    if (e != ESP_OK) ESP_LOGE(TAG, "ES8311 ecriture reg 0x%02x : %s", reg, esp_err_to_name(e));
    return e;
}

esp_err_t rd(uint8_t reg, uint8_t &val)
{
    esp_err_t e = i2c_master_transmit_receive(g_codec, &reg, 1, &val, 1, 100);
    if (e != ESP_OK) ESP_LOGE(TAG, "ES8311 lecture reg 0x%02x : %s", reg, esp_err_to_name(e));
    return e;
}

esp_err_t rmw(uint8_t reg, uint8_t keep_mask, uint8_t set)
{
    uint8_t v = 0;
    ESP_RETURN_ON_ERROR(rd(reg, v), TAG, "lecture reg 0x%02x", reg);
    return wr(reg, (v & keep_mask) | set);
}

// Sequence de espressif/es8311 1.0.0 (es8311_init + voice_volume_set), reecrite
// sur le pilote i2c_master : le composant utilise l'ancien pilote I2C, et melanger
// les deux pilotes fait avorter IDF 5.5 au demarrage. Esclave I2S, MCLK par la
// broche, 16 bits, 16 kHz avec MCLK 4,096 MHz (coefficients : pre_div 1, mult x1,
// adc/dac_div 1, lrck 0x00ff, bclk_div 4, osr 0x10).
esp_err_t es8311_init()
{
    ESP_RETURN_ON_ERROR(wr(0x00, 0x1F), TAG, "reset"); // reset
    vTaskDelay(pdMS_TO_TICKS(20));
    ESP_RETURN_ON_ERROR(wr(0x00, 0x00), TAG, "reset");
    ESP_RETURN_ON_ERROR(wr(0x00, 0x80), TAG, "power-on");
    ESP_RETURN_ON_ERROR(wr(0x01, 0x3F), TAG, "horloges");  // MCLK broche, horloges actives
    ESP_RETURN_ON_ERROR(rmw(0x06, 0xDF, 0x00), TAG, "");    // SCLK non inverse
    ESP_RETURN_ON_ERROR(rmw(0x02, 0x07, 0x00), TAG, "");    // pre_div 1, mult x1
    ESP_RETURN_ON_ERROR(wr(0x03, 0x10), TAG, "");           // simple vitesse, adc osr
    ESP_RETURN_ON_ERROR(wr(0x04, 0x10), TAG, "");           // dac osr
    ESP_RETURN_ON_ERROR(wr(0x05, 0x00), TAG, "");           // adc_div 1, dac_div 1
    ESP_RETURN_ON_ERROR(rmw(0x06, 0xE0, 0x03), TAG, "");    // bclk_div 4
    ESP_RETURN_ON_ERROR(rmw(0x07, 0xC0, 0x00), TAG, "");    // lrck haut
    ESP_RETURN_ON_ERROR(wr(0x08, 0xFF), TAG, "");           // lrck bas (256)
    ESP_RETURN_ON_ERROR(rmw(0x00, 0xBF, 0x00), TAG, "");    // port serie esclave
    ESP_RETURN_ON_ERROR(wr(0x09, 0x0C), TAG, "");           // SDP in : I2S 16 bits
    ESP_RETURN_ON_ERROR(wr(0x0A, 0x0C), TAG, "");           // SDP out : I2S 16 bits
    ESP_RETURN_ON_ERROR(wr(0x0D, 0x01), TAG, "");           // analogique alimente
    ESP_RETURN_ON_ERROR(wr(0x0E, 0x02), TAG, "");
    ESP_RETURN_ON_ERROR(wr(0x12, 0x00), TAG, "");           // DAC alimente
    ESP_RETURN_ON_ERROR(wr(0x13, 0x10), TAG, "");           // sortie HP
    ESP_RETURN_ON_ERROR(wr(0x1C, 0x6A), TAG, "");
    ESP_RETURN_ON_ERROR(wr(0x37, 0x08), TAG, "");           // egaliseur DAC contourne
    const uint8_t vol = es8311_vol_reg(VOLUME_DB);
    ESP_RETURN_ON_ERROR(wr(0x32, vol), TAG, "volume");
    uint8_t id1 = 0, id2 = 0;
    rd(0xFD, id1);
    rd(0xFE, id2);
    if (id1 != 0x83 || id2 != 0x11) ESP_LOGW(TAG, "ID ES8311 inattendu : %02x%02x (attendu 8311)", id1, id2);
    ESP_LOGI(TAG, "ES8311 init OK (id %02x%02x), volume %.1f dB (reg32=0x%02x)", id1, id2,
             (double)VOLUME_DB, vol);
    return ESP_OK;
}

void i2s_init()
{
    i2s_chan_config_t chan = I2S_CHANNEL_DEFAULT_CONFIG(I2S_NUM_0, I2S_ROLE_MASTER);
    chan.dma_desc_num = DMA_DESC;
    chan.dma_frame_num = DMA_FRAMES;
    chan.auto_clear = true; // silence quand la file est vide
    ESP_ERROR_CHECK(i2s_new_channel(&chan, &g_tx, nullptr));

    i2s_std_config_t std = {};
    std.clk_cfg.sample_rate_hz = RATE;
    std.clk_cfg.clk_src = I2S_CLK_SRC_DEFAULT;
    std.clk_cfg.mclk_multiple = static_cast<i2s_mclk_multiple_t>(MCLK_MULT);
    std.slot_cfg = I2S_STD_PHILIPS_SLOT_DEFAULT_CONFIG(I2S_DATA_BIT_WIDTH_16BIT, I2S_SLOT_MODE_STEREO);
    std.gpio_cfg.mclk = I2S_MCLK;
    std.gpio_cfg.bclk = I2S_SCLK;
    std.gpio_cfg.ws = I2S_LRCK;
    std.gpio_cfg.dout = I2S_DOUT;
    std.gpio_cfg.din = I2S_GPIO_UNUSED;
    ESP_ERROR_CHECK(i2s_channel_init_std_mode(g_tx, &std));
    ESP_ERROR_CHECK(i2s_channel_enable(g_tx)); // MCLK tourne avant l'init du codec
}

void pa_set(bool on) { gpio_set_level(PA_EN, on ? 1 : 0); }

// Joue req jusqu'au bout ou jusqu'a l'arrivee d'une nouvelle requete (renvoyee
// dans next). Retourne true si interrompue.
bool play(const Req &req, Req &next)
{
    static int16_t stereo[CHUNK * 2];
    ESP_LOGI(TAG, "lecture : %u echantillons (%u octets PCM)", (unsigned)req.count,
             (unsigned)(req.count * 2));
    size_t pos = 0, written_total = 0;
    while (pos < req.count) {
        if (xQueueReceive(g_queue, &next, 0) == pdTRUE) {
            ESP_LOGI(TAG, "lecture interrompue a %u/%u echantillons", (unsigned)pos,
                     (unsigned)req.count);
            return true;
        }
        const size_t n = req.count - pos < CHUNK ? req.count - pos : CHUNK;
        for (size_t i = 0; i < n; i++) { // mono duplique sur G et D
            stereo[2 * i] = req.samples[pos + i];
            stereo[2 * i + 1] = req.samples[pos + i];
        }
        size_t w = 0;
        esp_err_t e = i2s_channel_write(g_tx, stereo, n * 4, &w, portMAX_DELAY);
        if (e != ESP_OK) {
            ESP_LOGE(TAG, "i2s_channel_write : %s", esp_err_to_name(e));
            return false;
        }
        written_total += w;
        pos += n;
    }
    ESP_LOGI(TAG, "lecture finie : %u octets I2S ecrits, attendu %u (%u ech. x 2 canaux x 2 o)",
             (unsigned)written_total, (unsigned)(req.count * 4), (unsigned)req.count);
    return false;
}

void audio_task(void *)
{
    Req req;
    for (;;) {
        xQueueReceive(g_queue, &req, portMAX_DELAY);
        pa_set(true);
        vTaskDelay(pdMS_TO_TICKS(PA_SETTLE_MS));
        for (;;) {
            Req next;
            if (play(req, next)) {
                req = next;
                continue;
            }
            // Ampli maintenu le temps de vider la file DMA ; un son qui arrive
            // entre-temps repart sans couper l'ampli.
            if (xQueueReceive(g_queue, &req, pdMS_TO_TICKS(PA_HOLD_MS)) == pdTRUE) continue;
            break;
        }
        pa_set(false);
        ESP_LOGI(TAG, "ampli coupe");
    }
}

} // namespace

void audio_init()
{
    gpio_config_t pa = {};
    pa.pin_bit_mask = 1ULL << PA_EN;
    pa.mode = GPIO_MODE_OUTPUT;
    ESP_ERROR_CHECK(gpio_config(&pa));
    pa_set(false);

    i2c_device_config_t dev = {};
    dev.dev_addr_length = I2C_ADDR_BIT_LEN_7;
    dev.device_address = ES8311_ADDR;
    dev.scl_speed_hz = I2C_HZ;
    ESP_ERROR_CHECK(i2c_master_bus_add_device(i2c_bus(), &dev, &g_codec));

    i2s_init();
    if (es8311_init() != ESP_OK) {
        ESP_LOGE(TAG, "ES8311 : init echouee, pas de son");
        return;
    }
    g_queue = xQueueCreate(1, sizeof(Req));
    if (!g_queue) {
        ESP_LOGE(TAG, "xQueueCreate a echoue, pas de son");
        return;
    }
    if (xTaskCreate(audio_task, "audio", 4096, nullptr, 5, nullptr) != pdPASS) {
        ESP_LOGE(TAG, "xTaskCreate a echoue, pas de son");
        vQueueDelete(g_queue);
        g_queue = nullptr;
    }
}

void audio_play(const int16_t *samples, size_t count)
{
    if (!g_queue || !samples || count == 0) return;
    const Req r{samples, count};
    xQueueOverwrite(g_queue, &r);
}

void set_volume_db(float db)
{
    if (!g_codec) return; // codec absent : rien a regler
    const uint8_t vol = es8311_vol_reg(db);
    if (wr(0x32, vol) != ESP_OK) ESP_LOGW(TAG, "volume %.1f dB non applique", (double)db);
    else ESP_LOGI(TAG, "volume %.1f dB (reg32=0x%02x)", (double)db, vol);
}

} // namespace board_183_sq
