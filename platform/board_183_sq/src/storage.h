#pragma once
#include <string>

namespace board_183_sq {
void storage_init(); // nvs_flash_init (efface et reessaie si besoin)
std::string storage_load(const char *key);
bool storage_save(const char *key, const std::string &value);
} // namespace board_183_sq
