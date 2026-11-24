"""REQ-002 : statuts dans l'infobulle et le menu (SPEC-002, SPEC-003, SPEC-004)."""
import datetime

import pytest

from dbus_faux import lignes_statut
from outils import (JOURS, JOURS_COURTS, attendre, config_toml, hhmm, id_rappel, ligne_etat, maintenant)


def _prochain(delta_min, categorie="sortie"):
    """(config, texte attendu) pour un rappel dans delta_min minutes.

    Le rappel n'est actif que le jour de t : sans `jours`, il serait quotidien et sa
    prochaine occurrence tomberait aujourd'hui (SPEC-003, SPEC-007).
    """
    n = maintenant()
    t = n + datetime.timedelta(minutes=delta_min)
    if t.date() == n.date():
        attendu = f"{categorie} à {hhmm(t)}"
    elif t.date() == n.date() + datetime.timedelta(days=1):
        attendu = f"{categorie} demain à {hhmm(t)}"
    else:
        attendu = f"{categorie} {JOURS[t.weekday()]} à {hhmm(t)}"
    return config_toml([(categorie, hhmm(t), [JOURS_COURTS[t.weekday()]])]), attendu


# REQ-002 : la ligne d'etat du daemon porte tous les statuts demandes
def test_req002_daemon_publie_les_statuts(monde):
    m = monde()
    m.board.etat(promenade_debut="14:05", etat="promenade", pile=83)
    e = m.client.attendre_etat(lambda e: e["board"] == "connectee" and e["pile"] == 83, 5)
    assert isinstance(e["sage"], int) and 0 <= e["sage"] <= 100
    assert e["board"] == "connectee"
    assert e["promenade_debut"] == "14:05"
    assert e["pile"] == 83
    assert e["besoins"] == {"bouger": 70, "jouer": 70, "dormir": 70}
    assert "prochain_rappel" in e


# REQ-002 : « l'heure du prochain rappel » (aujourd'hui / demain / plus tard / aucun)
@pytest.mark.parametrize("delta", [30, 24 * 60 + 30, 3 * 24 * 60 + 30])
def test_req002_prochain_rappel_texte(monde, delta):
    cfg, attendu = _prochain(delta)
    m = monde(config=cfg)
    m.client.attendre_etat(lambda e: e["prochain_rappel"] == attendu, 5,
                           msg=f"prochain_rappel == {attendu!r}")


# REQ-002 : le prochain rappel est le plus tot, toutes categories confondues
def test_req002_prochain_rappel_est_le_plus_proche(monde):
    n = maintenant()
    t1, t2 = n + datetime.timedelta(minutes=20), n + datetime.timedelta(minutes=40)
    if t2.date() != n.date():
        pytest.skip("trop pres de minuit pour ce scenario")
    cfg = config_toml([("sport", hhmm(t2), None), ("repas", hhmm(t1), None)])
    m = monde(config=cfg)
    m.client.attendre_etat(lambda e: e["prochain_rappel"] == f"repas à {hhmm(t1)}", 5)


# REQ-002 : aucun rappel a venir
def test_req002_prochain_rappel_aucun_pendant_les_heures_calmes_permanentes(monde):
    # calme_debut != calme_fin sur 24 h moins une minute : le rappel place dans la minute
    # libre n'existe pas ici ; on prend donc des heures calmes qui couvrent tous les rappels.
    cfg = config_toml([("sortie", "10:00", None)], calme=("00:00", "23:59"))
    m = monde(config=cfg)
    n = maintenant()
    if hhmm(n) == "23:59":
        pytest.skip("minute hors des heures calmes")
    m.client.attendre_etat(lambda e: e["prochain_rappel"] == "aucun", 5)


def _lignes_attendues(sage, rappel, promenade, board, pile, besoins, geste):
    return [f"Sage : {sage} %", rappel, promenade, board, pile, besoins, geste]


# REQ-002 : infobulle et menu : formats exacts, dans l'ordre (board connectee)
def test_req002_infobulle_et_menu_board_connectee(monde):
    cfg, attendu = _prochain(30)
    m = monde(pc=True, config=cfg)
    item = m.item()
    m.board.etat(etat="promenade", promenade_debut="14:05", pile=83)
    e = m.client.attendre_etat(lambda e: e["pile"] == 83 and e["board"] == "connectee", 5)
    attendus = [f"Sage : {e['sage']} %", f"Prochain rappel : {attendu}",
                "Promenade : oui, depuis 14:05", "Board : connectée", "Pile : 83 %",
                "Besoins : bouger 70 %, jouer 70 %, dormir 70 %", "Dernier geste : aucun"]
    attendre(lambda: item.tooltip()[1].splitlines()[:7] == attendus, 5,
             msg=f"infobulle {item.tooltip()}")
    assert item.tooltip()[0] == "Osampo"
    attendre(lambda: lignes_statut(item)[:7] == attendus, 5, msg=f"menu {lignes_statut(item)}")
    statuts = [x for x in item.entrees() if not x["separator"]][:7]
    assert all(not s["enabled"] for s in statuts), "les statuts du menu sont non cliquables"


# REQ-002 : promenade « non », aucun prochain rappel, pas de rappel
def test_req002_promenade_non_et_besoins_inconnus_au_demarrage(monde):
    m = monde(pc=True, board=False, config=config_toml(calme=("00:00", "23:59")))
    item = m.item()
    attendre(lambda: "Promenade : non" in item.tooltip()[1], 5, msg=item.tooltip()[1])
    t = item.tooltip()[1]
    assert "Besoins : inconnus" in t
    assert "Prochain rappel : aucun" in t
    assert "Dernier geste : aucun" in t


# REQ-002 : lien coupe -> « Board : hors ligne » et pile « inconnue » (jamais un vieux %)
def test_req002_board_hors_ligne_pile_inconnue(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(pile=83)
    attendre(lambda: "Pile : 83 %" in item.tooltip()[1], 5)
    m.couper_board()
    attendre(lambda: "Board : hors ligne" in item.tooltip()[1], 5)
    t = item.tooltip()
    assert "Pile : inconnue" in t[1] and "Pile : 83 %" not in t[1]
    assert t[0] == "Osampo : board hors ligne"
    assert "Board : hors ligne" in lignes_statut(item)
    assert "Pile : inconnue" in lignes_statut(item)


# REQ-002 : pile -1 envoyee par la board = « inconnue »
def test_req002_pile_moins_un_est_inconnue(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(pile=-1)
    attendre(lambda: "Pile : inconnue" in item.tooltip()[1], 5, msg=item.tooltip()[1])


# REQ-002 : rappel en cours, en attente puis reporte, formats de SPEC-004
def test_req002_rappel_en_cours_dans_l_infobulle(monde):
    n = maintenant()
    h = hhmm(n)
    m = monde(pc=True, config=config_toml([("sport", h, None)]))
    item = m.item()
    attendre(lambda: f"Rappel : sport de {h}, en attente" in item.tooltip()[1], 5,
             msg=item.tooltip()[1])
    m.client.commande("reporter", rappel=id_rappel("sport", h))
    attendre(lambda: f"Rappel : sport de {h}, reporté" in item.tooltip()[1], 5,
             msg=item.tooltip()[1])


# REQ-002 : reclamation de la jauge
def test_req002_reclamation_de_la_jauge_dans_l_infobulle(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="reclame", raison="promenade")
    attendre(lambda: "Rappel : promenade (sage à 0), en attente" in item.tooltip()[1], 5,
             msg=item.tooltip()[1])


# REQ-002 : dernier geste
def test_req002_dernier_geste(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.evenement("os", source="board")
    attendre(lambda: any(
        l.startswith("Dernier geste : os (board) à ") and len(l) == len(
            "Dernier geste : os (board) à 14:12")
        for l in item.tooltip()[1].splitlines()), 5, msg=item.tooltip()[1])
