#pragma once
#include <cstdint>
#include <optional>

namespace pup {

struct LocalTime {
    int64_t day;   // jours depuis epoch local
    int minute;    // minute du jour, 0..1439
    int weekday;   // 0 = lundi
};

LocalTime to_local(int64_t epoch_utc, int32_t tz_s);

class Clock {
public:
    void sync(int64_t epoch_utc, int32_t tz_s, int64_t mono_s);
    bool is_set() const;
    // nullopt tant que jamais synchronisee (B1)
    std::optional<LocalTime> now(int64_t mono_s) const;

private:
    bool set_ = false;
    int64_t epoch_utc_ = 0;
    int32_t tz_s_ = 0;
    int64_t mono_s_ = 0;
};

} // namespace pup
