#include "fake_source.h"
#include <chrono>
#include <ctime>

namespace {
// Decalage local, heure d ete comprise, au moment t.
int32_t local_offset(std::time_t t)
{
    std::tm tm{};
    localtime_r(&t, &tm);
    return static_cast<int32_t>(tm.tm_gmtoff);
}
} // namespace

FakeSource::FakeSource(Sink sink, int64_t offset_s) : sink_(std::move(sink)), offset_s_(offset_s) {}

FakeSource::~FakeSource() { stop(); }

int64_t FakeSource::offset_to(int hh, int mm)
{
    const std::time_t now = std::time(nullptr);
    const int64_t local = static_cast<int64_t>(now) + local_offset(now);
    const int64_t sec_of_day = ((local % 86400) + 86400) % 86400;
    return static_cast<int64_t>(hh) * 3600 + mm * 60 - sec_of_day;
}

void FakeSource::start()
{
    if (running_.exchange(true)) {
        return;
    }
    thread_ = std::thread(&FakeSource::loop, this);
}

void FakeSource::stop()
{
    if (!running_.exchange(false)) {
        return;
    }
    if (thread_.joinable()) {
        thread_.join();
    }
}

void FakeSource::loop()
{
    const auto t0 = std::chrono::steady_clock::now();

    while (running_.load()) {
        const int64_t s = std::chrono::duration_cast<std::chrono::seconds>(
                              std::chrono::steady_clock::now() - t0)
                              .count();
        const std::time_t now = std::time(nullptr);
        DataFrame d;
        d.epoch = static_cast<int64_t>(now) + offset_s_;
        d.tz_s = local_offset(now);
        d.sage = 100 - static_cast<int>(s % 101); // 100..0 puis 100
        d.actif = true;
        d.claude_wait = s % 60 >= 30 && s % 60 < 40;
        d.age = 0;

        sink_(d);

        std::this_thread::sleep_for(std::chrono::milliseconds(500));
    }
}
