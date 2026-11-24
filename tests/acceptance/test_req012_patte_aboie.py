"""REQ-012 : la patte, puis l'aboiement si on l'ignore (SPEC-012, ADR-006).

La logique de la board (planche patte puis aboie, délai de `patte_avant_aboie_min`) tourne dans
le firmware : elle n'est pas pilotable ici. Les tests couvrent ce que le PC voit et ce que le
daemon, la fenêtre de réglages et le modèle de configuration doivent tenir :
- le délai réglable (clé, bornes, aide, modèle commenté, validation du daemon) ;
- le délai envoyé à la board dans la ligne `cfg` ;
- le PC (icône, chien posé) qui suit la pose publiée par la board, de la patte à l'aboiement
  (les poses elles-mêmes sont aussi couvertes dans test_req001_icone.py et
  test_req011_chien_barre.py).
"""
import shutil
import time
import tomllib

import pytest

from outils import RACINE, attendre, config_toml, reglages

MODELE = RACINE / "widgets/osampo/config.toml"
CLE = "chiot.patte_avant_aboie_min"
LIBELLE = "Patte levée avant d'aboyer"
AIDE = ("Quand Claude attend une réponse, le chiot lève d'abord la patte, puis aboie s'il est "
        "ignoré pendant ce nombre de minutes ; plus haut, il patiente plus longtemps en silence, "
        "plus bas, il aboie presque aussitôt.")


def _cfg(minutes):
    return config_toml(extra=f"[chiot]\npatte_avant_aboie_min = {minutes}\n")


@pytest.fixture
def conf(tmp_path):
    p = tmp_path / "osampo.toml"
    shutil.copy(MODELE, p)
    return p


def _lignes(bins, conf, *args):
    r = reglages(bins, conf, "--lister", *args)
    assert r.returncode == 0, r.stderr
    return {l.split("\t")[1]: l.split("\t") for l in r.stdout.splitlines() if l.strip()}


def _refus(m):
    return [l for l in m.stderr_daemon().splitlines() if l.startswith("config:") and "refus" in l]


def _cfgs(m):
    return [o for o in m.board.recus() if "cfg" in o]


# ============================================ réglage : clé, bornes, aide, modèle, validation
# REQ-012 : le modèle commenté a `patte_avant_aboie_min = 2` dans [chiot], avec un commentaire
def test_req012_modele_commente(conf):
    texte = MODELE.read_text()
    assert tomllib.loads(texte)["chiot"]["patte_avant_aboie_min"] == 2
    lignes = texte.splitlines()
    i = next(i for i, l in enumerate(lignes) if l.strip().startswith("patte_avant_aboie_min"))
    assert lignes[i - 1].lstrip().startswith("#"), "un commentaire doit preceder la cle"


# REQ-012 : bornes et défaut affichés dans le premier onglet (temps et inactivité)
def test_req012_reglages_lister(bins, conf):
    assert _lignes(bins, conf)[CLE] == ["temps", CLE, "2", "1 à 60 min"]


# REQ-012 : clé absente du fichier : le défaut 2 est affiché
def test_req012_reglages_defaut_si_cle_absente(bins, tmp_path):
    nu = tmp_path / "nu.toml"
    nu.write_text("[rappels]\nreport_min = 10\n")
    assert _lignes(bins, nu)[CLE][:3] == ["temps", CLE, "2"]


# REQ-012 : libellé et aide exacts, même sans commentaires dans le fichier
def test_req012_reglages_libelle_et_aide(bins, conf, tmp_path):
    l = _lignes(bins, conf, "--aide")[CLE]
    assert (l[4], l[5]) == (LIBELLE, AIDE)
    nu = tmp_path / "nu.toml"
    nu.write_text("[chiot]\npatte_avant_aboie_min = 9\n")
    n = _lignes(bins, nu, "--aide")[CLE]
    assert n[2] == "9" and (n[4], n[5]) == (LIBELLE, AIDE)


# REQ-012 : hors bornes non enregistrable (message du daemon), fichier inchangé
@pytest.mark.parametrize("v", [0, 61, 600])
def test_req012_reglages_hors_bornes_refuse(bins, conf, v):
    avant = conf.read_bytes()
    r = reglages(bins, conf, "--set", f"{CLE}={v}")
    assert r.returncode == 2, r.stderr
    assert f"{CLE}: {v} hors de 1..60" in r.stderr
    assert conf.read_bytes() == avant


# REQ-012 : valeurs limites et défaut enregistrées
@pytest.mark.parametrize("v", [1, 2, 30, 60])
def test_req012_reglages_valeur_enregistree(bins, conf, v):
    r = reglages(bins, conf, "--set", f"{CLE}={v}")
    assert r.returncode == 0, r.stderr
    assert tomllib.loads(conf.read_text())["chiot"]["patte_avant_aboie_min"] == v


# REQ-012 : le daemon refuse le fichier en entier hors bornes, avec le message exact
@pytest.mark.parametrize("v", [0, 61])
def test_req012_le_daemon_refuse_hors_bornes(monde, v):
    m = monde(config=_cfg(5))
    time.sleep(3)
    assert not _refus(m), m.stderr_daemon()
    m.config(_cfg(v))
    attendre(lambda: _refus(m), 6, msg="refus du fichier hors bornes")
    assert f"chiot.patte_avant_aboie_min: {v} hors de 1..60" in m.stderr_daemon()


# REQ-012 : le daemon accepte les bornes 1 et 60
@pytest.mark.parametrize("v", [1, 60])
def test_req012_le_daemon_accepte_les_bornes(monde, v):
    m = monde(config=_cfg(v))
    time.sleep(3)
    assert not _refus(m), m.stderr_daemon()


# ============================================================ délai envoyé à la board (ligne cfg)
# REQ-012 : le délai du fichier part à la board, juste après calme_validation_min
def test_req012_cfg_porte_le_delai_du_fichier(monde):
    m = monde(config=_cfg(7))
    attendre(lambda: _cfgs(m), 10, msg="ligne cfg envoyee a la board")
    c = _cfgs(m)[-1]
    assert c["patte_avant_aboie_min"] == 7
    cles = list(c)
    assert cles.index("patte_avant_aboie_min") == cles.index("calme_validation_min") + 1


# REQ-012 : clé absente : défaut de 2 minutes
def test_req012_cfg_defaut_deux_minutes(monde):
    m = monde()
    attendre(lambda: _cfgs(m), 10, msg="ligne cfg envoyee a la board")
    assert _cfgs(m)[-1]["patte_avant_aboie_min"] == 2


# REQ-012 : un nouveau délai valide est renvoyé à la board sans relancer le daemon
def test_req012_cfg_nouveau_delai_transmis(monde):
    m = monde(config=_cfg(3))
    attendre(lambda: _cfgs(m) and _cfgs(m)[-1]["patte_avant_aboie_min"] == 3, 10,
             msg="delai 3 envoye")
    m.config(_cfg(12))
    attendre(lambda: _cfgs(m)[-1]["patte_avant_aboie_min"] == 12, 10,
             msg=f"delai 12 envoye, vu {[c.get('patte_avant_aboie_min') for c in _cfgs(m)]}")


# REQ-012 : un fichier refusé (délai hors bornes) ne change pas le délai de la board
def test_req012_cfg_fichier_refuse_garde_le_delai(monde):
    m = monde(config=_cfg(4))
    attendre(lambda: _cfgs(m), 10, msg="ligne cfg envoyee a la board")
    m.config(_cfg(0))
    attendre(lambda: _refus(m), 6)
    time.sleep(3)
    assert {c["patte_avant_aboie_min"] for c in _cfgs(m)} == {4}


# ============================================================ le PC suit la patte puis l'aboiement
# REQ-012 : la board passe de la patte à l'aboiement, l'icône du PC suit en moins de 5 s ;
# traitée avant le délai, la réclamation ne fait jamais aboyer (jamais d'icône « aboie »)
def test_req012_icone_patte_puis_aboie(monde):
    m = monde(pc=True)
    item = m.item()
    m.board.etat(etat="reclame", raison="claude", pose="patte")
    attendre(lambda: str(item.get("IconName")) == "osampo-patte", 5)
    t0 = time.monotonic()
    m.board.etat(etat="reclame", raison="claude", pose="aboie")
    attendre(lambda: str(item.get("IconName")) == "osampo-aboie", 5)
    assert time.monotonic() - t0 < 5
    m.client.attendre_etat(lambda e: e["icone"] == "aboie", 1)


def test_req012_reclamation_traitee_avant_le_delai_n_aboie_jamais(monde):
    m = monde(pc=True)
    item = m.item()
    vues = []
    m.board.etat(etat="reclame", raison="claude", pose="patte")
    attendre(lambda: str(item.get("IconName")) == "osampo-patte", 5)
    m.board.etat(etat="repos")   # traitée : tape, laisse ou fin de l'attente de Claude
    t = time.monotonic()
    while time.monotonic() - t < 2:
        vues.append(str(item.get("IconName")))
        time.sleep(0.1)
    assert "osampo-aboie" not in vues, vues
    assert vues[-1] == "osampo-assis", vues
