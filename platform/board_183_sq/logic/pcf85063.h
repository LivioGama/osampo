#pragma once
#include <cstdint>

namespace board_183_sq {

// Registres 0x04..0x0A du PCF85063 (secondes, minutes, heures, jours,
// jour de semaine, mois, annees), tous en BCD. Annees 2000-2099, heure 24 h
// (le bit 12/24 h de la lecture est ignore : le pilote force le mode 24 h).
// Renvoie false si le bit OS (0x04 bit 7, oscillateur arrete) est leve ou si
// une valeur est hors bornes.
bool pcf_to_epoch(const uint8_t regs[7], int64_t &epoch_utc);

// Jour de semaine : 0 = dimanche. Epoch borne a 2000-01-01..2099-12-31.
void epoch_to_pcf(int64_t epoch_utc, uint8_t regs[7]);

}  // namespace board_183_sq
