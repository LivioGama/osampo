#pragma once
#include "data.h"
#include <atomic>
#include <cstdint>
#include <functional>
#include <thread>

// Produit des DataFrame qui evoluent, depuis un thread.
// En SP3, le transport BLE prendra sa place : meme forme, meme sink.
//
// Scenario (brief Task 7) : epoch reelle + offset_s, tz_s local, sage
// descend d 1 par seconde de 100 a 0 puis remonte a 100 (SP5h), claude_wait vrai 10 s
// toutes les 60 s (de 30 a 39 s dans chaque minute ecoulee).
class FakeSource {
public:
    using Sink = std::function<void(const DataFrame &)>;

    explicit FakeSource(Sink sink, int64_t offset_s = 0);
    ~FakeSource();

    void start();
    void stop();

    // Decalage a ajouter a l epoch reelle pour que l heure locale factice soit
    // hh:mm:00 maintenant.
    static int64_t offset_to(int hh, int mm);

private:
    void loop();

    Sink sink_;
    int64_t offset_s_;
    std::thread thread_;
    std::atomic<bool> running_{false};
};
