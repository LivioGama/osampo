"""REQ-010 : aide des reglages, libelle et phrase d'aide de chaque cle (SPEC-010, ADR-002).

La fenetre Slint n'est pas pilotable ici : les textes sont verifies par `--lister --aide`,
qui sort du meme code et de la meme table (SPEC-010, « Mode ligne de commande »).
"""
import re
import shutil

import pytest

from outils import RACINE, reglages

MODELE = RACINE / "widgets/osampo/config.toml"

# Libelles et aides exacts de SPEC-010, « Textes » : cle -> (libelle, aide)
TEXTES = {
    'rappels.report_min': (
        "Durée d'un report",
        'Un rappel reporté revient au bout de ce nombre de minutes ; plus haut, il revient plus tard, plus bas, il revient plus vite.'),
    'rappels.rattrapage_min': (
        'Délai de rattrapage',
        "Un rappel qui n'a pas pu sonner à son heure sonne encore pendant ce nombre de minutes ; plus haut, il est rattrapé plus longtemps, plus bas, il est vite abandonné."),
    'rappels.relances_s': (
        "Relances d'un rappel",
        "Tant qu'un rappel n'est pas pris en compte, le chiot insiste après chacun de ces délais, en secondes ; des valeurs hautes espacent les relances, des valeurs basses le rendent plus pressant."),
    'chiot.calme_apres_validation_min': (
        'Temps de calme après une validation',
        'Après un rappel validé, le chiot attend ce nombre de minutes avant de pouvoir réclamer la promenade ; plus haut, il reste tranquille plus longtemps, plus bas, il peut réclamer presque aussitôt.'),
    'chiot.patte_avant_aboie_min': (
        "Patte levée avant d'aboyer",
        "Quand Claude attend une réponse, le chiot lève d'abord la patte, puis aboie s'il est ignoré pendant ce nombre de minutes ; plus haut, il patiente plus longtemps en silence, plus bas, il aboie presque aussitôt."),
    'heures.calme_debut': (
        'Début des heures calmes',
        "Heure (HH:MM) à partir de laquelle le chiot et le PC se taisent et aucun rappel ne sonne ; plus tôt, le calme commence plus tôt ; égale à la fin, il n'y a pas d'heures calmes."),
    'heures.calme_fin': (
        'Fin des heures calmes',
        "Heure (HH:MM) à laquelle les sons et les rappels reprennent ; plus tard, le calme dure plus longtemps ; égale au début, il n'y a pas d'heures calmes."),
    'heures.nuit_debut': (
        'Début de la nuit',
        "Heure (HH:MM) à partir de laquelle l'écran de la board passe à la luminosité de nuit ; plus tôt, la nuit commence plus tôt ; égale à la fin, il ne fait jamais nuit."),
    'heures.nuit_fin': (
        'Fin de la nuit',
        "Heure (HH:MM) à laquelle l'écran repasse à la luminosité de jour ; plus tard, la nuit dure plus longtemps ; égale au début, il ne fait jamais nuit."),
    'promenade.vide_apres_min': (
        'Temps pour vider la jauge',
        'Minutes passées au clavier et à la souris qui vident la jauge sage, pleine au départ ; vide, le chiot réclame la promenade ; plus haut, la jauge baisse plus lentement, plus bas, il réclame plus souvent.'),
    'promenade.courte_min': (
        'Remplissage de la jauge : promenade courte',
        'Absence du PC, en minutes, à partir de laquelle la jauge commence à se recharger ; plus haut, une courte pause ne compte plus comme promenade, plus bas, la moindre pause recharge un peu.'),
    'promenade.longue_min': (
        'Remplissage de la jauge : promenade longue',
        'Absence du PC, en minutes, qui recharge la jauge entièrement ; plus haut, il faut une plus longue absence pour la remplir, plus bas, elle se remplit vite.'),
    'promenade.seuil_reclame': (
        'Redemande si la jauge reste sous',
        "Niveau de la jauge, de 0 à 100, sous lequel le chiot redemande la promenade 10 minutes après la fin d'une promenade ; plus haut, il redemande plus souvent, plus bas, une petite promenade lui suffit."),
    'promenade.retour_min': (
        "Fin d'une promenade oubliée",
        "Minutes d'activité au PC qui terminent une promenade oubliée sur la board ; plus haut, la promenade reste ouverte plus longtemps, plus bas, elle se termine dès le retour au PC."),
    'ecran.lum_jour': (
        'Luminosité de jour',
        "Luminosité de l'écran de la board le jour, en pourcent ; plus haut, l'écran est plus lumineux, plus bas, il est plus sombre et use moins la batterie."),
    'ecran.lum_nuit': (
        'Luminosité de nuit',
        "Luminosité de l'écran de la board la nuit, en pourcent ; plus haut, l'écran éclaire davantage la pièce, plus bas, il gêne moins dans le noir."),
    'ecran.batterie_faible_mv': (
        'Seuil de batterie faible',
        "Tension de la batterie, en millivolts, sous laquelle l'icône de batterie faible s'affiche (0 : jamais) ; plus haut, l'alerte vient plus tôt, plus bas, elle vient plus près de la panne."),
    'besoins.baisse_par_jour': (
        'Baisse des besoins',
        "Points perdus chaque jour par les besoins du chiot (bouger, jouer, dormir), de 0 à 100 ; plus haut, il faut s'en occuper plus souvent, plus bas, ses besoins restent longtemps remplis."),
    'besoins.gain': (
        "Gain d'un soin",
        'Points rendus à un besoin par chaque soin donné, de 0 à 100 ; plus haut, quelques soins suffisent, plus bas, il en faut beaucoup.'),
    'gestes.lancer_g': (
        "Force d'un lancer",
        'Accélération, en g, qui déclenche un lancer de la board ; plus haut, il faut lancer plus fort, plus bas, un petit mouvement suffit et les fausses détections augmentent.'),
    'gestes.lancer_dps': (
        "Rotation d'un lancer",
        'Vitesse de rotation, en degrés par seconde, qui déclenche un lancer ; plus haut, il faut faire tourner la board plus vite, plus bas, un lancer est détecté plus facilement.'),
    'gestes.lancer_ms': (
        "Durée d'un lancer",
        'Durée minimale du mouvement, en millisecondes, pour compter un lancer ; plus haut, les chocs brefs sont ignorés, plus bas, un lancer très court est compté.'),
    'gestes.secousse_g': (
        "Force d'une secousse",
        'Accélération, en g, qui compte comme un coup de secousse ; plus haut, il faut secouer plus fort, plus bas, une légère secousse suffit.'),
    'gestes.secousse_coups': (
        "Coups d'une secousse",
        'Nombre de coups à réunir pour une secousse ; plus haut, il faut secouer plus longtemps, plus bas, deux ou trois coups suffisent.'),
    'gestes.secousse_fenetre_ms': (
        "Fenêtre d'une secousse",
        'Temps, en millisecondes, pour réunir ces coups ; plus haut, on peut secouer plus lentement, plus bas, il faut secouer vite.'),
    'gestes.retourne_ms': (
        "Durée d'un retournement",
        'Temps, en millisecondes, pendant lequel la board doit rester retournée pour compter le geste ; plus haut, il faut la laisser plus longtemps, plus bas, un retournement bref suffit.'),
    'gestes.pause_gestes_ms': (
        'Pause entre deux gestes',
        "Temps, en millisecondes, pendant lequel un nouveau geste est ignoré après le précédent ; plus haut, les gestes s'enchaînent moins, plus bas, deux gestes rapprochés comptent tous les deux."),
    'gestes.journal': (
        'Journal des gestes',
        'Détail du journal des gestes de la board : 0 muet, 1 une ligne par salve, 2 salve et échantillons bruts ; plus haut, le journal est plus bavard.'),
    'clavier.lancer': (
        'Touche du lancer',
        'Touche, un caractère, qui simule un lancer dans le chiot du desktop ; elle doit différer des deux autres touches.'),
    'clavier.secousse': (
        'Touche de la secousse',
        'Touche, un caractère, qui simule une secousse dans le chiot du desktop ; elle doit différer des deux autres touches.'),
    'clavier.retourner': (
        'Touche du retournement',
        'Touche, un caractère, qui simule un retournement dans le chiot du desktop ; elle doit différer des deux autres touches.'),
    'pc.chien_px': (
        'Taille du chien sur le bureau',
        'Hauteur et largeur, en pixels, du chien posé au-dessus de la barre des tâches du PC ; plus haut, il est plus grand et cache plus de bureau, plus bas, il est plus discret.'),
    'rappel.categorie': (
        'Sorte de rappel',
        "Ce que le chiot réclame à l'heure dite : repas se valide d'un toucher, sortie, sport et dodo réclament la promenade (dodo : celle du soir, puis il dort)."),
    'rappel.heure': (
        'Heure du rappel',
        'Heure (HH:MM) à laquelle le rappel sonne ; plus tôt ou plus tard, il sonne plus tôt ou plus tard dans la journée.'),
    'rappel.jours': (
        'Jours du rappel',
        'Jours de la semaine où le rappel sonne, au moins un parmi lun..dim ; moins de jours, il sonne moins souvent.'),
}


# SPEC-007, onglet Avance (pc.chien_px : SPEC-011)
ONGLET_AVANCE = ("ecran.", "besoins.", "gestes.", "clavier.", "pc.")


@pytest.fixture
def conf(tmp_path):
    p = tmp_path / "osampo.toml"
    shutil.copy(MODELE, p)
    return p


def _aide(bins, conf):
    r = reglages(bins, conf, "--lister", "--aide")
    assert r.returncode == 0, r.stderr
    lignes = [l.split("\t") for l in r.stdout.splitlines() if l.strip()]
    assert lignes, "aucune ligne"
    assert all(len(l) == 6 for l in lignes), \
        f"6 colonnes attendues (onglet, cle, valeur, bornes, libelle, aide) : {lignes[:2]}"
    return {l[1]: l for l in lignes}


def _textes_de(cle):
    """Texte attendu pour une cle de --lister : rappel.N.champ prend ceux de rappel.champ."""
    m = re.fullmatch(r"rappel\.\d+\.(categorie|heure|jours)", cle)
    return TEXTES["rappel." + m[1]] if m else TEXTES[cle]


def _textes(par):
    return {c: (l[4], l[5]) for c, l in par.items()}


# REQ-010 : chaque cle du fichier a son libelle et sa phrase d'aide, exactement ceux de la spec
@pytest.mark.parametrize("cle", [c for c in TEXTES if not c.startswith("rappel.")])
def test_req010_libelle_et_aide_de_chaque_cle(bins, conf, cle):
    par = _aide(bins, conf)
    assert cle in par, f"{cle} absente de --lister --aide"
    libelle, aide = TEXTES[cle]
    assert par[cle][4] == libelle
    assert par[cle][5] == aide


# REQ-010 : les trois champs de chaque rappel de la liste portent les textes de rappel.*
def test_req010_aide_des_champs_de_rappel(bins, conf):
    par = _aide(bins, conf)
    rappels = {c: l for c, l in par.items() if c.startswith("rappel.")}
    assert len(rappels) >= 15, "le modele a cinq rappels, trois champs chacun"
    for cle, l in rappels.items():
        assert (l[4], l[5]) == _textes_de(cle), cle


# REQ-010 : un fichier a deux rappels : libelle et aide pour chaque champ de chaque rappel
def test_req010_aide_de_rappels_d_un_fichier_court(bins, tmp_path):
    p = tmp_path / "t.toml"
    p.write_text('[[rappel]]\ncategorie = "sport"\nheure = "18:00"\n\n'
                 '[[rappel]]\ncategorie = "dodo"\nheure = "22:30"\njours = ["lun"]\n')
    par = _aide(bins, p)
    for n in (1, 2):
        for champ in ("categorie", "heure", "jours"):
            l = par[f"rappel.{n}.{champ}"]
            assert (l[4], l[5]) == TEXTES[f"rappel.{champ}"]


# REQ-010 : les quatre premieres colonnes sont celles de --lister (format de SPEC-007 inchange)
def test_req010_aide_ajoute_deux_colonnes_sans_toucher_les_autres(bins, conf):
    sans = reglages(bins, conf, "--lister")
    avec = reglages(bins, conf, "--lister", "--aide")
    assert sans.returncode == 0 and avec.returncode == 0
    ls = [l.split("\t") for l in sans.stdout.splitlines() if l.strip()]
    la = [l.split("\t") for l in avec.stdout.splitlines() if l.strip()]
    assert all(len(l) == 4 for l in ls)
    assert [l[:4] for l in la] == ls


# REQ-010 : les onglets restent ceux de SPEC-007 avec l'aide
def test_req010_onglets_inchanges(bins, conf):
    for cle, l in _aide(bins, conf).items():
        attendu = "avance" if cle.startswith(ONGLET_AVANCE) else "temps"
        assert l[0] == attendu, cle


# REQ-010 : chaque cle affichee a un libelle et une aide non vides (la table n'a pas de trou)
def test_req010_aucune_cle_sans_texte(bins, conf):
    par = _aide(bins, conf)
    for cle, l in par.items():
        assert l[4].strip() and l[5].strip(), f"{cle} sans libelle ou sans aide"
    assert set(TEXTES) - {"rappel.categorie", "rappel.heure", "rappel.jours"} <= set(par)


# REQ-010 : l'aide s'affiche meme si les commentaires du fichier ont ete supprimes
def test_req010_fichier_sans_commentaires(bins, conf, tmp_path):
    attendu = _textes(_aide(bins, conf))
    nu = tmp_path / "nu.toml"
    lignes = []
    for l in conf.read_text().splitlines():
        if l.lstrip().startswith("#"):
            continue
        s = l.split("#")[0].rstrip()
        if s.strip():
            lignes.append(s)
    nu.write_text("\n".join(lignes) + "\n")
    assert "#" not in nu.read_text()
    assert _textes(_aide(bins, nu)) == attendu


# REQ-010 : fichier absent : memes textes (le modele sert de base)
def test_req010_fichier_absent(bins, tmp_path, conf):
    attendu = _textes(_aide(bins, conf))
    absent = tmp_path / "absent.toml"
    assert _textes(_aide(bins, absent)) == attendu
    assert not absent.exists()


# REQ-010 : des commentaires qui disent autre chose ne changent pas les textes
def test_req010_commentaires_trompeurs(bins, tmp_path, conf):
    attendu = _aide(bins, conf)
    p = tmp_path / "t.toml"
    p.write_text("# AIDE BIDON : ne pas utiliser\n[rappels]\n# report_min : nombre de bananes\n"
                 "report_min = 10   # bananes\nrattrapage_min = 60  # AIDE BIDON\n\n"
                 "[heures]\n# AIDE BIDON\ncalme_debut = \"22:00\"\n")
    par = _aide(bins, p)
    for cle in ("rappels.report_min", "rappels.rattrapage_min", "heures.calme_debut"):
        assert (par[cle][4], par[cle][5]) == (attendu[cle][4], attendu[cle][5])
        assert "BIDON" not in par[cle][4] + par[cle][5] and "banane" not in par[cle][5]


# REQ-010 : valeur et bornes viennent toujours du fichier et de la table, l'aide n'y change rien
def test_req010_valeurs_et_bornes_conservees(bins, tmp_path):
    p = tmp_path / "t.toml"
    p.write_text("[rappels]\nreport_min = 12\n")
    par = _aide(bins, p)
    assert par["rappels.report_min"][2] == "12"
    assert "120" in par["rappels.report_min"][3]
    assert par["rappels.rattrapage_min"][2] == "60"


# REQ-010 : --aide n'ecrit rien
def test_req010_aide_n_ecrit_rien(bins, conf):
    avant = conf.read_bytes()
    _aide(bins, conf)
    assert conf.read_bytes() == avant
