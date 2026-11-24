// Les assert doivent rester actifs, meme en Release.
#undef NDEBUG
#include <cassert>

#include "logic/clock.h"
#include "logic/settings.h"

namespace {

// Calcule avec : date -u -d @1795103600 '+%H %M %u' -> 15 53 4 (jeudi)
// 1795103600 / 86400 = 20776 jours, reste 57200 s = 15:53:20.
void to_local_epoch_zero()
{
    pup::LocalTime t = pup::to_local(0, 0);
    assert(t.day == 0);
    assert(t.minute == 0);
    assert(t.weekday == 3); // 1970-01-01 est un jeudi, 0 = lundi
}

void to_local_with_offset()
{
    pup::LocalTime t = pup::to_local(1795100000, 3600);
    assert(t.day == 20776);
    assert(t.minute == 15 * 60 + 53);
    assert(t.weekday == 3); // %u = 4 (jeudi) -> 4 - 1
}

void in_range_bounds()
{
    const pup::Settings s; // calme 22:00-07:00 par defaut
    assert(!pup::in_range(21 * 60 + 59, s.calme_debut, s.calme_fin));
    assert(pup::in_range(22 * 60, s.calme_debut, s.calme_fin));
    assert(pup::in_range(3 * 60, s.calme_debut, s.calme_fin));
    assert(!pup::in_range(7 * 60, s.calme_debut, s.calme_fin));
}

void clock_not_synced_is_nullopt()
{
    pup::Clock c;
    assert(!c.is_set());
    assert(!c.now(160).has_value());
}

void clock_sync_then_now()
{
    const int64_t e = 1795100000;
    const int32_t tz = 3600;
    pup::Clock c;
    c.sync(e, tz, 100);
    assert(c.is_set());
    std::optional<pup::LocalTime> t = c.now(160);
    assert(t.has_value());
    pup::LocalTime expected = pup::to_local(e + 60, tz);
    assert(t->day == expected.day);
    assert(t->minute == expected.minute);
    assert(t->weekday == expected.weekday);
}

} // namespace

int main()
{
    to_local_epoch_zero();
    to_local_with_offset();
    in_range_bounds();
    clock_not_synced_is_nullopt();
    clock_sync_then_now();
    return 0;
}
