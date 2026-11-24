#include "pcf85063.h"

namespace board_183_sq {
namespace {

// Algorithmes days_from_civil / civil_from_days (H. Hinnant), domaine public.
int64_t days_from_civil(int64_t y, unsigned m, unsigned d) {
    y -= m <= 2;
    const int64_t era = (y >= 0 ? y : y - 399) / 400;
    const unsigned yoe = static_cast<unsigned>(y - era * 400);
    const unsigned doy = (153 * (m + (m > 2 ? -3 : 9)) + 2) / 5 + d - 1;
    const unsigned doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    return era * 146097 + static_cast<int64_t>(doe) - 719468;
}

void civil_from_days(int64_t z, int64_t &y, unsigned &m, unsigned &d) {
    z += 719468;
    const int64_t era = (z >= 0 ? z : z - 146096) / 146097;
    const unsigned doe = static_cast<unsigned>(z - era * 146097);
    const unsigned yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    y = static_cast<int64_t>(yoe) + era * 400;
    const unsigned doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    const unsigned mp = (5 * doy + 2) / 153;
    d = doy - (153 * mp + 2) / 5 + 1;
    m = mp < 10 ? mp + 3 : mp - 9;
    y += m <= 2;
}

bool bcd_ok(uint8_t v) { return (v & 0x0F) <= 9 && (v >> 4) <= 9; }
int unbcd(uint8_t v) { return (v >> 4) * 10 + (v & 0x0F); }
uint8_t bcd(int v) { return static_cast<uint8_t>(((v / 10) << 4) | (v % 10)); }

bool leap(int y) { return (y % 4 == 0 && y % 100 != 0) || y % 400 == 0; }

}  // namespace

bool pcf_to_epoch(const uint8_t regs[7], int64_t &epoch_utc) {
    if (regs[0] & 0x80) return false;  // OS : l'horloge n'est pas fiable
    const uint8_t raw[7] = {static_cast<uint8_t>(regs[0] & 0x7F),
                            static_cast<uint8_t>(regs[1] & 0x7F),
                            static_cast<uint8_t>(regs[2] & 0x3F),
                            static_cast<uint8_t>(regs[3] & 0x3F),
                            static_cast<uint8_t>(regs[4] & 0x07),
                            static_cast<uint8_t>(regs[5] & 0x1F),
                            regs[6]};
    for (int i = 0; i < 7; ++i)
        if (!bcd_ok(raw[i])) return false;
    const int sec = unbcd(raw[0]), min = unbcd(raw[1]), hour = unbcd(raw[2]);
    const int day = unbcd(raw[3]), mon = unbcd(raw[5]), year = 2000 + unbcd(raw[6]);
    if (sec > 59 || min > 59 || hour > 23) return false;
    if (mon < 1 || mon > 12 || day < 1) return false;
    static const int dim[12] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    const int maxd = dim[mon - 1] + (mon == 2 && leap(year) ? 1 : 0);
    if (day > maxd) return false;
    const int64_t days = days_from_civil(year, static_cast<unsigned>(mon), static_cast<unsigned>(day));
    epoch_utc = days * 86400 + hour * 3600 + min * 60 + sec;
    return true;
}

void epoch_to_pcf(int64_t epoch_utc, uint8_t regs[7]) {
    if (epoch_utc < 946684800LL) epoch_utc = 946684800LL;            // 2000-01-01
    if (epoch_utc > 4102444799LL) epoch_utc = 4102444799LL;          // 2099-12-31 23:59:59
    int64_t days = epoch_utc / 86400;
    const int64_t rem = epoch_utc % 86400;
    int64_t y;
    unsigned m, d;
    civil_from_days(days, y, m, d);
    const int wday = static_cast<int>((days + 4) % 7);  // 1970-01-01 = jeudi (4)
    regs[0] = bcd(static_cast<int>(rem % 60));
    regs[1] = bcd(static_cast<int>(rem / 60 % 60));
    regs[2] = bcd(static_cast<int>(rem / 3600));
    regs[3] = bcd(static_cast<int>(d));
    regs[4] = static_cast<uint8_t>(wday);
    regs[5] = bcd(static_cast<int>(m));
    regs[6] = bcd(static_cast<int>(y - 2000));
}

}  // namespace board_183_sq
