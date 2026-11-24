"""REQ-001 : icone du chien dans la zone de notification (SPEC-002, SPEC-004)."""
import pathlib
import time

import pytest
from PIL import Image

from outils import RACINE, attendre, config_toml, hhmm, id_rappel, maintenant

ETATS = [  # (ligne d'etat de la board, icone attendue) ; ADR-004 et ADR-006 : pose de la board copiee
    (dict(etat="repos"), "assis"),
    (dict(etat="dort"), "niche"),
    (dict(etat="reclame", raison="rappel", rappel="repas,1200", pose="aboie"), "aboie"),
    (dict(etat="reclame", raison="rappel", rappel="sortie,1230", pose="laisse"), "reclame_laisse"),
    (dict(etat="reclame", raison="rappel", rappel="sport,1230", pose="laisse"), "reclame_laisse"),
    (dict(etat="reclame", raison="rappel", rappel="dodo,1230", pose="laisse"), "reclame_laisse"),
    (dict(etat="reclame", raison="promenade", pose="laisse"), "reclame_laisse"),
    (dict(etat="reclame", raison="claude", pose="patte"), "patte"),
    (dict(etat="reclame", raison="claude", pose="aboie"), "aboie"),  # escalade, SPEC-012
    (dict(etat="promenade", promenade_debut="14:05"), "laisse"),
]
RECLAME = ("aboie", "patte", "reclame_laisse")
# Board pas encore reflashee : pas de `pose`, ou valeur inconnue ; pose recalculee (SPEC-012)
SANS_POSE = [
    (dict(etat="reclame", raison="claude"), "patte"),
    (dict(etat="reclame", raison="claude", pose="?"), "patte"),
    (dict(etat="reclame", raison="promenade", pose="n'importe quoi"), "reclame_laisse"),
    (dict(etat="reclame", raison="rappel", rappel="sortie,1230"), "reclame_laisse"),
    (dict(etat="reclame", raison="rappel", rappel="sport,1230"), "reclame_laisse"),
    (dict(etat="reclame", raison="rappel", rappel="dodo,1230", pose="-"), "reclame_laisse"),
    (dict(etat="reclame", raison="rappel", rappel="repas,1200"), "aboie"),
]


# REQ-001 : l'etat publie par le daemon donne la bonne icone, en moins de 5 s
@pytest.mark.parametrize("etat,icone", ETATS + SANS_POSE)
def test_req001_daemon_publie_l_icone_de_l_etat_board(monde, etat, icone):
    m = monde()
    m.board.etat(**etat)
    m.client.attendre_etat(lambda e: e["icone"] == icone, 5, msg=f"icone {icone}")


# REQ-001 : l'icone du PC affiche l'etat, pour chaque etat (dont la patte levee, ADR-004)
@pytest.mark.parametrize("etat,icone", ETATS + SANS_POSE)
def test_req001_icone_sni_suit_l_etat_board(monde, etat, icone):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(**etat)
    attendre(lambda: str(item.get("IconName")) == f"osampo-{icone}", 5,
             msg=f"IconName osampo-{icone}, vu {item.get('IconName')}")
    assert str(item.get("Status")) == ("NeedsAttention" if icone in RECLAME else "Active")


# REQ-001 : l'icone change en moins de 5 s apres le changement d'etat (cote board)
def test_req001_icone_change_en_moins_de_5_s(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="repos")
    attendre(lambda: str(item.get("IconName")) == "osampo-assis", 5)
    m.board.etat(etat="dort")
    attendre(lambda: str(item.get("IconName")) == "osampo-niche", 5)
    m.board.etat(etat="promenade", promenade_debut="14:05")
    attendre(lambda: str(item.get("IconName")) == "osampo-laisse", 5)


# REQ-001 : l'icone change aussi quand le changement vient du daemon (rappel de sortie dû)
def test_req001_icone_aboie_quand_un_rappel_du_daemon_est_du(monde):
    n = maintenant()
    m = monde(pc=True, config=config_toml([("sortie", hhmm(n), None)]))
    item = m.item()
    attendre(lambda: str(item.get("IconName")) == "osampo-aboie", 5,
             msg="icone aboie au rappel du daemon")
    m.client.attendre_etat(lambda e: e["icone"] == "aboie", 1)


# REQ-001 : ADR-006, board connectee et reclamant : sa pose passe avant le rappel du daemon
# (le chien du PC copie « toujours » la pose de la board) ; le rappel accepte, la pose reste
def test_req001_pose_de_la_board_prime_sur_le_rappel_du_daemon(monde):
    n = maintenant()
    h = hhmm(n)
    m = monde(pc=True, config=config_toml([("sortie", h, None)]))
    item = m.item()
    m.board.etat(etat="reclame", raison="claude", pose="patte")
    attendre(lambda: str(item.get("IconName")) == "osampo-patte", 5, msg="patte copiee")
    time.sleep(1.5)
    assert str(item.get("IconName")) == "osampo-patte", "le rappel du daemon ne prend pas le pas"
    m.client.attendre_etat(lambda e: e["icone"] == "patte", 1)
    m.client.commande("accepter", rappel=id_rappel("sortie", h))
    time.sleep(1.5)
    assert str(item.get("IconName")) == "osampo-patte"


# REQ-001 : ADR-006, la board qui tient la laisse pendant un rappel de sortie : « reclame_laisse »
# (regle 1), pas « aboie » ; la board qui ne reclame pas (encore) : « aboie » (regle 2)
def test_req001_rappel_de_sortie_laisse_dans_la_gueule(monde):
    n = maintenant()
    h = hhmm(n)
    m = monde(pc=True, config=config_toml([("sortie", h, None)]))
    item = m.item()
    m.board.etat(etat="repos")
    attendre(lambda: str(item.get("IconName")) == "osampo-aboie", 5, msg="rappel dû, board au repos")
    m.board.etat(etat="reclame", raison="rappel", rappel=id_rappel("sortie", h),
                 pose="laisse")
    attendre(lambda: str(item.get("IconName")) == "osampo-reclame_laisse", 5,
             msg="la board tient la laisse")
    m.client.attendre_etat(lambda e: e["icone"] == "reclame_laisse", 1)


# REQ-001 : la patte, la laisse et l'aboiement se succedent selon la pose de la board
def test_req001_icone_passe_de_la_patte_a_l_aboiement_et_inversement(monde):
    m = monde(pc=True)
    item = m.item()
    suite = [("claude", "-", "patte", "patte"), ("claude", "-", "aboie", "aboie"),
             ("rappel", "sortie,1230", "laisse", "reclame_laisse"),
             ("claude", "-", "patte", "patte"),
             ("rappel", "repas,1200", "aboie", "aboie"),
             ("promenade", "-", "laisse", "reclame_laisse")]
    for raison, rappel, pose, icone in suite:
        m.board.etat(etat="reclame", raison=raison, rappel=rappel, pose=pose)
        attendre(lambda: str(item.get("IconName")) == f"osampo-{icone}", 5,
                 msg=f"{raison}/{pose} -> {icone}, vu {item.get('IconName')}")
        assert str(item.get("Status")) == "NeedsAttention"


# REQ-001 / REQ-012 : l'escalade de la board (patte puis aboie) se retrouve sur l'icone en moins de 5 s
def test_req001_req012_icone_suit_l_escalade_patte_puis_aboie(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="reclame", raison="claude", pose="patte")
    attendre(lambda: str(item.get("IconName")) == "osampo-patte", 5)
    t0 = time.monotonic()
    m.board.etat(etat="reclame", raison="claude", pose="aboie")   # la board change de planche
    attendre(lambda: str(item.get("IconName")) == "osampo-aboie", 5, msg="aboie apres la patte")
    assert time.monotonic() - t0 < 5
    m.board.etat(etat="repos")   # reclamation traitee
    attendre(lambda: str(item.get("IconName")) == "osampo-assis", 5)


# REQ-001 : SPEC-002 regle 5 : un `reclame` garde apres coupure du lien -> assis,
# qu'il soit pour Claude (patte ou aboie), un rappel repas (aboie) ou la jauge (laisse) ;
# SPEC-002 regle 2 et SPEC-003 : un rappel de sortie reclame par la board est adopte par le
# daemon et survit a la coupure -> aboie, quelle que soit la derniere pose recue
@pytest.mark.parametrize("raison,rappel,pose,icone,apres", [
    ("claude", "-", "patte", "patte", "assis"), ("claude", "-", "aboie", "aboie", "assis"),
    ("rappel", "repas,1200", "aboie", "aboie", "assis"),
    ("rappel", "sortie,1230", "laisse", "reclame_laisse", "aboie"),
    ("promenade", "-", "laisse", "reclame_laisse", "assis")])
def test_req001_icone_assis_pour_reclame_garde_apres_coupure(monde, raison, rappel, pose, icone,
                                                             apres):
    m = monde()
    m.board.etat(etat="reclame", raison=raison, rappel=rappel, pose=pose)
    m.client.attendre_etat(lambda e: e["icone"] == icone, 5)
    m.couper_board()
    m.client.attendre_etat(lambda e: e["board"] == "hors_ligne" and e["icone"] == apres, 5,
                           msg=f"apres coupure : {apres}")


# REQ-001 : icone dessinee pour 22-24 px, distincte des sprites 1.83"
def test_req001_icones_22_et_24_px_existent_et_ne_sont_pas_les_sprites(monde):
    m = monde(pc=True)
    item = m.item()
    dossier = pathlib.Path(str(item.get("IconThemePath")))
    assert dossier.is_dir(), f"IconThemePath {dossier!r} n'est pas un dossier"
    sprites = {p.read_bytes() for p in (RACINE / "widgets/osampo/ui/sprites").glob("*.png")}
    noms = ("assis", "niche", "aboie", "laisse", "patte", "reclame_laisse")
    contenu = {}
    for nom in noms:
        tailles = {}
        for p in dossier.rglob(f"osampo-{nom}.png"):
            with Image.open(p) as im:
                tailles[im.size] = p
        assert (22, 22) in tailles, f"osampo-{nom} en 22 px absente de {dossier}"
        assert (24, 24) in tailles, f"osampo-{nom} en 24 px absente de {dossier}"
        assert tailles[(22, 22)].read_bytes() != tailles[(24, 24)].read_bytes()
        for t, p in tailles.items():
            assert p.read_bytes() not in sprites, f"{p} est un sprite 1.83\" copie tel quel"
            contenu[(nom, t)] = p.read_bytes()
    # SPEC-004 : les six icones sont distinctes deux a deux a une meme taille
    for t in ((22, 22), (24, 24)):
        octets = [contenu[(nom, t)] for nom in noms]
        assert len(set(octets)) == len(noms), f"icones identiques en {t}"


# REQ-001 : l'item est un StatusNotifierItem de Cinnamon (SPEC-004)
def test_req001_item_enregistre_aupres_du_watcher(monde):
    m = monde(pc=True)
    item = m.item()
    assert str(item.get("Id")) == "osampo"
    assert str(item.get("Title")) == "Osampo"
    assert str(item.get("Category")) == "ApplicationStatus"


# REQ-001 : le watcher peut manquer au lancement, le programme reessaie (toutes les 5 s)
def test_req001_reessaie_de_s_inscrire_si_le_watcher_est_absent(monde):
    m = monde(pc=True, watcher=False)
    import time
    time.sleep(2)
    import dbus_faux
    m.faux_watcher = dbus_faux.FauxWatcher(m.bus.conn)
    m.item(timeout=10)


# REQ-001 : daemon injoignable -> une seule ligne et icone assis (SPEC-004)
def test_req001_daemon_injoignable_icone_assis(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="dort")
    attendre(lambda: str(item.get("IconName")) == "osampo-niche", 5)
    m.arreter_daemon()
    attendre(lambda: str(item.get("IconName")) == "osampo-assis", 5)
    assert "Daemon : injoignable" in item.tooltip()[1]
