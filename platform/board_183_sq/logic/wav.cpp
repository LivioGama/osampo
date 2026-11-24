#include "wav.h"

#include <cstring>

namespace board_183_sq {
namespace {
uint32_t rd32(const uint8_t *p) {
    return static_cast<uint32_t>(p[0]) | static_cast<uint32_t>(p[1]) << 8 |
           static_cast<uint32_t>(p[2]) << 16 | static_cast<uint32_t>(p[3]) << 24;
}
uint16_t rd16(const uint8_t *p) { return static_cast<uint16_t>(p[0] | p[1] << 8); }
}  // namespace

bool wav_parse(const uint8_t *data, size_t size, Pcm &out) {
    if (!data || size < 12) return false;
    if (memcmp(data, "RIFF", 4) != 0 || memcmp(data + 8, "WAVE", 4) != 0) return false;
    size_t pos = 12;
    bool have_fmt = false;
    while (pos + 8 <= size) {
        const uint8_t *hdr = data + pos;
        const size_t len = rd32(hdr + 4);
        const size_t body = pos + 8;
        if (len > size - body) {
            // chunk declare plus grand que le tampon : seul data peut etre tronque
            if (memcmp(hdr, "data", 4) != 0) return false;
        }
        if (memcmp(hdr, "fmt ", 4) == 0) {
            if (len < 16 || body + 16 > size) return false;
            if (rd16(data + body) != 1 || rd16(data + body + 2) != 1 ||
                rd32(data + body + 4) != 16000 || rd16(data + body + 14) != 16)
                return false;
            have_fmt = true;
        } else if (memcmp(hdr, "data", 4) == 0) {
            if (!have_fmt) return false;
            if (body % 2 != 0) return false;  // alignement int16
            size_t n = len;
            if (body + n > size) n = size - body;  // chunk tronque : borner
            out.samples = reinterpret_cast<const int16_t *>(data + body);
            out.count = n / 2;
            return true;
        }
        pos = body + len + (len & 1);  // chunks alignes sur 2 octets (len <= size - body : pas de debordement)
    }
    return false;
}

}  // namespace board_183_sq
