#pragma once
#include "logic/controller.h"
#include <slint.h>

// Le contrat de widget : pousse le ViewModel du controleur dans la vue.
//
// online : la vue montre l icone hors ligne sans lui. En ligne = lien ouvert
// (online, desktop ; toujours vrai sur la board) et donnees du PC fraiches
// (m.en_ligne, SP5d2 B11).
template <typename View>
void apply(View &v, const pup::ViewModel &m, bool online)
{
    v.set_state(slint::SharedString(m.state));
    v.set_reason(slint::SharedString(m.reason));
    v.set_reminder(slint::SharedString(m.reminder));
    v.set_anim(slint::SharedString(m.anim));
    v.set_night(m.night);
    v.set_clock_ok(m.clock_ok);
    v.set_online(online && m.en_ligne);
    v.set_heure(m.heure);
    v.set_minute(m.minute);
    v.set_laisse(m.laisse);
    v.set_bulle_dodo(m.bulle_dodo);
    v.set_batterie_faible(m.batterie_faible);
}
