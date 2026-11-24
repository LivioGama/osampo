#include "line_buffer.h"

namespace board_183_sq {

void LineBuffer::feed(const char *data, size_t n,
                      const std::function<void(const std::string &)> &on_line) {
    for (size_t i = 0; i < n; ++i) {
        const char c = data[i];
        if (c == '\r') continue;
        if (c == '\n') {
            if (!overflow_ && !buf_.empty() && on_line) on_line(buf_);
            buf_.clear();
            overflow_ = false;
            continue;
        }
        if (overflow_) continue;
        if (buf_.size() >= max_len_) {
            overflow_ = true;
            ++rejected_;
            buf_.clear();
            continue;
        }
        buf_.push_back(c);
    }
}

}  // namespace board_183_sq
