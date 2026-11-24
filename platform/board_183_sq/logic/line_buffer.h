#pragma once
#include <cstddef>
#include <functional>
#include <string>

namespace board_183_sq {

// Assemble des lignes terminees par '\n' ; '\r' ignore. Une ligne depassant
// max_len est rejetee en entier (jusqu'au '\n' suivant).
class LineBuffer {
public:
    explicit LineBuffer(size_t max_len = 1024) : max_len_(max_len) {}
    void feed(const char *data, size_t n, const std::function<void(const std::string &)> &on_line);
    // Oublie la ligne en cours (deconnexion : elle ne se recolle pas a la suivante).
    void reset() { buf_.clear(); overflow_ = false; }
    // Lignes rejetees depuis la creation (trop longues), pour le journal.
    size_t rejected() const { return rejected_; }

private:
    size_t max_len_;
    std::string buf_;
    bool overflow_ = false;
    size_t rejected_ = 0;
};

}  // namespace board_183_sq
