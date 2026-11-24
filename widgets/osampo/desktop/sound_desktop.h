#pragma once
#include "logic/reminders.h"

// Joue un cue avec paplay, detache : ne bloque jamais la boucle Slint.
// Cue::None ne joue rien. Les WAV sont dans OSAMPO_SOUNDS_DIR (CMake).
void play(pup::Cue c);
// Niveau 0..3 (SP5e) : 0 = muet, sinon gain de pup::volume_db passe a paplay.
void set_volume(int niveau);
