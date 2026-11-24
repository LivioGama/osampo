#pragma once
#include <cstddef>
#include <cstdint>

namespace board_183_sq {

struct Pcm {
    const int16_t *samples;
    size_t count;  // nombre d'echantillons (int16)
};

// Exige PCM (format 1), 1 canal, 16 kHz, 16 bits. Saute les chunks inconnus.
// `out.samples` pointe dans `data` (aucune copie) : le chunk data doit commencer
// a un offset pair ; `data` doit donc etre aligne sur 2 octets (sinon false
// ou lecture non alignee). Un chunk data tronque est borne a ce qui est present.
bool wav_parse(const uint8_t *data, size_t size, Pcm &out);

}  // namespace board_183_sq
