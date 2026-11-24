#pragma once
#include "esp_lcd_touch.h"

namespace board_183_sq {
// CST816D sur le bus I2C partage. Renvoie nullptr si absent (on continue sans).
esp_lcd_touch_handle_t touch_init();
} // namespace board_183_sq
