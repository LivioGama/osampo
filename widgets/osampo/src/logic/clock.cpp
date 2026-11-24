#include "logic/clock.h"

namespace pup {

namespace {

constexpr int64_t kDaySeconds = 86400;

// Division entiere vers le bas (floor), correcte pour un epoch negatif.
int64_t floor_div(int64_t a, int64_t b)
{
    int64_t q = a / b;
    if ((a % b != 0) && ((a < 0) != (b < 0)))
        --q;
    return q;
}

} // namespace

LocalTime to_local(int64_t epoch_utc, int32_t tz_s)
{
    const int64_t local = epoch_utc + tz_s;
    const int64_t day = floor_div(local, kDaySeconds);
    const int64_t sec_of_day = local - day * kDaySeconds;
    // 1970-01-01 (day 0) est un jeudi : weekday 0 = lundi -> decalage de 3.
    const int64_t wd = ((day + 3) % 7 + 7) % 7;
    return LocalTime{day, static_cast<int>(sec_of_day / 60), static_cast<int>(wd)};
}

void Clock::sync(int64_t epoch_utc, int32_t tz_s, int64_t mono_s)
{
    set_ = true;
    epoch_utc_ = epoch_utc;
    tz_s_ = tz_s;
    mono_s_ = mono_s;
}

bool Clock::is_set() const
{
    return set_;
}

std::optional<LocalTime> Clock::now(int64_t mono_s) const
{
    if (!set_)
        return std::nullopt;
    return to_local(epoch_utc_ + (mono_s - mono_s_), tz_s_);
}

} // namespace pup
