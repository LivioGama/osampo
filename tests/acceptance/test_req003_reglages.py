"""REQ-003 : fenetre de reglages, edition de osampo.toml (SPEC-007) via le mode ligne de commande."""
import os
import shutil
import stat
import time
import tomllib

import pytest

from outils import RACINE, attendre, reglages

MODELE = RACINE / "widgets/osampo/config.toml"

# (section.cle, onglet, valeur valide TOML, valeur invalide TOML)
CLES = [
    ("rappels.report_min", "temps", "15", "200"),
    ("rappels.rattrapage_min", "temps", "30", "721"),
    ("rappels.relances_s", "temps", "[30, 90]", "[90, 30]"),
    ("chiot.calme_apres_validation_min", "temps", "15", "121"),
    ("heures.calme_debut", "temps", '"23:00"', '"25:00"'),
    ("heures.calme_fin", "temps", '"06:00"', '"7h"'),
    ("heures.nuit_debut", "temps", '"21:00"', '"24:00"'),
    ("heures.nuit_fin", "temps", '"06:00"', '"06:60"'),
    ("promenade.vide_apres_min", "temps", "60", "601"),
    ("promenade.courte_min", "temps", "6", "121"),
    ("promenade.longue_min", "temps", "40", "241"),
    ("promenade.seuil_reclame", "temps", "40", "101"),
    ("promenade.retour_min", "temps", "3", "61"),
    ("ecran.lum_jour", "avance", "90", "4"),
    ("ecran.lum_nuit", "avance", "30", "101"),
    ("ecran.batterie_faible_mv", "avance", "3500", "2999"),
    ("besoins.baisse_par_jour", "avance", "20", "101"),
    ("besoins.gain", "avance", "25", "101"),
    ("gestes.lancer_g", "avance", "3.0", "0.4"),
    ("gestes.secousse_g", "avance", "3.0", "16.5"),
    ("gestes.lancer_dps", "avance", "400", "49"),
    ("gestes.lancer_ms", "avance", "30", "1001"),
    ("gestes.secousse_coups", "avance", "5", "21"),
    ("gestes.secousse_fenetre_ms", "avance", "1500", "199"),
    ("gestes.retourne_ms", "avance", "2000", "10001"),
    ("gestes.pause_gestes_ms", "avance", "2000", "10001"),
    ("gestes.journal", "avance", "1", "3"),
    ("clavier.lancer", "avance", '"x"', '"xy"'),
    ("clavier.secousse", "avance", '"z"', '" "'),
    ("clavier.retourner", "avance", '"g"', '"t"'),   # "t" est deja `lancer` : trois touches distinctes
]


@pytest.fixture
def conf(tmp_path):
    p = tmp_path / "osampo.toml"
    shutil.copy(MODELE, p)
    return p


def _lister(bins, conf):
    r = reglages(bins, conf, "--lister")
    assert r.returncode == 0, r.stderr
    lignes = [l.split("\t") for l in r.stdout.splitlines() if l.strip()]
    assert all(len(l) == 4 for l in lignes), f"4 colonnes attendues : {lignes[:3]}"
    return lignes


# REQ-003 : chaque cle est modifiable, avec ses bornes affichees, sur le bon onglet
def test_req003_lister_toutes_les_cles_avec_onglet_et_bornes(bins, conf):
    lignes = _lister(bins, conf)
    par_cle = {l[1]: l for l in lignes}
    for cle, onglet, _ok, _ko in CLES:
        assert cle in par_cle, f"{cle} absente de --lister"
        assert par_cle[cle][0] == onglet, f"{cle} : onglet {par_cle[cle][0]!r}, attendu {onglet!r}"
        assert par_cle[cle][3].strip(), f"{cle} : bornes absentes"
    assert par_cle["rappels.report_min"][2] == "10"
    assert "120" in par_cle["rappels.report_min"][3]
    assert "[[rappel]]" not in par_cle


# REQ-003 : la liste des rappels est editable : numerotee des 1, trois champs chacun
def test_req003_lister_les_rappels(bins, conf):
    lignes = _lister(bins, conf)
    rappels = [l for l in lignes if l[1].startswith("rappel.")]
    assert all(l[0] == "temps" for l in rappels)
    cles = {l[1] for l in rappels}
    for n in range(1, 6):   # le modele a cinq rappels
        for champ in ("categorie", "heure", "jours"):
            assert f"rappel.{n}.{champ}" in cles, f"rappel.{n}.{champ} absent"
    par = {l[1]: l[2] for l in rappels}
    assert "repas" in par["rappel.1.categorie"] and "12:00" in par["rappel.1.heure"]


# REQ-003 : une valeur hors bornes n'est pas enregistrable (code 2, fichier inchange)
@pytest.mark.parametrize("cle,onglet,ok,ko", CLES)
def test_req003_valeur_hors_bornes_refusee(bins, conf, cle, onglet, ok, ko):
    avant = conf.read_bytes()
    r = reglages(bins, conf, "--set", f"{cle}={ko}")
    assert r.returncode == 2, f"code {r.returncode}, stderr : {r.stderr}"
    assert r.stderr.strip(), "le message du daemon doit etre sur stderr"
    assert conf.read_bytes() == avant


# REQ-003 : la valeur valide est enregistree et le fichier reste du TOML lisible
@pytest.mark.parametrize("cle,onglet,ok,ko", CLES)
def test_req003_valeur_valide_enregistree(bins, conf, cle, onglet, ok, ko):
    r = reglages(bins, conf, "--set", f"{cle}={ok}")
    assert r.returncode == 0, r.stderr
    section, nom = cle.split(".")
    voulu = tomllib.loads(f"{nom} = {ok}")[nom]
    assert tomllib.loads(conf.read_text())[section][nom] == voulu


# REQ-003 : message exact du daemon pour une cle hors bornes (SPEC-007, Validation)
def test_req003_message_du_daemon(bins, conf):
    r = reglages(bins, conf, "--set", "rappels.report_min=200")
    assert r.returncode == 2
    assert "rappels.report_min: 200 hors de 1..120" in r.stderr


# REQ-003 : cle inconnue
def test_req003_cle_inconnue(bins, conf):
    avant = conf.read_bytes()
    r = reglages(bins, conf, "--set", "rappels.nimporte=3")
    assert r.returncode == 2
    assert "cle inconnue: rappels.nimporte" in r.stderr
    assert conf.read_bytes() == avant


# REQ-003 : regles croisees (longue > courte, trois touches distinctes, relances croissantes)
def test_req003_regles_croisees(bins, conf):
    avant = conf.read_bytes()
    for arg in ("promenade.longue_min=4", "clavier.secousse=\"t\"", "rappels.relances_s=[10, 10]",
                "rappels.relances_s=[10, 20, 30, 40]", "ecran.batterie_faible_mv=0",):
        r = reglages(bins, conf, "--set", arg)
        if arg == "ecran.batterie_faible_mv=0":
            assert r.returncode == 0, "0 (jamais) est une valeur permise"
        else:
            assert r.returncode == 2, f"{arg} aurait du etre refuse : {r.stderr}"
    # seul le dernier set (valide) a pu changer le fichier
    assert tomllib.loads(conf.read_text())["promenade"]["longue_min"] == 30
    assert avant


# REQ-003 : enregistrer produit un fichier que le daemon accepte et relit
def test_req003_le_daemon_accepte_le_fichier_enregistre(bins, monde):
    m = monde()
    r = reglages(bins, m.cfg, "--set", "rappels.report_min=15", "--set",
                 'heures.calme_debut="23:00"', "--set", "rappels.relances_s=[30, 90]",
                 "--set", "gestes.journal=1")
    assert r.returncode == 0, r.stderr
    # le daemon relit toutes les 2 s ; on attend la relecture du fichier enregistre
    attendre(lambda: any(l.startswith("config:") and "relue" in l
                         for l in m.stderr_daemon().splitlines()), 6,
             msg="relecture `config:` du fichier enregistre")
    time.sleep(1)
    assert not _refus_config(m.stderr_daemon()), m.stderr_daemon()


def _refus_config(journal):
    """Lignes `config:` qui signalent un refus (les lignes `(lue)`, `relue` sont normales)."""
    return [l for l in journal.splitlines() if l.startswith("config:") and "refus" in l]


# REQ-003 (garde-fou du test precedent) : le daemon journalise bien `config:` pour un fichier refuse
def test_req003_le_daemon_refuse_un_fichier_hors_bornes(monde):
    m = monde()
    m.config(m.cfg.read_text().replace("report_min = 10", "report_min = 200"))
    attendre(lambda: bool(_refus_config(m.stderr_daemon())), 6,
             msg="refus `config:` dans le journal du daemon")


# REQ-003 : les commentaires, l'ordre et les cles non modifiees sont conserves a l'octet pres
def test_req003_commentaires_et_ordre_conserves(bins, conf):
    avant = conf.read_text().splitlines(keepends=True)
    r = reglages(bins, conf, "--set", "rappels.report_min=15")
    assert r.returncode == 0, r.stderr
    apres = conf.read_text().splitlines(keepends=True)
    assert len(apres) == len(avant)
    diff = [(a, b) for a, b in zip(avant, apres) if a != b]
    assert len(diff) == 1, f"une seule ligne doit changer : {diff}"
    assert diff[0][0] == "report_min = 10\n" and diff[0][1].split("#")[0].replace(
        " ", "").strip() == "report_min=15"


# REQ-003 : un fichier a la mise en forme libre est conserve, sauf la valeur modifiee
def test_req003_mise_en_forme_libre_conservee(bins, tmp_path):
    p = tmp_path / "t.toml"
    texte = ("# mon fichier\n\n[rappels]   # section\nreport_min   =   12   # douze\n\n"
             "[heures]\ncalme_debut = \"22:00\"\n\n# fin\n")
    p.write_text(texte)
    r = reglages(bins, p, "--set", "heures.calme_debut=\"23:00\"")
    assert r.returncode == 0, r.stderr
    assert p.read_text() == texte.replace('calme_debut = "22:00"', 'calme_debut = "23:00"')


# REQ-003 : une cle modifiee absente est ajoutee dans sa section ; une cle non modifiee absente reste absente
def test_req003_cle_absente_ajoutee_section_creee(bins, tmp_path):
    p = tmp_path / "t.toml"
    p.write_text("[rappels]\nreport_min = 10\n")
    r = reglages(bins, p, "--set", 'heures.calme_debut="23:00"', "--set", "rappels.rattrapage_min=30")
    assert r.returncode == 0, r.stderr
    d = tomllib.loads(p.read_text())
    assert d["heures"]["calme_debut"] == "23:00"
    assert d["rappels"] == {"report_min": 10, "rattrapage_min": 30}
    assert "calme_fin" not in d["heures"] and "chiot" not in d
    assert p.read_text().startswith("[rappels]\nreport_min = 10\n")


# REQ-003 : fichier absent -> le modele commente est ecrit a l'enregistrement
def test_req003_fichier_absent_part_du_modele(bins, tmp_path):
    p = tmp_path / "absent.toml"
    r = reglages(bins, p, "--lister")
    assert r.returncode == 0 and "rappels.report_min" in r.stdout
    assert not p.exists(), "--lister n'ecrit rien"
    r = reglages(bins, p, "--set", "rappels.report_min=15")
    assert r.returncode == 0, r.stderr
    t = p.read_text()
    assert "# Reglages du chiot Osampo." in t
    assert tomllib.loads(t)["rappels"]["report_min"] == 15
    assert tomllib.loads(t)["promenade"]["vide_apres_min"] == 90


# REQ-003 : une cle absente s'affiche a sa valeur par defaut
def test_req003_cle_absente_affichee_par_defaut(bins, tmp_path):
    p = tmp_path / "t.toml"
    p.write_text("[rappels]\nreport_min = 12\n")
    par = {l[1]: l for l in _lister(bins, p)}
    assert par["rappels.report_min"][2] == "12"
    assert par["rappels.rattrapage_min"][2] == "60"
    assert par["gestes.secousse_coups"][2] == "4"


# REQ-003 : fichier illisible -> erreur, rien n'est ecrit
def test_req003_fichier_non_toml_rien_n_est_ecrit(bins, tmp_path):
    p = tmp_path / "t.toml"
    p.write_text("ceci n'est [pas du toml\n")
    r = reglages(bins, p, "--set", "rappels.report_min=15")
    assert r.returncode != 0
    assert r.stderr.strip()
    assert p.read_text() == "ceci n'est [pas du toml\n"


# REQ-003 : [poste] est reecrit en [promenade], memes valeurs, commentaires conserves
def test_req003_poste_devient_promenade(bins, tmp_path):
    p = tmp_path / "t.toml"
    p.write_text("[rappels]\nreport_min = 10\n\n[poste]\n# duree avant la laisse\n"
                 "laisse_apres_min = 80\npause_min = 4   # courte\n")
    r = reglages(bins, p, "--set", "rappels.report_min=11")
    assert r.returncode == 0, r.stderr
    t = p.read_text()
    d = tomllib.loads(t)
    assert "poste" not in d
    assert d["promenade"]["vide_apres_min"] == 80 and d["promenade"]["courte_min"] == 4
    assert "# duree avant la laisse" in t and "# courte" in t


# REQ-003 : [poste] et [promenade] ensemble : rien n'est enregistre
def test_req003_poste_et_promenade_ensemble_refuse(bins, tmp_path):
    p = tmp_path / "t.toml"
    texte = "[promenade]\nvide_apres_min = 90\n\n[poste]\nlaisse_apres_min = 80\n"
    p.write_text(texte)
    r = reglages(bins, p, "--set", "rappels.report_min=11")
    assert r.returncode != 0 and r.stderr.strip()
    assert p.read_text() == texte


# REQ-003 : ecriture atomique (fichier temporaire puis renommage) : pas de fichier a moitie ecrit
def test_req003_ecriture_atomique(bins, conf):
    inode = conf.stat().st_ino
    r = reglages(bins, conf, "--set", "rappels.report_min=15")
    assert r.returncode == 0
    assert conf.stat().st_ino != inode, "ecriture en place : l'inode n'a pas change"
    assert sorted(os.listdir(conf.parent)) == ["osampo.toml"], "fichier temporaire oublie"


# REQ-003 : un clic sur l'icone (ou le menu) ouvre le programme de reglages, une seule fois
def test_req003_clic_icone_lance_les_reglages_une_seule_instance(bins, monde):
    m = monde(pc="manuel")
    bindir = m.tmp / "bin"
    bindir.mkdir()
    pc = bindir / "osampo-pc"
    shutil.copy(bins["osampo-pc"], pc)
    journal = m.tmp / "lancements"
    faux = bindir / "osampo-reglages"
    faux.write_text(f"#!/bin/sh\necho \"$@\" >> {journal}\nexec sleep 60\n")
    faux.chmod(faux.stat().st_mode | stat.S_IXUSR)
    m.demarrer_pc(binaire=str(pc), env_extra={"PATH": f"{bindir}:{os.environ['PATH']}"})
    item = m.item()
    item.activer()
    attendre(lambda: journal.exists() and journal.read_text().count("\n") == 1, 5,
             msg="le programme de reglages doit etre lance par le clic")
    item.activer()
    item.cliquer("Réglages…")
    time.sleep(2)
    assert journal.read_text().count("\n") == 1, "un second programme de reglages a ete lance"
