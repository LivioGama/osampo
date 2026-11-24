#pragma once
#include <cstdint>
#include <deque>
#include <string>
#include <vector>

// Journal de decharge (SP5h tache 1) : tension batterie, une valeur par minute,
// les 720 dernieres (12 h). Texte : valeurs decimales separees par des virgules.
class VbatLog {
public:
    static constexpr size_t kMax = 720;
    void push(uint16_t mv);
    std::string encode() const;
    bool decode(const std::string &s); // false et inchange si invalide
    std::vector<uint16_t> values() const { return {v_.begin(), v_.end()}; }

private:
    std::deque<uint16_t> v_;
};
