#pragma once
#include "driver/gpio.h"

// Broches et reglages de la Waveshare ESP32-S3-Touch-LCD-1.83 (verifies au spike).
namespace board_183_sq {
constexpr int W = 240;
constexpr int H = 284;

constexpr gpio_num_t LCD_CS = GPIO_NUM_5;
constexpr gpio_num_t LCD_SCK = GPIO_NUM_6;
constexpr gpio_num_t LCD_DC = GPIO_NUM_4;
constexpr gpio_num_t LCD_MOSI = GPIO_NUM_7;
constexpr gpio_num_t LCD_RST = GPIO_NUM_38;
constexpr gpio_num_t LCD_BL = GPIO_NUM_40;
constexpr int LCD_HZ = 40 * 1000 * 1000;

constexpr gpio_num_t I2C_SDA = GPIO_NUM_15;
constexpr gpio_num_t I2C_SCL = GPIO_NUM_14;
constexpr int I2C_HZ = 400000;
constexpr gpio_num_t TP_RST = GPIO_NUM_39;
constexpr gpio_num_t TP_INT = GPIO_NUM_13;

constexpr int ES8311_ADDR = 0x18;
constexpr int QMI8658_ADDR = 0x6B; // centrale inertielle (SA0 haut)
constexpr gpio_num_t I2S_MCLK = GPIO_NUM_16;
constexpr gpio_num_t I2S_SCLK = GPIO_NUM_9;
constexpr gpio_num_t I2S_LRCK = GPIO_NUM_45;
constexpr gpio_num_t I2S_DOUT = GPIO_NUM_8;
constexpr gpio_num_t PA_EN = GPIO_NUM_46; // ampli, actif haut
} // namespace board_183_sq
