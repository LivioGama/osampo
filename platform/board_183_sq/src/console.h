#pragma once
#include <functional>
#include <string>

namespace board_183_sq {
void console_init(); // installe le pilote USB-Serial/JTAG et la tache de lecture
void console_set_handler(std::function<void(const std::string &)> cb);
// Remet une ligne recue (USB ou Bluetooth) au gestionnaire, sur la boucle Slint.
// Appelable depuis n'importe quelle tache.
void console_deliver(const std::string &line);
} // namespace board_183_sq
