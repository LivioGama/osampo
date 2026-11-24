"""REQ-004 : rappel de sortie sur le PC (SPEC-001, SPEC-003, SPEC-005, SPEC-008)."""
import os
import time

import pytest

from outils import attendre, config_toml, hhmm, id_rappel, maintenant


def _cfg_du(categorie="sortie", **kw):
    h = hhmm(maintenant())
    return config_toml([(categorie, h, None)], **kw), h, id_rappel(categorie, h)


def _cfg_futur(categorie="sortie"):
    """Rappel loin de maintenant : le daemon ne le declenche pas ; la board peut le reclamer."""
    n = maintenant()
    h = f"{(n.hour + 5) % 24:02d}:{n.minute:02d}"
    return config_toml([(categorie, h, None)]), h, id_rappel(categorie, h)


def _notifs(m, n=1, timeout=5):
    return attendre(lambda: len(m.faux_notif.recues) >= n and m.faux_notif.recues, timeout,
                    msg="notification de bureau")


# REQ-004 : un rappel de sortie du daemon declenche la notification, au contenu de SPEC-005
@pytest.mark.parametrize("categorie", ["sortie", "sport", "dodo"])
def test_req004_notification_du_rappel(monde, categorie):
    cfg, h, rid = _cfg_du(categorie, report_min=10)
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    assert n["app_name"] == "Osampo"
    assert n["summary"] == "Le chiot veut sortir"
    assert n["body"] == f"Rappel {categorie} de {h}"
    assert n["actions"] == ["accepter", "Accepter", "reporter", "Reporter 10 min"]
    assert n["hints"]["urgency"] == 2
    assert n["hints"]["resident"] is True and n["hints"]["suppress-sound"] is True
    assert n["expire_timeout"] == 0
    assert os.path.isabs(n["app_icon"]) and os.path.exists(n["app_icon"]), n["app_icon"]
    e = m.client.attendre_etat(lambda e: e["rappel_en_cours"] is not None, 5)
    assert e["rappel_en_cours"] == {"id": rid, "categorie": categorie, "heure": h,
                                    "etat": "reclame"}


# REQ-004 : la duree annoncee est report_min
def test_req004_duree_du_report_dans_le_bouton(monde):
    cfg, _h, _ = _cfg_du(report_min=25)
    m = monde(pc=True, config=cfg)
    assert _notifs(m)[0]["actions"][3] == "Reporter 25 min"


# REQ-004 : le son wouf est joue avec la notification
def test_req004_son_wouf_joue(monde):
    cfg, _h, _ = _cfg_du()
    m = monde(pc=True, config=cfg)
    _notifs(m)
    attendre(lambda: "son: wouf\n" in m.stderr_pc() + "\n", 5, msg=m.stderr_pc())


# REQ-004 : un rappel `repas` ne notifie pas le PC
def test_req004_repas_ne_notifie_pas(monde):
    cfg, _h, _ = _cfg_du("repas")
    m = monde(pc=True, config=cfg)
    m.client.attendre_etat(lambda e: e["prochain_rappel"] is not None, 5)
    time.sleep(3)
    assert m.faux_notif.recues == []
    assert m.client.etat()["rappel_en_cours"] is None


# REQ-004 : la notification est celle de la board : la board reclame un rappel que le daemon n'a pas declenche
def test_req004_adopte_le_rappel_reclame_par_la_board(monde):
    cfg, h, rid = _cfg_futur("sortie")
    m = monde(pc=True, config=cfg)
    m.board.etat(etat="reclame", raison="rappel", rappel=rid)
    n = _notifs(m)[0]
    assert n["body"] == f"Rappel sortie de {h}"
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] and e["rappel_en_cours"]["id"] == rid, 2)


# REQ-004 : la reclamation de la jauge sage a 0 notifie aussi
def test_req004_reclamation_de_la_jauge(monde):
    m = monde(pc=True)
    m.board.etat(etat="reclame", raison="promenade")
    n = _notifs(m)[0]
    assert n["body"] == "Sage à 0 : promenade"
    assert n["actions"][:4] == ["accepter", "Accepter", "reporter", "Reporter 10 min"]
    e = m.client.attendre_etat(lambda e: e["rappel_en_cours"], 2)
    assert e["rappel_en_cours"]["id"] == "promenade" and e["rappel_en_cours"]["etat"] == "reclame"


# REQ-004 : la reclamation de la board pour Claude ne notifie pas (hors perimetre SPEC-005)
def test_req004_reclamation_claude_ne_notifie_pas(monde):
    m = monde(pc=True)
    m.board.etat(etat="reclame", raison="claude")
    time.sleep(3)
    assert m.faux_notif.recues == []


# REQ-004 : « accepter » sur le PC modifie le rappel du daemon et la board cesse de reclamer (commande relayee)
def test_req004_accepter_depuis_la_notification(monde):
    cfg, _h, rid = _cfg_du()
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    m.faux_notif.invoquer(n["id"], "accepter")
    c = m.board.attendre_commande(lambda c: c["cmd"] == "accepter", 5)
    assert c["rappel"] == rid
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None, 2)
    attendre(lambda: n["id"] in m.faux_notif.fermees, 2, msg="CloseNotification apres l'action")


# REQ-004 : « reporter » sur le PC : commande relayee, rappel reporte, notification fermee
def test_req004_reporter_depuis_la_notification(monde):
    cfg, _h, rid = _cfg_du(report_min=10)
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    m.faux_notif.invoquer(n["id"], "reporter")
    c = m.board.attendre_commande(lambda c: c["cmd"] == "reporter", 5)
    assert c["rappel"] == rid
    m.client.attendre_etat(lambda e: e["rappel_en_cours"]
                           and e["rappel_en_cours"]["etat"] == "reporte", 2)
    attendre(lambda: n["id"] in m.faux_notif.fermees, 2)


def _menu_du_rappel(m):
    """Menu de l'icone une fois « Accepter le rappel » present et actif."""
    item = m.item()
    attendre(lambda: any(e["label"] == "Accepter le rappel" and e["enabled"]
                         for e in item.entrees()), 5, msg="« Accepter le rappel » dans le menu")
    return item


# REQ-004 (retour de l'auteur 2026-11-22) : notification disparue, le menu de l'icone accepte le rappel
def test_req004_accepter_depuis_le_menu(monde):
    cfg, _h, rid = _cfg_du(report_min=10)
    m = monde(pc=True, config=cfg)
    _notifs(m)
    item = _menu_du_rappel(m)
    assert any(e["label"] == "Reporter 10 min" and e["enabled"] for e in item.entrees())
    item.cliquer("Accepter le rappel")
    c = m.board.attendre_commande(lambda c: c["cmd"] == "accepter", 5)
    assert c["rappel"] == rid
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None, 2)
    attendre(lambda: not any(e["label"] == "Accepter le rappel" for e in item.entrees()), 5,
             msg="entree retiree apres le rappel")


# REQ-004 (retour de l'auteur 2026-11-22) : le menu de l'icone reporte le rappel
def test_req004_reporter_depuis_le_menu(monde):
    cfg, _h, rid = _cfg_du(report_min=10)
    m = monde(pc=True, config=cfg)
    _notifs(m)
    item = _menu_du_rappel(m)
    item.cliquer("Reporter 10 min")
    c = m.board.attendre_commande(lambda c: c["cmd"] == "reporter", 5)
    assert c["rappel"] == rid
    m.client.attendre_etat(lambda e: e["rappel_en_cours"]
                           and e["rappel_en_cours"]["etat"] == "reporte", 2)


# REQ-004 : un rappel reporte revient en reclamation apres report_min : nouvelle notification
@pytest.mark.lent
def test_req004_le_rappel_reporte_revient(monde):
    cfg, _h, _ = _cfg_du(report_min=1)
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    m.faux_notif.invoquer(n["id"], "reporter")
    _notifs(m, 2, timeout=75)
    assert m.client.etat()["rappel_en_cours"]["etat"] == "reclame"


# REQ-004 (inverse) : sortie acceptee sur la board -> notification retiree en moins de 5 s
def test_req004_accepte_sur_la_board_retire_la_notification(monde):
    cfg, _h, rid = _cfg_du()
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    t0 = time.monotonic()
    m.board.evenement("accepte", rappel=rid)
    attendre(lambda: n["id"] in m.faux_notif.fermees, 5, msg="CloseNotification")
    assert time.monotonic() - t0 < 5
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None, 2)


# REQ-004 (inverse) : sortie reportee sur la board -> notification retiree
def test_req004_reporte_sur_la_board_retire_la_notification(monde):
    cfg, _h, rid = _cfg_du()
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    m.board.evenement("reporte", rappel=rid)
    attendre(lambda: n["id"] in m.faux_notif.fermees, 5)
    e = m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None
                               or e["rappel_en_cours"]["etat"] == "reporte", 2)
    assert e["rappel_en_cours"]["etat"] == "reporte"


# REQ-004 (inverse) : la reclamation de la jauge acceptee sur la board retire la notification
def test_req004_jauge_acceptee_sur_la_board(monde):
    m = monde(pc=True)
    m.board.etat(etat="reclame", raison="promenade")
    n = _notifs(m)[0]
    m.board.evenement("accepte", rappel="promenade")
    m.board.etat(etat="promenade", promenade_debut="14:05")
    attendre(lambda: n["id"] in m.faux_notif.fermees, 5)


# REQ-004 : accepter la jauge depuis le PC est relaye a la board
def test_req004_jauge_acceptee_depuis_le_pc(monde):
    m = monde(pc=True)
    m.board.etat(etat="reclame", raison="promenade")
    n = _notifs(m)[0]
    m.faux_notif.invoquer(n["id"], "accepter")
    c = m.board.attendre_commande(lambda c: c["cmd"] == "accepter", 5)
    assert c["rappel"] == "promenade"


# REQ-004 : fermer la notification sans action ne change pas le rappel
def test_req004_fermeture_sans_action_ne_change_rien(monde):
    cfg, _h, _ = _cfg_du()
    m = monde(pc=True, config=cfg)
    n = _notifs(m)[0]
    m.faux_notif.fermer_sans_action(n["id"])
    time.sleep(2)
    assert m.client.etat()["rappel_en_cours"]["etat"] == "reclame"
    assert len(m.faux_notif.recues) == 1, "pas de nouvelle notification"
    assert not [c for c in m.board.commandes() if c["cmd"] in ("accepter", "reporter")]


# REQ-004 : une seule notification par passage en reclamation (pas de rafale)
def test_req004_une_seule_notification(monde):
    cfg, _h, _ = _cfg_du()
    m = monde(pc=True, config=cfg)
    _notifs(m)
    time.sleep(12)
    assert len(m.faux_notif.recues) == 1


# REQ-004 : board hors ligne, le rappel du daemon notifie quand meme ; l'action s'applique au daemon seul
def test_req004_board_hors_ligne_le_rappel_notifie_quand_meme(monde):
    cfg, _h, _ = _cfg_du()
    m = monde(pc=True, board=False, config=cfg)
    n = _notifs(m)[0]
    m.faux_notif.invoquer(n["id"], "accepter")
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None, 3)
    assert m.client.etat()["board"] == "hors_ligne"


# REQ-004 : rappel `accepter` du PC lien coupe, puis lien revenu et board qui reclame encore : la commande lui part
def test_req004_commande_rejouee_au_retour_du_lien(monde):
    cfg, _h, rid = _cfg_du()
    m = monde(pc=True, board=False, config=cfg)
    n = _notifs(m)[0]
    m.faux_notif.invoquer(n["id"], "accepter")
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None, 3)
    b = m.connecter_board()
    b.etat(etat="reclame", raison="rappel", rappel=rid)
    b.attendre_commande(lambda c: c["cmd"] == "accepter" and c["rappel"] == rid, 5)


# REQ-004 : commande accepter dont l'id ne correspond a rien
def test_req004_commande_accepter_id_inconnu(monde):
    m = monde()
    r = m.client.commande("accepter", rappel="sortie,0000")
    assert r["ok"] is False and r["erreur"] == "aucun rappel sortie,0000 en cours"


# REQ-004 : un rappel dû pendant les heures calmes ne se declenche pas
def test_req004_pas_de_rappel_pendant_les_heures_calmes(monde):
    h = hhmm(maintenant())
    cfg = config_toml([("sortie", h, None)], calme=("00:00", "23:59"))
    m = monde(pc=True, config=cfg)
    time.sleep(4)
    assert m.faux_notif.recues == []
    assert m.client.etat()["rappel_en_cours"] is None


# REQ-004 : le rappel se declenche au plus une fois par jour, y compris apres un redemarrage du daemon
def test_req004_une_fois_par_jour_meme_apres_redemarrage(monde):
    cfg, _h, rid = _cfg_du()
    m = monde(config=cfg)
    m.client.attendre_etat(lambda e: e["rappel_en_cours"], 5)
    m.board.evenement("accepte", rappel=rid)
    m.client.attendre_etat(lambda e: e["rappel_en_cours"] is None, 3)
    m.arreter_daemon()
    m.sock.unlink(missing_ok=True)
    m.ble.unlink(missing_ok=True)
    m.demarrer_daemon()
    from outils import ClientPc
    c = ClientPc(m.sock)
    m.clients.append(c)
    c.attendre_etat(lambda e: True, 5)
    time.sleep(3)
    assert c.etat()["rappel_en_cours"] is None
