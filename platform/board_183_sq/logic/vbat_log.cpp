#include "vbat_log.h"

void VbatLog::push(uint16_t mv)
{
    v_.push_back(mv);
    if (v_.size() > kMax) v_.pop_front();
}

std::string VbatLog::encode() const
{
    std::string s;
    for (uint16_t x : v_) {
        if (!s.empty()) s += ',';
        s += std::to_string(x);
    }
    return s;
}

bool VbatLog::decode(const std::string &s)
{
    std::deque<uint16_t> lu;
    size_t i = 0;
    while (i < s.size()) {
        size_t j = s.find(',', i);
        if (j == std::string::npos) j = s.size();
        if (j == i || j - i > 5) return false;
        unsigned long x = 0;
        for (size_t k = i; k < j; ++k) {
            if (s[k] < '0' || s[k] > '9') return false;
            x = x * 10 + static_cast<unsigned long>(s[k] - '0');
        }
        if (x > 65535) return false;
        lu.push_back(static_cast<uint16_t>(x));
        i = j + 1;
        if (j < s.size() && i == s.size()) return false; // virgule finale
    }
    if (lu.size() > kMax) return false;
    v_ = std::move(lu);
    return true;
}
