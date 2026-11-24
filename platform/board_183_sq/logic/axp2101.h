#pragma once
#include <cstdint>

// AXP2101 (PMIC de la 1.83), decodage pur des registres utilises par SP5d2.
// Source des valeurs : XPowersLib (MIT), src/REG/AXP2101Constants.h et
// src/XPowersParams.hpp, lus le 2026-11-20. Le schema Waveshare montre le
// reseau PWRON sur l'AXP2101 ; le cablage exact du bouton est verifie sur la
// board (journal au demarrage).
namespace board_183_sq {

constexpr uint8_t kAxpAddr = 0x34;      // AXP2101_SLAVE_ADDRESS
constexpr uint8_t kAxpChipIdReg = 0x03; // XPOWERS_AXP2101_IC_TYPE
constexpr uint8_t kAxpChipId = 0x4A;    // XPOWERS_AXP2101_CHIP_ID
constexpr uint8_t kAxpStatus1 = 0x00;   // bit 3 batterie presente, bit 5 VBUS bon
constexpr uint8_t kAxpPwronStatus = 0x20; // cause du demarrage (bit 0 PWRON, bit 2 VBUS)
constexpr uint8_t kAxpCommon = 0x10;    // COMMON_CONFIG : bit 0 = extinction
constexpr uint8_t kAxpOffOnLevel = 0x27; // IRQ_OFF_ON_LEVEL_CTRL : bits 5:4 duree d'appui long
constexpr uint8_t kAxpIrqLevel2s = 2;   // XPOWERS_AXP2101_IRQ_TIME_2S
constexpr uint8_t kAxpAdcCtrl = 0x30;   // ADC_CHANNEL_CTRL : bit 0 tension batterie
constexpr uint8_t kAxpVbatH = 0x34;     // ADC_DATA_RELUST0 (5 bits)
constexpr uint8_t kAxpVbatL = 0x35;     // ADC_DATA_RELUST1
constexpr uint8_t kAxpIntEn2 = 0x41;    // INTEN2
constexpr uint8_t kAxpIntSts2 = 0x49;   // INTSTS2, efface en ecrivant 1
constexpr uint8_t kAxpPkeyLong = 0x04;  // PKEY_LONG_IRQ (_BV(10) >> 8)
constexpr uint8_t kAxpPkeyShort = 0x08; // PKEY_SHORT_IRQ (_BV(11) >> 8)

// Vrai si le statut INTSTS2 signale un appui long sur PWRON.
bool axp_appui_long(uint8_t intsts2);
// Tension batterie en mV (1 mV par pas, comme getBattVoltage de XPowersLib).
int axp_vbat_mv(uint8_t hi, uint8_t lo);
// Registre 0x27 avec la duree d'appui long remplacee (bits 5:4).
uint8_t axp_avec_appui_long(uint8_t reg, uint8_t niveau);

} // namespace board_183_sq
