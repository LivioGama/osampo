#include "es8311.h"

#include <cmath>

namespace board_183_sq {

uint8_t es8311_vol_reg(float db)
{
    const long r = 0xBF + std::lround(db * 2.0f);
    return static_cast<uint8_t>(r < 0 ? 0 : r > 0xFF ? 0xFF : r);
}

} // namespace board_183_sq
