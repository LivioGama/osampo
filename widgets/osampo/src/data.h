#pragma once
#include <cstdint>

// Les valeurs qu un widget affiche, deja decodees.
//
// Tout vient du daemon sauf online : il n est JAMAIS emis, c est l etat du
// lien, decide par le widget lui-meme.
struct DataFrame {
    int64_t epoch = 0;      // UTC, secondes ; 0 = jamais recu
    int32_t tz_s = 0;       // decalage local, heure d ete comprise
    int sage = 100;         // jauge du daemon, 0..100 (SP5h)
    bool actif = false;     // clavier ou souris dans la derniere minute
    int absence_min = 0;    // absence en cours, minutes
    int absence_finie_min = 0;      // derniere absence terminee, minutes
    int absence_finie_age_min = -1; // minutes depuis sa fin, -1 : aucune
    bool claude_wait = false;
    int age = 0;
    bool online = false;    // jamais emis, porte par le widget
};
