"""REQ-005 : au rappel de sortie, le chien posé sur la barre (REQ-011) prend la pose de la board
(laisse, « aboie » aux relances) ; aucun autre chien n'apparaît (ADR-008, qui remplace ADR-005 ;
SPEC-006, SPEC-005, SPEC-011, ADR-001, ADR-003).

Une seule fenêtre du chien : le chien posé (permanent, WM_CLASS `Osampo`). L'ancienne fenêtre
de sortie (WM_CLASS `OsampoSortie`) ne doit plus jamais apparaître.
"""
import time

import pytest

import xfaux
from outils import attendre, config_toml, hhmm, id_rappel, maintenant

SORTIE = ("osampo-pc", "OsampoSortie")


def _cfg_du(categorie="sortie", extra=""):
    h = hhmm(maintenant())
    return config_toml([(categorie, h, None)], extra=extra), id_rappel(categorie, h)


def _chien(xvfb, timeout=5):
    """Le chien posé (toujours affiché, REQ-011)."""
    return attendre(lambda: (xvfb.visibles() or [None])[0], timeout,
                    msg="fenetre du chien pose visible")


def _anim(xvfb, w):
    try:
        return xvfb.anim(w)
    except Exception:
        return None


def _attendre_anim(xvfb, w, anim, timeout, msg=""):
    attendre(lambda: _anim(xvfb, w) == anim, timeout,
             msg=f"animation {anim!r} attendue ({msg}), vue : {_anim(xvfb, w)!r}")


@pytest.fixture(autouse=True)
def _fond_propre(xvfb):
    """Chaque test part sans fenetre (le osampo-pc precedent est arrete) et sans zone modifiee."""
    xvfb.workarea(xfaux.ZONE)
    attendre(lambda: not xvfb.visibles() and not xvfb.visibles(SORTIE), 5)
    yield
    xvfb.workarea(xfaux.ZONE)


# ============================================================ un seul chien
# REQ-005 (ADR-008) : à chaque rappel sortie, sport ou dodo, aucune fenêtre ne sort du coin :
# seul le chien posé est affiché, et il aboie
@pytest.mark.parametrize("categorie", ["sortie", "sport", "dodo"])
def test_req005_un_seul_chien_au_rappel(monde, xvfb, categorie):
    cfg, _rid = _cfg_du(categorie)
    m = monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5, categorie)
    attendre(lambda: m.faux_notif.recues, 5, msg="notification du rappel")
    fin = time.monotonic() + 3
    while time.monotonic() < fin:
        assert xvfb.visibles(SORTIE) == [], "aucune fenetre de sortie (ADR-008)"
        assert [x.id for x in xvfb.visibles()] == [pose.id], "un seul chien affiche"
        time.sleep(0.2)


# REQ-005 (ADR-008) : la réclamation de la jauge sage à 0 ne fait pas sortir de second chien
def test_req005_un_seul_chien_reclamation_de_la_jauge(monde, xvfb):
    m = monde(pc=True)
    m.board.etat(etat="reclame", raison="promenade", pose="laisse")
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "reclame_laisse", 5)
    time.sleep(2)
    assert xvfb.visibles(SORTIE) == []
    assert [x.id for x in xvfb.visibles()] == [pose.id]


# ============================================================ le chien posé suit la pose de la board
# (ADR-006, ADR-007 : laisse, « aboie » 3 s à chaque relance, « aboie » board hors ligne)
# REQ-005 : le chien posé prend la pose de la board (laisse), sur place, sans être recréé ni retiré
def test_req005_le_chien_pose_suit_la_pose_sur_place(monde, xvfb):
    m = monde(pc=True)
    m.board.etat(etat="repos")
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "repos", 5)
    avant = xvfb.geometrie(pose)
    ids_pose = {x.id for x in xvfb.fenetres()}
    m.board.etat(etat="reclame", raison="promenade", pose="laisse")
    _attendre_anim(xvfb, pose, "reclame_laisse", 5)
    assert xvfb.geometrie(pose) == avant
    assert {x.id for x in xvfb.fenetres()} == ids_pose, "aucune fenetre du chien pose creee"
    assert [x.id for x in xvfb.visibles()] == [pose.id]


# REQ-005 : à chaque rappel sortie, sport ou dodo, le chien posé aboie
@pytest.mark.parametrize("categorie", ["sortie", "sport", "dodo"])
def test_req005_le_chien_pose_aboie_au_rappel(monde, xvfb, categorie):
    cfg, _rid = _cfg_du(categorie)
    monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5, categorie)


# REQ-005 : le chien posé aboie tant que la réclamation dure et ne disparaît pas
def test_req005_le_chien_pose_aboie_tant_que_ca_reclame(monde, xvfb):
    cfg, _rid = _cfg_du()
    monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5)
    fin = time.monotonic() + 6
    while time.monotonic() < fin:
        assert xvfb.visibles(), "le chien pose a disparu pendant la reclamation"
        assert _anim(xvfb, pose) == "aboie"
        time.sleep(0.2)


# REQ-005 : le chien posé ne prend pas le focus pendant le rappel
def test_req005_le_chien_pose_sans_focus(monde, xvfb):
    cfg, _rid = _cfg_du()
    actif = xvfb.active()
    monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5)
    hints = pose.get_wm_hints()
    assert hints is not None and hints.input == 0
    assert xvfb.active() == actif


# REQ-005 : un clic gauche sur le chien posé pendant l'aboiement ne fait rien
def test_req005_clic_sur_le_chien_pose_pendant_l_aboiement(monde, xvfb):
    cfg, _rid = _cfg_du()
    m = monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5)
    time.sleep(1.5)
    xvfb.clic(*xvfb.centre(pose))
    time.sleep(1.5)
    assert xvfb.visibles()
    assert m.board.commandes() == []
    assert m.client.etat()["rappel_en_cours"]["etat"] == "reclame"
    assert _anim(xvfb, pose) == "aboie"


# REQ-005 : une réaction en cours est abandonnée au profit de la pose de la board, qui n'est pas coupée
def test_req005_la_pose_de_la_board_prime_sur_une_reaction(monde, xvfb):
    m = monde(pc=True)
    m.board.etat(etat="repos")
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "repos", 5)
    m.board.evenement("os", source="board")
    m.board.etat(etat="reclame", raison="promenade", pose="laisse")
    _attendre_anim(xvfb, pose, "reclame_laisse", 5)
    fin = time.monotonic() + 2
    while time.monotonic() < fin:
        assert _anim(xvfb, pose) == "reclame_laisse"
        time.sleep(0.1)


# ============================================================ la fin de la réclamation
# REQ-005 : après « accepter » ou « reporter » sur le PC, le chien posé revient à l'état du chiot
# en 2 s, sans disparaître
@pytest.mark.parametrize("action", ["accepter", "reporter"])
def test_req005_fin_apres_action_sur_le_pc(monde, xvfb, action):
    cfg, _rid = _cfg_du()
    m = monde(pc=True, config=cfg)
    m.board.etat(etat="repos")
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5)
    n = attendre(lambda: m.faux_notif.recues, 5, msg="notification du rappel")[0]
    m.faux_notif.invoquer(n["id"], action)
    _attendre_anim(xvfb, pose, "repos", 2, f"apres {action} sur le PC")
    assert xvfb.visibles(), "le chien pose ne doit pas disparaitre"


# REQ-005 : après une action sur la board, le chien posé revient à l'état du chiot en 5 s
@pytest.mark.parametrize("evt", ["accepte", "reporte", "abandonne"])
def test_req005_fin_apres_action_sur_la_board(monde, xvfb, evt):
    cfg, rid = _cfg_du()
    m = monde(pc=True, config=cfg)
    m.board.etat(etat="repos")
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "aboie", 5)
    time.sleep(1.5)
    m.board.evenement(evt, rappel=rid)
    _attendre_anim(xvfb, pose, "repos", 5, f"apres {evt} sur la board")
    assert xvfb.visibles()


# REQ-005 : chaque nouvelle réclamation refait prendre la laisse au chien posé
def test_req005_chaque_reclamation_reprend_la_laisse(monde, xvfb):
    m = monde(pc=True)
    m.board.etat(etat="reclame", raison="promenade", pose="laisse")
    pose = _chien(xvfb)
    _attendre_anim(xvfb, pose, "reclame_laisse", 5)
    m.board.etat(etat="repos")
    _attendre_anim(xvfb, pose, "repos", 5)
    m.board.etat(etat="reclame", raison="promenade", pose="laisse")
    _attendre_anim(xvfb, pose, "reclame_laisse", 5)
    assert xvfb.visibles(SORTIE) == []
