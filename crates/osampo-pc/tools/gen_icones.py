#!/usr/bin/env python3
"""Icones 22 et 24 px du chien (SPEC-004) pour la zone de notification.

Pixel art dessine a la main en ASCII, une grille par icone. Les grilles 22 px sont
la reference ; les grilles 24 px en sont derivees par duplication de deux colonnes
et de deux lignes de remplissage (le museau et le corps s'elargissent, les yeux
et le nez gardent leur taille), puis retouchees pixel par pixel (PATCHES_24). Ce
ne sont pas des reductions des sprites 1.83" de widgets/osampo.

Usage : python3 gen_icones.py   (reecrit les 12 PNG et la planche de validation)
Dependance : Pillow.
"""
import os

from PIL import Image, ImageDraw

ICI = os.path.dirname(os.path.abspath(__file__))
ICONES = os.path.join(ICI, "..", "icons", "hicolor")
PLANCHE = os.path.join(ICI, "planche_icones.png")

# Teintes de la palette du chiot de widgets/osampo/tools/gen_assets.py.
COULEURS = {
    ".": (0, 0, 0, 0),
    "o": (52, 30, 22, 255),      # contour brun tres sombre
    "f": (226, 168, 102, 255),   # pelage
    "s": (184, 122, 68, 255),    # pelage ombre
    "c": (250, 230, 190, 255),   # creme
    "e": (128, 74, 42, 255),     # oreilles
    "w": (255, 255, 255, 255),   # reflet
    "p": (242, 118, 142, 255),   # langue
    "r": (222, 52, 66, 255),     # collier, toit
    "l": (64, 124, 224, 255),    # laisse, z
    "y": (255, 212, 60, 255),    # traits d'aboiement
}
NOMS = ("assis", "niche", "aboie", "laisse", "patte", "reclame_laisse")


def c(inner, w=22):
    """Ligne centree dans une largeur w, completee de '.'."""
    assert (w - len(inner)) % 2 == 0, inner
    pad = "." * ((w - len(inner)) // 2)
    return pad + inner + pad


def tete(bouche):
    """Tete vue de face ; `bouche` : les deux lignes du bas du museau."""
    return [
        c("oooooooooo"),
        c("ooffffffffffoo"),
        c("oeoffffffffffoeo"),
        c("oeeoffffffffffoeeo"),
        c("oeeoffoffffoffoeeo"),
        c("oeeoffoffffoffoeeo"),
        c("oeeoffffffffffoeeo"),
        c("oeoffccccccffoeo"),
        c("oofcccoocccfoo"),
    ] + bouche


BOUCHE_CONTENTE = [c("ooffccppccffoo"), c("oooooooooo")]
BOUCHE_OUVERTE = [c("ooffcoooocffoo"), c("oooppppooo")]

CORPS = [
    c("rrrrrrrr"),
    c("offccccffo"),
    c("offccccffo"),
    c("offccccffo"),
    c("offfccccfffo"),
    c("offfsccccsfffo"),
    c("offfsccccsfffo"),
    c("offcccffcccffo"),
    c("oooooooooooooo"),
]


def grille(rows, h=22):
    rows = list(rows)
    assert len(rows) <= h
    haut = (h - len(rows)) // 2
    return ["." * 22] * haut + rows + ["." * 22] * (h - len(rows) - haut)


def poser(g, pts):
    g = [list(r) for r in g]
    for x, y, ch in pts:
        g[y][x] = ch
    return ["".join(r) for r in g]


def assis():
    return grille(tete(BOUCHE_CONTENTE) + CORPS)


def aboie():
    g = grille(tete(BOUCHE_OUVERTE) + CORPS)
    marques = [(0, 3), (1, 4), (0, 5), (21, 3), (20, 4), (21, 5)]
    return poser(g, [(x, y, "y") for x, y in marques])


def laisse():
    g = grille(tete(BOUCHE_CONTENTE) + CORPS)
    pts = [(x, 13, "l") for x in range(15, 21)] + [(20, y, "l") for y in range(6, 13)]
    return poser(g, pts)


def patte():
    """Chien assis, la patte avant droite (cote droit de l'image) levee, sans traits d'aboiement.

    Le corps de `assis` perd son bord droit sur les rangees de l'epaule (la patte part de
    la) ; l'avant-bras monte en diagonale le long de la tete et finit par une paume creme.
    """
    g = grille(tete(BOUCHE_CONTENTE) + CORPS)
    efface = [(15, y, ".") for y in (13, 14)] + [(14, 12, "r"), (15, 12, ".")]
    patte_levee = [
        (19, 9, "o"), (20, 9, "o"),
        (18, 10, "o"), (19, 10, "c"), (20, 10, "c"), (21, 10, "o"),
        (18, 11, "o"), (19, 11, "f"), (20, 11, "f"), (21, 11, "o"),
        (17, 12, "o"), (18, 12, "f"), (19, 12, "f"), (20, 12, "o"),
        (16, 13, "o"), (17, 13, "f"), (18, 13, "f"), (19, 13, "o"),
        (15, 14, "o"), (16, 14, "f"), (17, 14, "f"), (18, 14, "o"),
        (15, 15, "f"), (16, 15, "f"), (17, 15, "o"),
    ]
    return poser(g, efface + patte_levee)


def reclame_laisse():
    """Chien assis, la laisse (bleue) tenue dans la gueule, comme `laisse.png` de la board.

    La laisse traverse la gueule, retombe sur le poitrail puis finit en boucle en bas a
    gauche. Se distingue de `laisse` (promenade, laisse seule) par le chien qui la tient.
    """
    bouche = [c("ooffcllllcffoo"), c("ooollllooo")]
    g = grille(tete(bouche) + CORPS)
    brin = [(10, y, "l") for y in range(12, 17)] + [(11, 12, "l")]
    boucle = [(9, 16, "l"), (8, 17, "l"), (3, 15, "l"), (3, 16, "l"), (3, 17, "l"),
              (3, 18, "l"), (4, 19, "l"), (5, 19, "l"), (6, 19, "l"), (7, 18, "l")]
    return poser(g, brin + boucle)


def niche():
    toit = [c("o" + "r" * n + "o") for n in (0, 2, 4, 6, 8, 10, 12, 14)]
    toit[0] = c("oo")
    mur = c("o" + "f" * 14 + "o")
    porte = c("o" + "ffff" + "oooooo" + "ffff" + "o")
    yeux = c("o" + "ffff" + "owoowo" + "ffff" + "o")
    rows = toit + [c("o" * 18), mur, porte, yeux, porte, porte, porte, c("o" * 16)]
    g = grille(rows)
    z = [(17, 1), (18, 1), (19, 1), (19, 2), (18, 3), (17, 4), (18, 4), (19, 4)]
    return poser(g, [(x, y, "l") for x, y in z])


GRILLES_22 = {"assis": assis, "niche": niche, "aboie": aboie, "laisse": laisse, "patte": patte,
              "reclame_laisse": reclame_laisse}

# Retouches 24 px : (x, y, caractere), appliquees apres le remplissage.
PATCHES_24 = {n: [] for n in NOMS}


def vers_24(g22):
    cols = list(range(0, 10)) + [9] + [10, 11, 12] + [12] + list(range(13, 22))
    lignes = list(range(0, 8)) + [7] + list(range(8, 15)) + [14] + list(range(15, 22))
    return ["".join(g22[y][x] for x in cols) for y in lignes]


def png(g, chemin):
    h, w = len(g), len(g[0])
    im = Image.new("RGBA", (w, h))
    for y, r in enumerate(g):
        assert len(r) == w, (chemin, y, len(r))
        for x, ch in enumerate(r):
            im.putpixel((x, y), COULEURS[ch])
    im.save(chemin, optimize=True)
    return im


def generer():
    ims = {}
    for nom in NOMS:
        g22 = GRILLES_22[nom]()
        g24 = poser(vers_24(g22), PATCHES_24[nom])
        for taille, g in ((22, g22), (24, g24)):
            dossier = os.path.join(ICONES, f"{taille}x{taille}", "status")
            os.makedirs(dossier, exist_ok=True)
            ims[(nom, taille)] = png(g, os.path.join(dossier, f"osampo-{nom}.png"))
    return ims


def planche(ims):
    """Pour chaque fond (clair, sombre) : 22 et 24 px a taille reelle, puis agrandis x8."""
    fonds = {"clair": (239, 240, 241, 255), "sombre": (43, 46, 52, 255)}
    k, m = 8, 10
    cell = 24 * k
    colonnes = [(f, t, z) for f in fonds for z in (1, k) for t in (22, 24)]
    largeur = lambda z: (24 if z == 1 else cell) + m
    haut = cell + 2 * m
    im = Image.new("RGBA", (sum(largeur(z) for _, _, z in colonnes) + m, haut * len(NOMS) + 20),
                   (128, 128, 128, 255))
    d = ImageDraw.Draw(im)
    x = m
    for f, t, z in colonnes:
        d.text((x, 4), f"{f[0]}{t}" + ("x8" if z > 1 else ""), fill=(255, 255, 255, 255))
        w = 24 if z == 1 else cell
        for i, nom in enumerate(NOMS):
            y0 = 20 + i * haut + m
            d.rectangle([x - 2, y0 - 2, x + w + 1, y0 + w + 1], fill=fonds[f])
            src = ims[(nom, t)]
            if z > 1:
                src = src.resize((t * z, t * z), Image.NEAREST)
            im.alpha_composite(src, (x + (w - src.width) // 2, y0 + (w - src.height) // 2))
        x += largeur(z)
    im.convert("RGB").save(PLANCHE, optimize=True)


if __name__ == "__main__":
    planche(generer())
