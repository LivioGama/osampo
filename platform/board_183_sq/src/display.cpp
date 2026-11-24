#include "display.h"
#include "driver/spi_master.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_vendor.h"
#include "esp_heap_caps.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include "esp_lcd_panel_ops.h"
#include "pins.h"

static const char *TAG = "display";

namespace board_183_sq {

esp_lcd_panel_handle_t display_init()
{
    spi_bus_config_t bus = {};
    bus.sclk_io_num = LCD_SCK;
    bus.mosi_io_num = LCD_MOSI;
    bus.miso_io_num = GPIO_NUM_NC;
    bus.quadwp_io_num = GPIO_NUM_NC;
    bus.quadhd_io_num = GPIO_NUM_NC;
    bus.max_transfer_sz = W * 40 * 2;
    ESP_ERROR_CHECK(spi_bus_initialize(SPI2_HOST, &bus, SPI_DMA_CH_AUTO));

    esp_lcd_panel_io_handle_t io = nullptr;
    esp_lcd_panel_io_spi_config_t io_cfg = {};
    io_cfg.dc_gpio_num = LCD_DC;
    io_cfg.cs_gpio_num = LCD_CS;
    io_cfg.pclk_hz = LCD_HZ;
    io_cfg.lcd_cmd_bits = 8;
    io_cfg.lcd_param_bits = 8;
    io_cfg.spi_mode = 0;
    io_cfg.trans_queue_depth = 10;
    ESP_ERROR_CHECK(esp_lcd_new_panel_io_spi((esp_lcd_spi_bus_handle_t)SPI2_HOST, &io_cfg, &io));

    esp_lcd_panel_handle_t panel = nullptr;
    esp_lcd_panel_dev_config_t pcfg = {};
    pcfg.reset_gpio_num = LCD_RST;
    pcfg.rgb_ele_order = LCD_RGB_ELEMENT_ORDER_RGB;
    pcfg.bits_per_pixel = 16;
    ESP_ERROR_CHECK(esp_lcd_new_panel_st7789(io, &pcfg, &panel));
    ESP_ERROR_CHECK(esp_lcd_panel_reset(panel));
    ESP_ERROR_CHECK(esp_lcd_panel_init(panel));
    // IPS : inversion requise
    ESP_ERROR_CHECK(esp_lcd_panel_invert_color(panel, true));
    ESP_ERROR_CHECK(esp_lcd_panel_set_gap(panel, 0, 0));
    // Ecran noir avant l'allumage : la RAM du panneau contient du bruit au
    // demarrage. Bandes de 40 lignes depuis un tampon de zeros (DMA).
    constexpr int BAND = 40;
    void *zeros = heap_caps_calloc(W * BAND, 2, MALLOC_CAP_DMA);
    if (zeros) {
        for (int y = 0; y < H; y += BAND) {
            const int ye = y + BAND < H ? y + BAND : H;
            if (esp_lcd_panel_draw_bitmap(panel, 0, y, W, ye, zeros) != ESP_OK) break;
        }
        // draw_bitmap en file : attendre la fin des transferts avant de liberer
        vTaskDelay(pdMS_TO_TICKS(50));
        heap_caps_free(zeros);
    } else {
        ESP_LOGW(TAG, "tampon noir : malloc a echoue");
    }
    ESP_ERROR_CHECK(esp_lcd_panel_disp_on_off(panel, true));
    ESP_LOGI(TAG, "ST7789 pret, SPI %d MHz", LCD_HZ / 1000000);
    return panel;
}

} // namespace board_183_sq
