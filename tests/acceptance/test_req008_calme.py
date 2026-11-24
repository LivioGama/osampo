"""REQ-008 : heures calmes du PC (SPEC-005, SPEC-008)."""
import datetime
import time

import pytest

from outils import attendre, config_toml, hhmm, id_rappel, maintenant


def _fenetre(debut_min, fin_min):
    """Heures calmes allant de now+debut_min a now+fin_min (peut passer minuit)."""
    n = maintenant()
    d = lambda k: hhmm(n + datetime.timedelta(minutes=k))
    return (d(debut_min), d(fin_min))


def _lignes_sons(m):
    return [l.strip() for l in m.stderr_pc().splitlines() if l.startswith("son:")]


# REQ-008 : le daemon dit si le PC est en heures calmes (de calme_debut inclus a calme_fin exclu)
@pytest.mark.parametrize("debut,fin,calme", [
    (-60, 60, True),       # on est dedans
    (60, 120, False),      # plus tard
    (0, 60, True),         # debut inclus
    (-60, 0, False),       # fin exclue
    (120, -120, False),    # de dans 2 h a il y a 2 h : exclut maintenant, passe minuit ou non
])
def test_req008_calme_selon_les_heures(monde, debut, fin, calme):
    c = _fenetre(debut, fin)
    m = monde(config=config_toml(calme=c))
    m.client.attendre_etat(lambda e: e["calme"] is calme, 5, msg=f"calme == {calme} pour {c}")


# REQ-008 : debut = fin : jamais calme
def test_req008_debut_egal_fin_jamais_calme(monde):
    h = hhmm(maintenant())
    m = monde(config=config_toml(calme=(h, h)))
    m.client.attendre_etat(lambda e: e["calme"] is False, 5)


# REQ-008 : un changement des cles vaut des la relecture du daemon
def test_req008_changement_pris_en_compte_a_la_relecture(monde):
    m = monde(config=config_toml(calme=_fenetre(60, 120)))
    m.client.attendre_etat(lambda e: e["calme"] is False, 5)
    m.config(config_toml(calme=_fenetre(-60, 60)))
    m.client.attendre_etat(lambda e: e["calme"] is True, 6, msg="calme apres relecture")
    m.config(config_toml(calme=_fenetre(60, 120)))
    m.client.attendre_etat(lambda e: e["calme"] is False, 6)


# REQ-008 : en heures calmes, la notification de rappel est affichee mais le wouf est muet
def test_req008_notification_sans_son(monde, xvfb):
    n = maintenant()
    h = f"{(n.hour + 5) % 24:02d}:{n.minute:02d}"
    cfg = config_toml([("sortie", h, None)], calme=_fenetre(-60, 60))
    m = monde(pc=True, config=cfg)
    m.board.etat(etat="reclame", raison="rappel", rappel=id_rappel("sortie", h))
    notif = attendre(lambda: m.faux_notif.recues, 5, msg="la notification reste affichee")[0]
    assert notif["hints"]["suppress-sound"] is True
    attendre(lambda: "son: wouf muet (heures calmes)" in _lignes_sons(m), 5, msg=m.stderr_pc())
    attendre(lambda: xvfb.visibles(), 3, msg="la fenetre du chien reste affichee")
    assert "son: wouf" not in _lignes_sons(m)


# REQ-008 : hors heures calmes, le son est joue
def test_req008_hors_heures_calmes_le_son_est_joue(monde):
    n = maintenant()
    h = f"{(n.hour + 5) % 24:02d}:{n.minute:02d}"
    cfg = config_toml([("sortie", h, None)], calme=_fenetre(60, 120))
    m = monde(pc=True, config=cfg)
    m.board.etat(etat="reclame", raison="rappel", rappel=id_rappel("sortie", h))
    attendre(lambda: "son: wouf" in _lignes_sons(m), 5, msg=m.stderr_pc())
    assert "son: wouf muet (heures calmes)" not in _lignes_sons(m)


# REQ-008 : chatouille et os en heures calmes : reaction affichee, jappe muet
@pytest.mark.parametrize("label,cmd", [("Chatouiller", "chatouille"), ("Donner un os", "os")])
def test_req008_gestes_muets(monde, xvfb, label, cmd):
    m = monde(pc=True, config=config_toml(calme=_fenetre(-60, 60)))
    item = m.item()
    m.board.etat(etat="repos")
    m.client.attendre_etat(lambda e: e["calme"] is True and e["etat"] == "repos", 5)
    attendre(lambda: any(e["label"] == label and e["enabled"] for e in item.entrees()), 5)
    item.cliquer(label)
    m.board.attendre_commande(lambda c: c == {"cmd": cmd}, 5)
    attendre(lambda: "son: jappe muet (heures calmes)" in _lignes_sons(m), 3, msg=m.stderr_pc())
    attendre(lambda: xvfb.visibles(), 3, msg="la reaction reste affichee")
    assert "son: jappe" not in _lignes_sons(m)


# REQ-008 : un rappel du daemon ne se declenche pas pendant les heures calmes (rien a notifier)
def test_req008_pas_de_rappel_du_daemon_en_heures_calmes(monde):
    h = hhmm(maintenant())
    m = monde(pc=True, config=config_toml([("sortie", h, None)], calme=_fenetre(-60, 60)))
    time.sleep(4)
    assert m.faux_notif.recues == []
    assert _lignes_sons(m) == []
