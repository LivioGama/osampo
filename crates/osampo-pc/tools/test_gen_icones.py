"""Verifie les 12 icones generees : taille, RGBA, fond transparent, contenu, distinctes."""
import os
import sys

from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_icones as g  # noqa: E402


def test_icones_a_jour_et_bien_formees():
    ims = g.generer()
    for nom in g.NOMS:
        for t in (22, 24):
            chemin = os.path.join(g.ICONES, f"{t}x{t}", "status", f"osampo-{nom}.png")
            im = Image.open(chemin)
            assert im.size == (t, t) and im.mode == "RGBA", chemin
            assert im.getpixel((0, t - 1))[3] == 0, "coin transparent"
            opaques = sum(1 for p in im.getdata() if p[3] == 255)
            assert t * t * 0.25 < opaques < t * t * 0.8, (chemin, opaques)
            assert im.tobytes() == ims[(nom, t)].tobytes()


def test_six_icones_distinctes_par_taille():
    ims = g.generer()
    for t in (22, 24):
        assert len({ims[(n, t)].tobytes() for n in g.NOMS}) == 6


def test_patte_est_dans_les_noms_et_sans_traits_d_aboiement():
    assert "patte" in g.NOMS
    for t in (22, 24):
        im = Image.open(os.path.join(g.ICONES, f"{t}x{t}", "status", "osampo-patte.png"))
        jaune = g.COULEURS["y"]
        assert jaune not in set(im.getdata())
