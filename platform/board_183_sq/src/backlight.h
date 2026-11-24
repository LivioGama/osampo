#pragma once
namespace board_183_sq {
void backlight_init();            // LEDC 5 kHz 10 bits sur GPIO 40, eteint
void backlight_set(int percent);  // 0..100
} // namespace board_183_sq
