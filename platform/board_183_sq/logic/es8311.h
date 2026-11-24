#pragma once
#include <cstdint>

namespace board_183_sq {

// Registre 0x32 (volume DAC) de l'ES8311 pour un gain en dB : 0xBF = 0 dB,
// pas de 0,5 dB, borne a 0x00..0xFF (+32 dB). Arrondi au demi-dB le plus proche.
uint8_t es8311_vol_reg(float db);

} // namespace board_183_sq
