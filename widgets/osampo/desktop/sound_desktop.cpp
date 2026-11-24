#include "sound_desktop.h"

#include <csignal>
#include <cstdio>
#include <cstring>
#include <string>
#include <spawn.h>
#include <cmath>

#include "logic/volume.h"

extern char **environ;

#ifndef OSAMPO_SOUNDS_DIR
#error "OSAMPO_SOUNDS_DIR doit etre defini par CMake"
#endif

namespace {
const char *cue_name(pup::Cue c)
{
    switch (c) {
    case pup::Cue::Wouf: return "wouf";
    case pup::Cue::Aboie: return "aboie";
    case pup::Cue::Gemit: return "gemit";
    case pup::Cue::Jappe: return "jappe";
    case pup::Cue::None: return nullptr;
    }
    return nullptr;
}
} // namespace

namespace {
int g_niveau = pup::kVolumeDefaut;
}

void set_volume(int niveau) { g_niveau = niveau; }

void play(pup::Cue c)
{
    const char *name = cue_name(c);
    if (!name || g_niveau == 0) return;
    // Pas de zombies : le noyau recolte les enfants, on n attend jamais.
    // Effet de bord : SIGCHLD = SIG_IGN vaut pour tout le processus, aucun
    // autre code ne pourra plus attendre un enfant (waitpid echoue en ECHILD).
    // Acceptable : sur desktop, paplay est le seul enfant.
    static const bool reap = [] {
        std::signal(SIGCHLD, SIG_IGN);
        return true;
    }();
    (void)reap;
    const std::string path = std::string(OSAMPO_SOUNDS_DIR) + "/" + name + ".wav";
    // posix_spawnp plutot que fork : le processus a d autres threads (lien).
    char arg0[] = "paplay";
    // paplay : 65536 = 100 % ; le gain relatif au defaut (-6,5 dB = 100 %).
    const int vol = static_cast<int>(65536.0 * std::pow(10.0, (pup::volume_db(g_niveau) - pup::volume_db(pup::kVolumeDefaut)) / 20.0));
    std::string opt = "--volume=" + std::to_string(vol);
    char *argv[] = {arg0, opt.data(), const_cast<char *>(path.c_str()), nullptr};
    pid_t pid;
    const int err = posix_spawnp(&pid, "paplay", nullptr, nullptr, argv, environ);
    static bool signale = false; // un seul message, pas un par cue
    if (err != 0 && !signale) {
        signale = true;
        std::fprintf(stderr, "son: paplay non lance: %s\n", std::strerror(err));
    }
}
