"""REQ-005 et REQ-012 : à chaque relance d'un rappel en laisse, le chien du PC (chien posé et icône)
suit la pose de la board : « aboie » pendant la relance, puis retour à la laisse (ADR-006,
ADR-007, SPEC-012 « Relances en laisse », SPEC-002, SPEC-006, SPEC-011).

La board (firmware) n'est pas pilotable ici : la fausse board envoie les lignes d'état qu'elle
enverrait, avec le champ `pose`. Les 3 s de la relance côté board ne sont pas testables ici ; on
vérifie ce que le PC fait de chaque changement de `pose` (en moins de 5 s, jamais par « repos »).
"""
import time

import pytest

import xfaux
from outils import attendre, config_toml, hhmm, id_rappel, maintenant

SORTIE = ("osampo-pc", "OsampoSortie")
EN_LAISSE = [("rappel", "sortie,1230"), ("rappel", "sport,1230"), ("rappel", "dodo,1230"),
             ("promenade", "-")]


@pytest.fixture(autouse=True)
def _fond_propre(xvfb):
    xvfb.workarea(xfaux.ZONE)
    attendre(lambda: not xvfb.visibles() and not xvfb.visibles(SORTIE), 5)
    yield
    xvfb.workarea(xfaux.ZONE)


def _anim(xvfb, w):
    try:
        return xvfb.anim(w)
    except Exception:
        return None


def _chien(xvfb):
    return attendre(lambda: (xvfb.visibles() or [None])[0], 5, msg="chien pose visible")


def _vers(xvfb, w, anim, msg="", timeout=5, premier=False):
    """Attend l'animation `anim`. Sauf `premier` (départ du repos), l'animation ne passe jamais par
    « repos » entre deux poses de réclamation."""
    vues = []

    def atteint():
        vues.append(_anim(xvfb, w))
        return vues[-1] == anim
    try:
        attendre(atteint, timeout, pas=0.02, msg=f"{anim!r} attendue ({msg})")
    except AssertionError as e:
        raise AssertionError(f"{e} ; vues {vues[-6:]}") from None
    if not premier:
        assert "repos" not in vues, f"passage par repos ({msg}) : {vues}"


def _rappel_du_jour(categorie="sortie"):
    h = hhmm(maintenant())
    return config_toml([(categorie, h, None)]), id_rappel(categorie, h)


# ============================================================ chien posé
# REQ-005 : à chaque relance (jusqu'à 3), le chien posé passe de la laisse à « aboie » puis revient
# à la laisse, sans disparaître ; aucun second chien n'apparaît (ADR-008)
@pytest.mark.parametrize("categorie", ["sortie", "sport", "dodo"])
def test_req005_chien_pose_aboie_a_chaque_relance(monde, xvfb, categorie):
    cfg, rid = _rappel_du_jour(categorie)
    m = monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="laisse")
    _vers(xvfb, pose, "reclame_laisse", "laisse", premier=True)
    for n in (1, 2, 3):
        m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="aboie")
        _vers(xvfb, pose, "aboie", f"relance {n}")
        assert xvfb.visibles(), f"le chien pose a disparu a la relance {n}"
        m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="laisse")
        _vers(xvfb, pose, "reclame_laisse", f"fin de la relance {n}")
        assert xvfb.visibles(SORTIE) == [], f"fenetre de sortie a la relance {n} (ADR-008)"


# REQ-005 / REQ-011 : même suivi pour la jauge vide et pour chaque rappel en laisse, sans « repos »
@pytest.mark.parametrize("raison,rappel", EN_LAISSE)
def test_req005_req011_relances_laisse_aboie_laisse(monde, xvfb, raison, rappel):
    m = monde(pc=True)
    w = _chien(xvfb)
    premier = True
    suite = [("laisse", "reclame_laisse")] + [("aboie", "aboie"), ("laisse", "reclame_laisse")] * 3
    for pose, anim in suite:
        m.board.etat(etat="reclame", raison=raison, rappel=rappel, pose=pose)
        _vers(xvfb, w, anim, f"{raison}/{pose}", premier=premier)
        premier = False
        assert xvfb.visibles()


# REQ-005 : la réclamation prend fin pendant la relance (fin sur la board) : chien
# posé revenu à l'état du chiot, sans garder « aboie »
@pytest.mark.parametrize("evt", ["accepte", "reporte", "abandonne"])
def test_req005_fin_du_rappel_pendant_une_relance(monde, xvfb, evt):
    cfg, rid = _rappel_du_jour()
    m = monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="aboie")
    _vers(xvfb, pose, "aboie", "relance", premier=True)
    m.board.evenement(evt, rappel=rid)
    m.board.etat(etat="repos")
    attendre(lambda: _anim(xvfb, pose) == "repos", 5, msg=f"repos apres {evt}")
    assert xvfb.visibles()


# REQ-005 : accepté sur le PC pendant la relance : même résultat
def test_req005_accepte_sur_le_pc_pendant_une_relance(monde, xvfb):
    cfg, rid = _rappel_du_jour()
    m = monde(pc=True, config=cfg)
    pose = _chien(xvfb)
    m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="aboie")
    _vers(xvfb, pose, "aboie", "relance", premier=True)
    n = attendre(lambda: m.faux_notif.recues, 5, msg="notification du rappel")[0]
    m.faux_notif.invoquer(n["id"], "accepter")
    m.board.etat(etat="repos")
    attendre(lambda: _anim(xvfb, pose) == "repos", 5, msg="repos apres accepter")


# REQ-005 / REQ-011 : la jauge finit pendant la relance, ou le chiot s'endort : état du chiot
def test_req005_fin_de_la_jauge_et_dodo_pendant_une_relance(monde, xvfb):
    m = monde(pc=True)
    w = _chien(xvfb)
    m.board.etat(etat="reclame", raison="promenade", pose="aboie")
    _vers(xvfb, w, "aboie", "relance de la jauge", premier=True)
    m.board.etat(etat="repos")
    attendre(lambda: _anim(xvfb, w) == "repos", 5, msg="repos")
    m.board.etat(etat="reclame", raison="rappel", rappel="dodo,1230", pose="aboie")
    _vers(xvfb, w, "aboie", "relance du dodo", premier=True)
    # la board qui entre en dodo abandonne le rappel en cours (SPEC-001, evt abandonne), sinon le
    # daemon garde le rappel adopté en `reclame` et le PC aboie (SPEC-002 règle 2, SPEC-003)
    m.board.evenement("abandonne", rappel="dodo,1230")
    m.board.etat(etat="dort")
    attendre(lambda: _anim(xvfb, w) == "dort", 5, msg="le chiot s'endort pendant la relance")


# REQ-005 : une réclamation en laisse qui remplace l'aboiement de Claude prend la laisse (pas
# l'aboie précédent), puis la relance suivante repasse à « aboie »
def test_req005_laisse_remplace_claude_puis_relance(monde, xvfb):
    m = monde(pc=True)
    w = _chien(xvfb)
    m.board.etat(etat="reclame", raison="claude", pose="aboie")
    _vers(xvfb, w, "aboie", "Claude, apres le delai", premier=True)
    m.board.etat(etat="reclame", raison="rappel", rappel="sortie,1230", pose="laisse")
    _vers(xvfb, w, "reclame_laisse", "rappel de sortie sur Claude")
    m.board.etat(etat="reclame", raison="rappel", rappel="sortie,1230", pose="aboie")
    _vers(xvfb, w, "aboie", "relance")


# REQ-005 : une réclamation en laisse remplace l'aboie de la relance d'une autre (jauge puis rappel)
def test_req005_rappel_remplace_la_relance_de_la_jauge(monde, xvfb):
    m = monde(pc=True)
    w = _chien(xvfb)
    m.board.etat(etat="reclame", raison="promenade", pose="aboie")
    _vers(xvfb, w, "aboie", "relance de la jauge", premier=True)
    m.board.etat(etat="reclame", raison="rappel", rappel="sortie,1230", pose="laisse")
    _vers(xvfb, w, "reclame_laisse", "le rappel remplace la jauge, avec sa propre pose")


# ============================================================ board hors ligne
# REQ-005 : board hors ligne dès le départ : le chien posé aboie, sans disparaître
def test_req005_board_hors_ligne_le_chien_pose_aboie(monde, xvfb):
    cfg, _rid = _rappel_du_jour()
    monde(pc=True, board=False, config=cfg)
    w = _chien(xvfb)
    attendre(lambda: _anim(xvfb, w) == "aboie", 5, msg="aboie, board hors ligne")
    assert xvfb.visibles()


# REQ-005 : le lien se coupe en pleine laisse ou en pleine relance : « aboie » ; il revient : la
# pose de la board
def test_req005_coupure_du_lien_pendant_laisse_et_relance(monde, xvfb):
    cfg, rid = _rappel_du_jour()
    m = monde(pc=True, config=cfg)
    w = _chien(xvfb)
    m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="laisse")
    _vers(xvfb, w, "reclame_laisse", "laisse", premier=True)
    m.couper_board()
    attendre(lambda: _anim(xvfb, w) == "aboie", 6, msg="lien coupe pendant la laisse")
    m.connecter_board().etat(etat="reclame", raison="rappel", rappel=rid, pose="laisse")
    attendre(lambda: _anim(xvfb, w) == "reclame_laisse", 6, msg="lien revenu, board en laisse")
    m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="aboie")
    _vers(xvfb, w, "aboie", "relance")
    m.couper_board()
    attendre(lambda: _anim(xvfb, w) == "aboie", 6, msg="lien coupe pendant la relance")
    assert xvfb.visibles()


# ============================================================ icône (REQ-012)
# REQ-012 : à chaque relance en laisse, l'icône passe de « reclame_laisse » à « aboie » puis revient,
# chaque passage en moins de 5 s, jamais par « assis »
@pytest.mark.parametrize("raison,rappel", EN_LAISSE)
def test_req012_icone_aboie_a_chaque_relance_en_laisse(monde, raison, rappel):
    m = monde(pc=True)
    item = m.item()

    def icone():
        return str(item.get("IconName"))

    def envoyer(p):
        m.board.etat(etat="reclame", raison=raison, rappel=rappel, pose=p)

    def vers(nom, msg, premier=False):
        vues = []
        try:
            attendre(lambda: vues.append(icone()) or vues[-1] == nom, 5, pas=0.02,
                     msg=f"{msg} : {nom}")
        except AssertionError as e:
            raise AssertionError(f"{e} ; vues {vues[-5:]}") from None
        if not premier:
            assert "osampo-assis" not in vues, vues

    envoyer("laisse")
    vers("osampo-reclame_laisse", "laisse", premier=True)
    for n in (1, 2, 3):
        envoyer("aboie")
        vers("osampo-aboie", f"relance {n}")
        m.client.attendre_etat(lambda e: e["icone"] == "aboie", 1)
        envoyer("laisse")
        vers("osampo-reclame_laisse", f"fin de la relance {n}")
        m.client.attendre_etat(lambda e: e["icone"] == "reclame_laisse", 1)


# REQ-012 : Status NeedsAttention pendant toute la réclamation en laisse, relances comprises
def test_req012_status_en_attention_pendant_les_relances(monde):
    m = monde(pc=True)
    item = m.item()
    for pose, icone in [("laisse", "osampo-reclame_laisse"), ("aboie", "osampo-aboie"),
                        ("laisse", "osampo-reclame_laisse")]:
        m.board.etat(etat="reclame", raison="rappel", rappel="sortie,1230", pose=pose)
        attendre(lambda: str(item.get("IconName")) == icone, 5, msg=icone)
        assert str(item.get("Status")) == "NeedsAttention"


# REQ-012 : la réclamation finit pendant la relance : l'icône revient à « assis »
def test_req012_fin_de_la_reclamation_pendant_la_relance(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="reclame", raison="promenade", pose="aboie")
    attendre(lambda: str(item.get("IconName")) == "osampo-aboie", 5)
    m.board.etat(etat="repos")
    attendre(lambda: str(item.get("IconName")) == "osampo-assis", 5)
    m.client.attendre_etat(lambda e: e["icone"] == "assis", 1)


# REQ-012 : board pas reflashée (ligne sans `pose`) : les relances ne sont pas signalées, la pose
# recalculée reste la laisse
@pytest.mark.parametrize("raison,rappel", EN_LAISSE)
def test_req012_sans_pose_la_laisse_est_recalculee(monde, raison, rappel):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="reclame", raison=raison, rappel=rappel)
    attendre(lambda: str(item.get("IconName")) == "osampo-reclame_laisse", 5)
    m.client.attendre_etat(lambda e: e["icone"] == "reclame_laisse", 1)


# REQ-012 : lien coupé pendant une relance avec un rappel du daemon en cours : « aboie »
def test_req012_lien_coupe_pendant_un_rappel_icone_aboie(monde):
    cfg, rid = _rappel_du_jour()
    m = monde(pc=True, config=cfg)
    item = m.item()
    m.board.etat(etat="reclame", raison="rappel", rappel=rid, pose="laisse")
    attendre(lambda: str(item.get("IconName")) == "osampo-reclame_laisse", 5)
    m.couper_board()
    attendre(lambda: str(item.get("IconName")) == "osampo-aboie", 6)
