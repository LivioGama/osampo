#pragma once
#include <cstdint>

namespace board_183_sq {
// PCF85063 (0x51) sur le bus partage. Usage interne ; l'API publique est dans board_183_sq.h.
void rtc_init();                  // cree le device I2C ; apres i2c_bus, avant rtc_get/rtc_set
bool rtc_get(int64_t &epoch_utc); // false si absent, oscillateur arrete ou date invalide
void rtc_set(int64_t epoch_utc);  // force aussi le mode 24 h et l'oscillateur en marche
} // namespace board_183_sq
