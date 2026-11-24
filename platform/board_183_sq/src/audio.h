#pragma once
#include <cstddef>
#include <cstdint>

namespace board_183_sq {
// ES8311 (I2C 0x18) + I2S 16 kHz 16 bits, ampli GPIO 46. Tache de lecture dediee.
void audio_init();
// Non bloquant : depose (samples, count) dans une file d'une case ; une lecture
// en cours est interrompue par le nouvel appel. Le tampon doit rester valide.
void audio_play(const int16_t *samples, size_t count);
} // namespace board_183_sq
