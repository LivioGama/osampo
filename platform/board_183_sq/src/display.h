#pragma once
#include "esp_lcd_panel_ops.h"

namespace board_183_sq {
// ST7789P en SPI2. Renvoie le panneau (eteint tant que le rétroéclairage l'est).
esp_lcd_panel_handle_t display_init();
} // namespace board_183_sq
