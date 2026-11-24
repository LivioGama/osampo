#include "axp2101.h"

namespace board_183_sq {

bool axp_appui_long(uint8_t intsts2) { return (intsts2 & kAxpPkeyLong) != 0; }

int axp_vbat_mv(uint8_t hi, uint8_t lo) { return ((hi & 0x1F) << 8) | lo; }

uint8_t axp_avec_appui_long(uint8_t reg, uint8_t niveau)
{
    return static_cast<uint8_t>((reg & 0xCF) | ((niveau & 0x03) << 4));
}

} // namespace board_183_sq
