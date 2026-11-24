#!/usr/bin/env python3
"""Genere les images et les sons du chiot osampo.

Poses de face (repos, patte, content, aboie, boude, tournis, laisse, lache, bain,
bain_mousse, bain_fin, os, os_fin) :
derivees du chiot assis de face de strawheart (« Animals and Tea »,
OpenGameArt, CC-BY-SA 3.0, attribution http://mtsids.com/), agrandi x2 dans
la case et retouche au pixel. Ces planches sont sous CC-BY-SA 3.0, voir
assets/third_party/strawheart_animals_and_tea/SOURCE.md.
Poses de profil (dort, court, court_droite, balle, roulade, promenade*), niche, fonds,
icones et sons : dessines ici, en pixel art original ; le chiot de profil
reprend seulement la palette de strawheart (le pack rmazanek/dog n'est pas
utilise, voir assets/third_party/rmazanek_dog/SOURCE.md).

Sorties (relatives a widgets/osampo/) :
  ui/sprites/<anim>.png        planche horizontale, cases 64 x 64, <= 16 couleurs
  ui/sprites/niche.png         64 x 64, meme cadre que le chiot (affiche x3)
  ui/sprites/fond_jour.png     240 x 284
  ui/sprites/fond_nuit.png     240 x 284
  ui/sprites/besoin_<b>_<n>.png, horloge_barree.png, lien.png, lien_barre.png   16 x 16
  sounds/<cue>.wav             mono, 16 kHz, 16 bits
  tools/planche_contact.png    toutes les cases x4 avec leur nom

Lancer : python3 -I widgets/osampo/tools/gen_assets.py
"""
import math
import os
import site
import sys
import wave

# `python3 -I` ignore le site-packages utilisateur, ou numpy est installe
# (le Python systeme est « externally managed ») : on l'ajoute en dernier.
if site.getusersitepackages() not in sys.path:
    sys.path.append(site.getusersitepackages())

import numpy as np  # noqa: E402
from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STRAWHEART_DOG = os.path.join(ROOT, "assets", "third_party",
                              "strawheart_animals_and_tea", "pixel-ani-dog.gif")
SPRITES = os.path.join(ROOT, "ui", "sprites")
SOUNDS = os.path.join(ROOT, "sounds")
CONTACT = os.path.join(ROOT, "tools", "planche_contact.png")

S = 64          # cote d'une case
GROUND = 58     # ligne du sol dans la case (premiere ligne sous les pattes)

# --- Palette indexee (16 entrees) de la niche, des icones et des effets -------
# Les dessins sur Canvas n'utilisent que ces indices ; les couleurs viennent de
# make_palette(COATS[COAT]). Le chiot lui-meme est dessine avec la palette de
# strawheart (SH, plus bas), en RGBA ; seules ses retouches (balle, coeurs,
# etoiles, laisse...) passent par ces indices.
T, OUT, FUR, SHADE, CREAM, EAR, WHITE, PINK, RED, BALL, LEASH, STAR, ZBLUE, \
    SHADOW, DUST, GREY = range(16)

# Teintes brunes reprises par la niche et les icones (ancienne robe du chiot).
COATS = {
    "brun_clair": {
        "outline": (52, 30, 22, 255),    # contour brun tres sombre
        "base": (226, 168, 102, 255),    # pelage
        "shade": (184, 122, 68, 255),    # pelage ombre
        "light": (250, 230, 190, 255),   # creme (toit, bol)
        "ears": (128, 74, 42, 255),      # oreilles
    },
}
COAT = "brun_clair"

# Couleurs des accessoires et effets.
ACCENTS = {
    T: (0, 0, 0, 0),
    WHITE: (255, 255, 255, 255),     # reflet de l'oeil
    PINK: (242, 118, 142, 255),      # langue, joues, coussinets
    RED: (222, 52, 66, 255),         # collier, coeurs, toit de la niche
    BALL: (206, 230, 64, 255),       # balle de tennis
    LEASH: (64, 124, 224, 255),      # laisse, gouttes, bol
    STAR: (255, 212, 60, 255),       # etoiles, traits d'aboiement
    ZBLUE: (160, 200, 255, 255),     # les « z »
    SHADOW: (0, 0, 0, 72),           # ombre au sol
    DUST: (216, 206, 186, 255),      # poussiere de course
    GREY: (128, 128, 140, 255),      # nuage de bouderie
}


def make_palette(coat):
    pal = dict(ACCENTS)
    pal.update({OUT: coat["outline"], FUR: coat["base"],
                SHADE: coat["shade"], CREAM: coat["light"],
                EAR: coat["ears"]})
    return [pal[i] for i in range(16)]


PALETTE = make_palette(COATS[COAT])


# --- Masques et canevas ------------------------------------------------------
class Mask:
    """Fabrique de masques booleens 64 x 64 dessines avec PIL."""

    def __init__(self, w=S, h=S):
        self.w, self.h = w, h

    def _new(self):
        img = Image.new("L", (self.w, self.h), 0)
        return img, ImageDraw.Draw(img)

    def ell(self, cx, cy, rx, ry):
        img, d = self._new()
        d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=1)
        return np.array(img, bool)

    def seg(self, p0, p1, w):
        """Segment epais a bouts arrondis."""
        img, d = self._new()
        d.line([p0, p1], fill=1, width=w)
        r = (w - 1) / 2
        for (x, y) in (p0, p1):
            d.ellipse([x - r, y - r, x + r, y + r], fill=1)
        return np.array(img, bool)

    def poly(self, pts):
        img, d = self._new()
        d.polygon(pts, fill=1)
        return np.array(img, bool)

    def rect(self, x0, y0, x1, y1):
        m = np.zeros((self.h, self.w), bool)
        m[max(0, y0):y1 + 1, max(0, x0):x1 + 1] = True
        return m


def dilate4(m):
    o = m.copy()
    o[1:, :] |= m[:-1, :]
    o[:-1, :] |= m[1:, :]
    o[:, 1:] |= m[:, :-1]
    o[:, :-1] |= m[:, 1:]
    return o


def shift(m, dx, dy):
    o = np.zeros_like(m)
    h, w = m.shape
    xs0, xs1 = max(0, -dx), min(w, w - dx)
    ys0, ys1 = max(0, -dy), min(h, h - dy)
    o[ys0 + dy:ys1 + dy, xs0 + dx:xs1 + dx] = m[ys0:ys1, xs0:xs1]
    return o


class Canvas:
    def __init__(self, w=S, h=S):
        self.a = np.zeros((h, w), np.uint8)
        self.M = Mask(w, h)

    def part(self, mask, fill, shade=None, outline=True, rim=2):
        """Pose une forme : contour 1 px, remplissage, ombre sur le bas."""
        if outline:
            self.a[dilate4(mask) & ~mask] = OUT
        self.a[mask] = fill
        if shade is not None:
            bottom = mask & ~shift(mask, 0, -rim)
            self.a[bottom] = shade

    def paint(self, mask, color):
        self.a[mask] = color

    def px(self, x, y, color):
        if 0 <= x < self.a.shape[1] and 0 <= y < self.a.shape[0]:
            self.a[y, x] = color

    def bitmap(self, rows, x, y, color, outline=True, extra=None):
        """Petit motif ('X' = couleur, 'o' = couleur extra) avec contour."""
        m = np.zeros_like(self.a, bool)
        e = np.zeros_like(self.a, bool)
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                yy, xx = y + j, x + i
                if 0 <= yy < m.shape[0] and 0 <= xx < m.shape[1]:
                    if ch in "Xo":
                        m[yy, xx] = True
                    if ch == "o":
                        e[yy, xx] = True
        if outline:
            self.a[dilate4(m) & ~m] = OUT
        self.a[m] = color
        if extra is not None:
            self.a[e] = extra


def to_image(a, palette=PALETTE):
    lut = np.array(palette, np.uint8)
    return Image.fromarray(lut[a], "RGBA")


# --- Le chiot ----------------------------------------------------------------
HEART = [".X.X.", "XoXXX", "XXXXX", ".XXX.", "..X.."]
HEART_S = ["X.X", "XXX", ".X."]
STAR_B = ["..X..", ".XXX.", "XXXXX", ".XXX.", ".X.X."]
Z_BIG = ["XXXXX", "...X.", "..X..", ".X...", "XXXXX"]
Z_SMALL = ["XXXX", "..X.", ".X..", "XXXX"]


def ball(c, x, y):
    c.part(c.M.ell(x, y, 3, 3), BALL)
    c.px(x - 1, y - 2, WHITE)
    c.px(x - 2, y - 1, WHITE)


# --- Animations --------------------------------------------------------------
# Repos : chiot assis de face de strawheart, 23 x 25, deux images (queue en bas,
# queue levee). Couleurs d'origine gardees (8 couleurs + ombre au sol), dans
# la palette SH.
REPOS_ZOOM = 2           # 23 x 25 -> 46 x 50 dans la case 64 x 64
REPOS_BODY_CX = 9        # colonne du milieu du corps dans la source (sans queue)
REPOS_SEQ = [0, 1, 0, 1, 0, 0, 0, 0]   # deux battements de queue puis pause
REPOS_CLIGNE = 5         # case de REPOS_SEQ ou le chiot cligne des yeux
# Yeux redessines au pixel de la case (la source n'a qu'un pixel par oeil) :
# meme dessin que l'oeil "open" des autres poses, 2 x 4 avec reflet en haut a
# gauche, et un arc pour l'oeil ferme. (x, y) = coin haut gauche de l'oeil
# agrandi (2 x 2) dans la case.
REPOS_YEUX = [(25, 22), (37, 22)]
REPOS_PUPILLE = (0, 0, 0, 255)          # noir de la source
REPOS_POIL = (255, 189, 96, 255)        # pelage de la source autour des yeux


def strawheart_frames():
    """Images RGBA du GIF d'origine, sans modification."""
    gif = Image.open(STRAWHEART_DOG)
    out = []
    for k in range(gif.n_frames):
        gif.seek(k)
        out.append(gif.convert("RGBA"))
    return out


# Couleurs de la source strawheart, pour retoucher ses images au pixel d'origine.
SH = {".": (0, 0, 0, 0), "a": (123, 66, 25, 255), "b": (255, 189, 96, 255),
      "d": (208, 142, 61, 255),
      "c": (255, 199, 115, 255), "e": (0, 0, 0, 255),
      "w": ACCENTS[WHITE], "L": ACCENTS[LEASH], "z": ACCENTS[ZBLUE],
      "f": (255, 235, 194, 255),
      "p": (232, 128, 120, 255),
      "g": (206, 206, 218, 255)}   # ombre de l'os (SP5f)   # coussinets (ajout, absent de la source)


def sh_stamp(a, rows, x, y):
    """Pose un motif ('.' = transparent, laisse la source) sur une image source."""
    for dy, row in enumerate(rows):
        for dx, ch in enumerate(row):
            if ch != ".":
                a[y + dy, x + dx] = SH[ch]


def strawheart_case(im):
    """Image source 23 x 25 -> case 64 x 64 : agrandie, posee au sol (sans ombre)."""
    big = im.resize((im.width * REPOS_ZOOM, im.height * REPOS_ZOOM),
                    Image.NEAREST)
    x = 32 - REPOS_BODY_CX * REPOS_ZOOM - REPOS_ZOOM // 2
    y = GROUND - big.height
    case = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    case.paste(big, (x, y))
    return case


def sh_fin(case, saut=0, dx=0, sol=GROUND):
    """Remonte le chiot de `saut` pixels de case (et le decale de `dx`), puis
    pose l'ombre au sol (ligne `sol`)."""
    if saut or dx:
        haut = Image.new("RGBA", (S, S), (0, 0, 0, 0))
        haut.paste(case, (dx, -saut))
        case = haut
    a = np.array(case)
    sh = Mask().ell(32, sol, 17 - saut, 1) & (a[:, :, 3] == 0)
    a[sh] = ACCENTS[SHADOW]
    return Image.fromarray(a, "RGBA")


def anim_repos():
    """Liste d'images RGBA 64 x 64 (pas de Canvas : couleurs d'origine)."""
    cases = [strawheart_case(im) for im in strawheart_frames()]
    return [sh_fin(repos_yeux(cases[i],
                              "fermes" if k == REPOS_CLIGNE else "ouverts"))
            for k, i in enumerate(REPOS_SEQ)]


# Patte (Claude attend) : la patte avant gauche monte et se pose contre l'ecran,
# face au spectateur, coussinets visibles ; elle appuie, puis redescend.
PATTE_MI = [".aaa.", "abbba", "abbba", "adbda", ".aaa."]
PATTE_FACE = [".aaaaa.",
              "apbpbpa",
              "abbbbba",
              "abpppba",
              "abpppba",
              "abbbbba",
              ".aaaaa."]
PATTE_APPUIE = ["..aaaaa..",
                ".abbbbba.",
                "abpbpbpba",
                "abbbbbbba",
                "abbpppbba",
                "abbpppbba",
                "abbbbbbba",
                ".abbbbba.",
                "..aaaaa.."]
PATTE_HAUTEURS = [0, 1, 2, 3, 4, 4, 3, 2]


def patte_levee(im, h):
    """h = 0 au sol, 1 decollee, 2 devant le poitrail, 3 contre l'ecran,
    4 appuyee (plus grande, a plat contre la vitre)."""
    a = np.array(im)
    if h == 1:                      # le coussinet remonte d'une rangee
        a[22, 6:9] = a[23, 6:9]
        a[23, 6:9] = SH["a"]
    elif h >= 2:                    # jambe levee : le ventre prend sa place
        a[18:21, 6:9] = SH["b"]
        a[21:23, 6:9] = SH["d"]
        a[23, 6:9] = SH["a"]
        motif, x, y = {2: (PATTE_MI, 5, 15), 3: (PATTE_FACE, 4, 12),
                       4: (PATTE_APPUIE, 3, 11)}[h]
        sh_stamp(a, motif, x, y)
    return Image.fromarray(a, "RGBA")


def anim_patte():
    src = strawheart_frames()
    return [sh_fin(repos_yeux(strawheart_case(patte_levee(src[k % 2], h))))
            for k, h in enumerate(PATTE_HAUTEURS)]


def repos_yeux(case, mode="ouverts"):
    """Copie de la case avec les yeux redessines : "ouverts" (2 x 4, reflet),
    "fermes" (arc vers le bas, clignement), "rieurs" (^), "tristes"
    (paupiere lourde) ou "vertiges" (X)."""
    a = np.array(case)
    for (x, y) in REPOS_YEUX:
        # contrat : la source a bien un oeil noir 2 x 2 a cet endroit
        assert (a[y:y + 2, x:x + 2] == REPOS_PUPILLE).all(), (x, y)
        a[y:y + 2, x:x + 2] = REPOS_POIL
        if mode == "ouverts":
            a[y - 1:y + 3, x:x + 2] = REPOS_PUPILLE
            a[y - 1, x] = (255, 255, 255, 255)
        else:
            arc = {"fermes": ((-1, 0), (0, 1), (1, 1), (2, 0)),
                   "rieurs": ((-1, 1), (0, 0), (1, 0), (2, 1))}.get(mode)
            if mode == "vertiges":       # X de 4 x 4
                arc = ((-1, -1), (2, -1), (0, 0), (1, 0), (0, 1), (1, 1),
                       (-1, 2), (2, 2))
            if mode == "tristes":
                # pupille basse sous une paupiere plate qui tombe cote exterieur
                a[y + 1:y + 3, x:x + 2] = REPOS_PUPILLE
                gauche = x < 32
                arc = ((-1, 1), (0, 0), (1, 0), (2, 0)) if gauche else \
                      ((-1, 0), (0, 0), (1, 0), (2, 1))
            for (dx, dy) in arc:
                a[y + dy, x + dx] = REPOS_PUPILLE
    return Image.fromarray(a, "RGBA")


# Dort (heures calmes, ou board face contre table) : sous une couverture bleue
# a motifs remontee jusqu'au cou, tete de profil posee au sol, le museau au
# niveau du bas de la couverture, queue en tire-bouchon. Dessin original au
# pixel source, couleurs de strawheart (adaptation d'une idee trouvee par
# l'auteur), deux souffles. L'oeil ferme est dessine au pixel de la case.
DORT = [
    ["................................",
     "................................",
     ".......................aaa......",
     "......................abbba.....",
     ".................aaaaaababa.....",
     "......aaaaa...aaawwLLLLaaba.....",
     "....aacccaadaawwLLLLLLLLLa......",
     "...acccccadddawwLLLzLLLzLLa.....",
     "..abccbbbaddawwzLLLLLLLLLLza....",
     "..abbbbbaddawwLLLLLLLLLLLLLa....",
     "..abbbbbadddawwLLLLLLzLLLLLa....",
     "..abbbbbbaddawwLLzLLLLLLLzLa....",
     "aaabbbbbbbadawwLLLLLLLLLLLLa....",
     "effffbbbbbbaawwLLLLzLLLzLLLa....",
     "affffpbbbbbbawwLLLLLLLLLLLLa....",
     "aaaaaaaaaaaaaaaaaaaaaaaaaaa....."],
    ["................................",
     "................................",
     ".......................aaa......",
     ".................aaaaaabbba.....",
     "................awwLLLLaaba.....",
     "......aaaaa...aawwLLLLLLaba.....",
     "....aacccaadaawwLLLLLLLLLa......",
     "...acccccadddawwLLLzLLLzLLa.....",
     "..abccbbbaddawwzLLLLLLLLLLza....",
     "..abbbbbaddawwLLLLLLLLLLLLLa....",
     "..abbbbbadddawwLLLLLLzLLLLLa....",
     "..abbbbbbaddawwLLzLLLLLLLzLa....",
     "aaabbbbbbbadawwLLLLLLLLLLLLa....",
     "effffbbbbbbaawwLLLLzLLLzLLLa....",
     "affffpbbbbbbawwLLLLLLLLLLLLa....",
     "aaaaaaaaaaaaaaaaaaaaaaaaaaa....."],
]
DORT_X = 8                       # coin gauche dans la case (place pour la bulle)
DORT_BAS = 3                     # devant la niche : sol 3 px plus bas (perspective)
DORT_Y0 = GROUND - 2 * 16 + DORT_BAS        # haut de la grille dans la case
DORT_OEIL = (DORT_X + 8, DORT_Y0 + 22)      # debut de l'arc, pixels case
DORT_NEZ = (DORT_X - 1, DORT_Y0 + 27)       # la bulle s'accroche ici


def dort_case(grille):
    a = np.zeros((len(grille), len(grille[0]), 4), np.uint8)
    for y, row in enumerate(grille):
        for x, ch in enumerate(row):
            a[y, x] = SH[ch]
    im = Image.fromarray(a, "RGBA")
    big = im.resize((im.width * REPOS_ZOOM, im.height * REPOS_ZOOM),
                    Image.NEAREST)
    case = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    case.paste(big, (DORT_X, DORT_Y0))
    return case


def anim_dort():
    z = [  # (motif, x, y) : les z montent a droite, hors du toit de la niche
        [(Z_SMALL, 50, 26)],
        [(Z_SMALL, 52, 19), (Z_SMALL, 49, 27)],
        [(Z_BIG, 53, 11), (Z_SMALL, 51, 20)],
        [(Z_BIG, 55, 3), (Z_BIG, 52, 13)],
    ]
    souffles = [0, 1, 1, 0]
    bulle = [1, 2, 3, 2]             # rayon de la bulle au bout de la truffe
    out = []
    for i in range(4):
        case = sh_fin(dort_case(DORT[souffles[i]]), sol=GROUND + DORT_BAS)
        a = np.array(case)
        ox, oy = DORT_OEIL               # oeil ferme : arc vers le bas
        for (dx, dy) in ((0, 0), (1, 1), (2, 1), (3, 1), (4, 0)):
            a[oy + dy, ox + dx] = SH["e"]
        case = Image.fromarray(a, "RGBA")
        c = Canvas()
        for (rows, x, y) in z[i]:
            c.bitmap(rows, x, y, ZBLUE)
        r = bulle[i]
        bx, by = DORT_NEZ[0] - r, DORT_NEZ[1]
        c.part(c.M.ell(bx, by, r, r), ZBLUE)
        c.px(bx - r + 1, by - r + 1, WHITE)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


def bark_lines(c, x, y):
    for (x0, y0, x1, y1) in ((x, y - 5, x + 3, y - 7), (x + 1, y, x + 4, y),
                             (x, y + 5, x + 3, y + 7)):
        c.part(c.M.seg((x0, y0), (x1, y1), 1), STAR)


# Aboie (rappel) : la gueule s'ouvre une image sur deux, le chiot se dresse
# un peu et des traits d'aboiement partent des deux cotes de la tete.
# Gueule ouverte sous la truffe (pixels source, coin haut gauche en 7, 10) :
# large, langue au fond, bas arrondi. Choisie par l'auteur parmi trois tailles.
GUEULE = ["eeeee",
          "epppe",
          ".eee."]


def gueule_ouverte(im):
    """Gueule ouverte sous la truffe, langue au fond (pixels source)."""
    a = np.array(im)
    for dy, row in enumerate(GUEULE):
        for dx, ch in enumerate(row):
            if ch != ".":
                a[10 + dy, 7 + dx] = SH[ch]
    return Image.fromarray(a, "RGBA")


def anim_aboie():
    src = strawheart_frames()
    out = []
    for i in range(4):
        fort = i % 2 == 1
        im = gueule_ouverte(src[i % 2]) if fort else src[i % 2]
        case = sh_fin(repos_yeux(strawheart_case(im)), 2 if fort else 0)
        if fort:
            c = Canvas()
            bark_lines(c, 53, 22)
            gauche = Canvas()
            bark_lines(gauche, 53, 22)
            gauche.a = gauche.a[:, ::-1].copy()
            for k in (c, gauche):
                case.alpha_composite(to_image(k.a))
        out.append(case)
    return out


def sh_vers_case(x, y, saut=0):
    """Pixel source (x, y) -> coin haut gauche du bloc 2 x 2 dans la case."""
    x0 = 32 - REPOS_BODY_CX * REPOS_ZOOM - REPOS_ZOOM // 2
    y0 = GROUND - 25 * REPOS_ZOOM
    return x0 + x * REPOS_ZOOM, y0 + y * REPOS_ZOOM - saut


def anim_laisse():
    """Le chiot reclame la laisse : collier rouge (seulement dans cette pose,
    pour marquer la sortie), laisse accrochee a l'anneau qui traine a gauche
    jusqu'au sol ; il trepigne, la queue bat."""
    src = strawheart_frames()
    sauts = [0, 2, 2, 0]
    out = []
    for i in range(4):
        case = sh_fin(repos_yeux(strawheart_case(src[i % 2])), sauts[i])
        c = Canvas()
        M = c.M
        cx, cy = sh_vers_case(6, 12, sauts[i])      # collier sous le menton
        c.part(M.rect(cx, cy, cx + 13, cy + 1), RED)
        ax, ay = sh_vers_case(7, 12, sauts[i])      # anneau, a gauche du centre
        gx, gy = ax + 2, ay - 6
        anneau = M.rect(ax, ay + 1, ax + 1, ay + 3)
        brin = (M.seg((ax, ay + 3), (gx - 6, gy + 12), 1)
                | M.seg((gx - 6, gy + 12), (gx - 9, GROUND - 3), 1)
                | M.seg((gx - 9, GROUND - 3), (gx - 14, GROUND - 2), 1))
        boucle = M.ell(gx - 16, GROUND - 5, 3, 3) & ~M.ell(gx - 16, GROUND - 5, 1, 1)
        c.part(anneau | brin | boucle, LEASH)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


def hearts(c, t):
    """Coeurs qui montent ; t = phase 0..5."""
    for (x0, phase) in ((50, 0), (56, 3)):
        k = (t + phase) % 6
        y = 22 - 3 * k
        if k < 5:
            c.bitmap(HEART if k > 1 else HEART_S, x0 + (k % 2), y, RED,
                     extra=WHITE if k > 1 else None)


# Content (apres une caresse) : yeux rieurs, langue, petits sauts, queue qui
# bat vite, coeurs qui montent.
CONTENT_SAUTS = [0, 0, 2, 2, 0, 0]


def langue(im):
    """Langue rose qui pend sous la truffe (pixels source)."""
    a = np.array(im)
    a[11, 9] = SH["p"]
    a[12, 9] = SH["p"]
    return Image.fromarray(a, "RGBA")


def anim_content():
    src = strawheart_frames()
    out = []
    for i, saut in enumerate(CONTENT_SAUTS):
        case = sh_fin(repos_yeux(strawheart_case(langue(src[i % 2])), "rieurs"),
                      saut)
        c = Canvas()
        hearts(c, i)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


# Court : chiot de profil tourne vers la gauche, galop rotatif en 6 images
# (d'apres un cycle de galop de chien en 12 images, une image sur deux).
# Dessine a la resolution source de strawheart puis agrandi x2, comme dort.
# Les pattes avant reviennent sous le ventre et croisent les pattes arriere a
# la phase compressee ; les pattes levees se plient au genou / au poignet.
COURT_W, COURT_H = 32, 20
COURT_TETE = ["......aaaaa......",
              "....aacccccaa....",
              "...accccccccbaa..",
              "..abccbbbbbbbbba.",
              "..abbbbbbbbbadda.",
              "aaabbebbbbbadddda",
              "effffebbbbbadddda",
              "afffffbbbbbaddda.",
              ".aaaaafbbbbbaaa..",
              "..apppafbbbbba...",
              "...apa.aaaaaa....",
              "....a............"]
# (dy avant, dy arriere, demi-longueur du corps,
#  (genou, pied) : avant fond, avant proche, arriere fond, arriere proche)
COURT = [
    (0, 1, 7.8, ((12, 15), (15, 16)), ((10, 14), (8, 16)),
     ((27, 14), (29, 16)), ((23, 15), (22, 18))),   # arriere proche au sol
    (0, 0, 8.4, ((9, 13), (6, 14)), ((11, 15), (9, 18)),
     ((28, 14), (31, 15)), ((27, 15), (30, 17))),   # avant proche au sol
    (-1, 0, 8.6, ((10, 14), (7, 16)), ((14, 15), (14, 18)),
     ((27, 14), (30, 16)), ((26, 15), (29, 17))),   # corps etire
    (0, -1, 8.0, ((16, 15), (18, 18)), ((12, 15), (14, 17)),
     ((26, 15), (29, 17)), ((27, 14), (28, 17))),   # avant fond au sol
    (-1, -1, 7.6, ((17, 15), (19, 18)), ((14, 15), (17, 16)),
     ((24, 14), (23, 17)), ((23, 15), (21, 17))),   # suspension
    (0, 0, 7.0, ((17, 15), (20, 18)), ((11, 14), (9, 16)),
     ((22, 14), (19, 17)), ((21, 15), (17, 18))),   # compresse, pattes croisees
]


def _g_disk(cx, cy, rx, ry):
    return {(x, y) for y in range(int(cy - ry) - 1, int(cy + ry) + 2)
            for x in range(int(cx - rx) - 1, int(cx + rx) + 2)
            if ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0}


def _g_membre(pts, rs):
    """Membre effile : segments entre articulations, rayon interpole."""
    out = set()
    for (p, q), (r0, r1) in zip(zip(pts, pts[1:]), zip(rs, rs[1:])):
        for k in range(11):
            t = k / 10
            r = r0 + (r1 - r0) * t
            out |= _g_disk(p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, r, r)
    return out


def _g_piece(g, pts, fill, interieur, ymin=None):
    """Remplit pts. Contour 'a' vers le vide (et sous ymin) ; ailleurs, sur ce
    qui est deja dessine, le contour prend la couleur `interieur`."""
    h, w = len(g), len(g[0])
    for (x, y) in pts:
        if 0 <= x < w and 0 <= y < h:
            g[y][x] = fill
    for (x, y) in pts:
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            qx, qy = x + dx, y + dy
            if (qx, qy) in pts or not (0 <= qx < w and 0 <= qy < h):
                continue
            if g[qy][qx] in ".a" or (ymin is not None and qy >= ymin):
                g[qy][qx] = "a"
            else:
                g[qy][qx] = interieur


def _court_patte(g, haut, genou, pied, fill, epais, ymin):
    pts = _g_membre([haut, genou, (pied[0], pied[1] - 1)], [epais, 0.9, 0.6])
    pts |= _g_disk(pied[0] - 0.5, pied[1] - 0.6, 1.3, 0.9)
    _g_piece(g, pts, fill, fill, ymin)


def profil_grille(dya, dyr, rx, af, ap, rf, rp, tete=(0, 0), queue=0, ox=0,
                  avec_langue=True, avec_tete=True, avec_queue=True):
    """Chiot de profil tourne vers la gauche, a la resolution source.
    tete = decalage (dx, dy) de la tete (la baisser vers le sol), ox = decalage
    de tout le chiot vers la droite."""
    g = [["."] * COURT_W for _ in range(COURT_H)]
    by = 10
    sh = (14, by + 1 + dya)                                  # epaule
    hp = (int(round(19.5 + rx - 4)), by + 1 + dyr)           # hanche
    ymin = by + 3            # sous le ventre, les pattes ont leur contour
    _court_patte(g, sh, *af, "d", 1.3, ymin)                 # pattes du fond
    _court_patte(g, (hp[0] - 1, hp[1]), *rf, "d", 1.6, ymin)
    if avec_queue:
        _g_piece(g, _g_membre([(hp[0] + 3, hp[1] - 3),
                               (hp[0] + 5, hp[1] - 6 - queue)], [0.7, 0.7]),
                 "b", "a")                                   # queue
    cx = 14 + rx - 0.5
    corps = set()
    for y in range(COURT_H):
        for x in range(COURT_W):
            t = min(1, max(0, (x - 12) / (hp[0] - 12)))      # dos incline
            cy = by + dya + (dyr - dya) * t
            if ((x - cx) / rx) ** 2 + ((y - cy) / 3.8) ** 2 <= 1:
                corps.add((x, y))
    _g_piece(g, corps, "b", "a")
    for (x, y) in corps:                                     # ventre clair
        if (x, y + 1) not in corps and g[y][x] == "b" and 13 < x < hp[0]:
            g[y][x] = "c"
    _court_patte(g, sh, *ap, "b", 1.6, ymin)                 # pattes proches
    _court_patte(g, (hp[0], hp[1] - 1), *rp, "b", 2.2, ymin)
    for (x, y) in ((hp[0] - 2, hp[1] - 3), (hp[0] - 3, hp[1] - 2),
                   (hp[0] - 3, hp[1] - 1)):                  # pli de la cuisse
        if g[y][x] in "bc":
            g[y][x] = "d"
    tdx, tdy = tete
    for y, row in enumerate(COURT_TETE if avec_tete else []):
        for x, ch in enumerate(row):
            if not avec_langue and y >= 9 and x <= 6:
                continue                     # gueule fermee sur la balle
            if ch != "." and 0 <= y + 1 + dya + tdy < COURT_H and 0 <= x + tdx:
                g[y + 1 + dya + tdy][x + tdx] = ch
    if ox:
        g = [["."] * ox + row[:COURT_W - ox] for row in g]
    return g


COURT_ARRIERE_DX = 2   # pattes arriere reculees sous la croupe (choix G2 de l'auteur)


def court_grille(i):
    """Image i du galop, pattes arriere reculees de COURT_ARRIERE_DX (choix G2
    de l'auteur, 2026-11-23 : la croupe ne doit pas depasser derriere les pattes ;
    les pattes tendues peuvent toucher le bord de la case)."""
    dya, dyr, rx, af, ap, rf, rp = COURT[i]
    recule = lambda patte: tuple((min(COURT_W - 1, x + COURT_ARRIERE_DX), y)
                                 for x, y in patte)
    return profil_grille(dya, dyr, rx, af, ap, recule(rf), recule(rp), queue=i % 2)


def profil_cases(grilles):
    """Grilles source -> cases 64 x 64, pieds sur GROUND - 1, avec ombre.
    Rend aussi l'origine (x0, y0) de la grille agrandie dans la case."""
    bas = max(y for g in grilles for y, row in enumerate(g)
              if any(ch != "." for ch in row))
    y0 = GROUND - (bas + 1) * REPOS_ZOOM
    out = []
    for g in grilles:
        a = np.array([[SH[ch] for ch in row] for row in g], np.uint8)
        im = Image.fromarray(a, "RGBA")
        big = im.resize((im.width * REPOS_ZOOM, im.height * REPOS_ZOOM),
                        Image.NEAREST)
        case = Image.new("RGBA", (S, S), (0, 0, 0, 0))
        case.paste(big, (0, y0))
        out.append(sh_fin(case))
    return out, (0, y0)


def anim_court():
    return profil_cases([court_grille(i) for i in range(len(COURT))])[0]


def anim_court_droite():
    """Meme course vers la droite (balle lancee de l'autre cote) : miroir."""
    return [c.transpose(Image.FLIP_LEFT_RIGHT) for c in anim_court()]


def anim_lache():
    """Il lache la balle : assis de face, balle dans la gueule, il la lache,
    elle rebondit a ses pattes, puis il est content (yeux rieurs, langue)."""
    src = strawheart_frames()
    gx, gy = sh_vers_case(9, 11)
    gx, gy = gx + 1, gy + 1                  # centre de la gueule, pixels case
    sol = GROUND - 4                         # centre de la balle posee
    # (image source, yeux, langue/gueule, centre de la balle ou None)
    seq = [
        (0, "ouverts", None, (gx, gy + 1)),          # balle en gueule
        (1, "ouverts", None, (gx, gy + 1)),
        (0, "ouverts", "gueule", (gx, gy + 9)),      # il la lache, elle tombe
        (1, "ouverts", "gueule", (gx, sol)),         # elle touche le sol
        (0, "rieurs", "langue", (gx, sol - 4)),      # petit rebond
        (1, "rieurs", "langue", (gx, sol)),
    ]
    out = []
    for i, (k, yeux, bouche, pos) in enumerate(seq):
        im = src[k]
        if bouche == "gueule":
            im = gueule_ouverte(im)
        elif bouche == "langue":
            im = langue(im)
        case = sh_fin(repos_yeux(strawheart_case(im), yeux))
        c = Canvas()
        x, y = pos
        ball(c, x, y)
        if i == 4:                            # ombre de la balle en l'air
            c.paint(c.M.ell(x, GROUND - 1, 2, 0.6) & (c.a == T), SHADOW)
        if yeux == "rieurs":
            hearts(c, i)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


# Roulade (d'apres des croquis de chiens qui se roulent sur le dos), au sol :
# couche sur le ventre, puis sur le flanc (pattes tendues, menton sur les
# pattes avant), pattes en diagonale, puis sur le dos (pose au sol, queue
# couchee) : il pedale des quatre pattes en 4 temps, se tortille de gauche a
# droite et se frotte la tete (a l'envers, langue au vent). Retour par le meme
# chemin. Les poses sur le dos sont dessinees pattes vers le bas puis
# retournees.
COUCHE = (4, 4, 7.4, ((10, 17), (6, 18)), ((12, 17), (8, 18)),
          ((21, 17), (18, 18)), ((23, 17), (20, 18)))
FLANC = (2, 2, 7.2, ((8, 16), (3, 16)), ((9, 18), (4, 18)),
         ((26, 16), (31, 16)), ((25, 18), (30, 18)))
MI_DOS = (0, 0, 7.4, ((11, 15), (7, 17)), ((13, 16), (9, 18)),
          ((25, 15), (29, 17)), ((26, 16), (30, 18)))
PEDALE = [  # (genou, pied) : avant fond, avant proche, arriere fond, arriere proche
    (((12, 16), (10, 18)), ((15, 17), (14, 19)), ((23, 16), (22, 18)),
     ((26, 17), (27, 19))),
    (((11, 17), (12, 19)), ((14, 16), (12, 18)), ((24, 17), (25, 19)),
     ((27, 16), (25, 18))),
    (((12, 17), (13, 18)), ((14, 17), (15, 19)), ((23, 17), (24, 19)),
     ((26, 16), (28, 18))),
    (((11, 16), (9, 17)), ((15, 16), (13, 17)), ((24, 16), (23, 17)),
     ((27, 17), (28, 19))),
]
ROULADE_OX = 2
ROULADE_SOL = COURT_H - 2        # derniere ligne opaque de la grille


def _poser(g):
    """Descend la grille pour que sa ligne la plus basse touche le sol."""
    bas = max(y for y, row in enumerate(g) if any(ch != "." for ch in row))
    d = ROULADE_SOL - bas
    return [["."] * COURT_W for _ in range(d)] + g[:COURT_H - d] if d > 0 else g


def _roulade_corps(pose, queue, dos, dx=0, hdy=0):
    """Corps sans tete (retourne si `dos`), pose au sol ; queue couchee au sol
    sur le dos ; tete a l'endroit (ventre, flanc) ou a l'envers (dos)."""
    ox = ROULADE_OX + dx
    g = profil_grille(*pose, queue=queue, ox=ox, avec_tete=False,
                      avec_queue=not dos)
    g = _poser(g[::-1] if dos else g)
    if dos:
        x = int(round(19.5 + pose[2] - 4)) + ox + 3
        _g_piece(g, _g_membre([(x, ROULADE_SOL - 1),
                               (x + 3, ROULADE_SOL - 1 - queue)], [0.7, 0.7]),
                 "b", "a")
    tete = COURT_TETE[:11]
    if dos:
        tete = tete[::-1]
    for y, row in enumerate(tete):
        for x, ch in enumerate(row):
            yy, xx = ROULADE_SOL - 10 - hdy + y, ox - 1 + x
            if ch != "." and 0 <= yy < COURT_H and 0 <= xx < COURT_W:
                g[yy][xx] = ch
    return g


def anim_roulade():
    couche = _poser(profil_grille(*COUCHE, ox=ROULADE_OX))
    flanc = _roulade_corps(FLANC, 1, False)
    mi_dos = _roulade_corps(MI_DOS, 0, True)

    def sur_le_dos(k, dx, hdy):
        return _roulade_corps((0, 0, 7.4) + PEDALE[k % 4], k % 2, True,
                              dx, hdy)
    dos = [sur_le_dos(k, dx, k % 2)
           for k, dx in enumerate((-1, 0, 1, 0, -1, 0))]
    grilles = [couche, flanc, mi_dos] + dos + [mi_dos, flanc, couche]
    out = []
    for i, case in enumerate(profil_cases(grilles)[0]):
        c = Canvas()
        hearts(c, i % 6)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


# Balle : enchainee apres court. Il freine (pattes avant plantees devant,
# fesses basses), s'arrete, puis ramasse la balle posee devant lui et la
# prend dans la gueule (tete baissee puis relevee, la queue remue).
DEBOUT = (0, 0, 7.0, ((13, 15), (13, 18)), ((15, 15), (15, 18)),
          ((24, 15), (24, 18)), ((26, 15), (26, 18)))
FREINE = (-1, 1, 7.4, ((10, 15), (8, 18)), ((12, 15), (10, 18)),
          ((21, 15), (20, 18)), ((23, 15), (22, 18)))
BALLE_OX = 2                     # le chiot recule : la balle tient devant lui
BALLE = [  # (pose, decalage de tete, queue, balle : "sol" ou "gueule")
    (FREINE, (0, 0), 0, "sol"),          # il freine des quatre pattes
    (DEBOUT, (0, 0), 1, "sol"),          # arrete
    (DEBOUT, (-1, 3), 0, "sol"),
    (DEBOUT, (-1, 6), 1, "sol"),         # museau sur la balle
    (DEBOUT, (-1, 6), 0, "gueule"),      # il la saisit
    (DEBOUT, (0, 1), 1, "gueule"),       # tete relevee, balle en gueule
    (DEBOUT, (0, 0), 0, "gueule"),
    (DEBOUT, (0, 0), 1, "gueule"),
]


def anim_balle():
    grilles = [profil_grille(*pose, tete=t, queue=q, ox=BALLE_OX,
                             avec_langue=ou == "sol")
               for (pose, t, q, ou) in BALLE]
    cases, (x0, y0) = profil_cases(grilles)
    out = []
    for i, (case, (_, t, _, ou)) in enumerate(zip(cases, BALLE)):
        c = Canvas()
        if ou == "sol":
            bx, by = x0 + REPOS_ZOOM * (BALLE_OX - 1) + 4, GROUND - 4
        else:                    # au bout du museau (lignes 7-9 de la tete)
            bx = x0 + REPOS_ZOOM * (BALLE_OX + t[0]) + 3
            by = y0 + REPOS_ZOOM * (1 + t[1] + 9)
        ball(c, bx, by)
        if i == 0:               # poussiere sous les pattes avant qui freinent
            c.part(c.M.ell(x0 + REPOS_ZOOM * (BALLE_OX + 10), GROUND - 2, 3, 2),
                   DUST)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


def anim_balle_droite():
    """Meme ramassage quand la balle est partie a droite : miroir."""
    return [c.transpose(Image.FLIP_LEFT_RIGHT) for c in anim_balle()]


def anim_tournis():
    """Yeux en X, langue, le chiot tangue, trois etoiles tournent sur sa tete."""
    src = strawheart_frames()
    sway = [-2, -1, 0, 1, 2, 0]
    out = []
    for i in range(6):
        case = sh_fin(repos_yeux(strawheart_case(langue(src[0])), "vertiges"),
                      dx=sway[i])
        c = Canvas()
        cx, cy = 31 + sway[i], 6
        for k in range(3):
            a = 2 * math.pi * (i / 6 + k / 3 + 1 / 12)  # 1/12 : jamais deux alignees
            x = round(cx + 15 * math.cos(a)) - 2
            y = round(cy + 4 * math.sin(a)) - 2
            c.bitmap(STAR_B, x, y, STAR)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out


def anim_boude():
    """Assis, queue basse immobile, yeux tristes, petit nuage qui pleut."""
    queue_basse = strawheart_frames()[0]
    cloud = ["..XXX...", ".XXXXXX.", "XXXXXXXX", ".XXXXXX."]
    out = []
    for i in range(4):
        case = sh_fin(repos_yeux(strawheart_case(queue_basse), "tristes"))
        c = Canvas()
        cx, cy = 28, (i in (1, 2))
        c.bitmap(cloud, cx, cy, GREY)
        # gouttes sous le nuage, une case sur deux
        gouttes = ((30, 6), (34, 7)) if i % 2 == 0 else ((32, 7), (30, 8))
        for (x, y) in gouttes:
            c.px(x, y, LEASH)
        case.alpha_composite(to_image(c.a))
        out.append(case)
    return out



# Bain (SP5e) : le chiot assis dans une bassine bleue, une brosse en bois le
# frotte et pose de la mousse (touffes blanches) ; des bulles s'envolent ; la
# mousse disparait ensuite touffe par touffe. Trois planches jouees a la suite
# par la vue : bain (brossage), bain_mousse (en boucle quelques tours),
# bain_fin. Le fond de la bassine est sur la ligne GROUND - 1, ombre sur GROUND.
# Couleurs reprises du chiot (contour "a", bois "d"/"b", soies "f") et des
# accents (bleu laisse, bleu des z, blanc) : aucune couleur nouvelle.
BAIN_REBORD = 43                      # ligne du haut du rebord avant
BAIN_TOUFFES = [(31, 9, 3), (24, 12, 2), (39, 12, 2), (17, 22, 2),
                (46, 22, 2), (24, 37, 2), (40, 37, 2), (32, 39, 2),
                (53, 35, 2)]          # (x, y, rayon), dans l'ordre d'apparition
BAIN_BROSSE = ["...............aa",
               "..............adda",
               ".............adda.",
               "............adda..",
               "..aaaaaaaaaadda...",
               ".adddddddddddda...",
               "adbbbbbbbbbbda",
               "adddddddddddda",
               ".aaaaaaaaaaaa.",
               "..f.f.f.f.f.f.",
               "..f.f.f.f.f.f."]
# Brossage : (x, y) du bas des soies, la brosse va et vient sur la tete puis
# derriere les oreilles.
BAIN_COUPS = [(27, 9), (35, 9), (27, 9), (35, 9), (29, 8), (35, 9),
              (18, 19), (22, 19), (45, 19), (41, 19), (18, 19), (45, 19)]
# Bulles : (x, depart en pixels, rayon) ; montent de 4,5 px par case, une
# boucle de 8 cases = 36 px, donc la phase mousse se reboucle sans saut.
BAIN_BULLES = [(9, 0, 2), (21, 9, 3), (45, 18, 2), (56, 27, 3), (33, 13, 2)]


def bain_bulle(a, x, y, r):
    m = Mask()
    anneau = m.ell(x, y, r, r) & ~m.ell(x, y, r - 1, r - 1)
    a[anneau] = ACCENTS[WHITE]
    a[int(y - r + 1), int(x - 1)] = ACCENTS[ZBLUE]   # reflet


def bain_case(src, yeux, touffes, coup=None, bulles=(), pops=()):
    """Une case : chiot (src = image strawheart), bassine, mousse, bulles,
    brosse. touffes = nombre de touffes posees ; pops = touffes qui eclatent
    (petite croix blanche a leur place)."""
    a = np.array(repos_yeux(strawheart_case(src), yeux))
    m = Mask()
    contour = SH["a"]
    # Bassine : paroi avant evasee, rebord clair, ombre sous le fond.
    # le contour du fond tombe sur GROUND - 1, comme les pattes des autres poses
    paroi = m.poly([(7, BAIN_REBORD), (57, BAIN_REBORD), (53, GROUND - 2), (11, GROUND - 2)])
    rebord = m.ell(32, BAIN_REBORD, 25, 2.5)
    corps = paroi | rebord
    a[dilate4(corps) & ~corps] = contour
    a[paroi] = ACCENTS[LEASH]
    a[paroi & m.rect(0, BAIN_REBORD + 3, 63, BAIN_REBORD + 4)] = ACCENTS[ZBLUE]  # reflet
    a[rebord] = ACCENTS[ZBLUE]
    ombre = m.ell(32, GROUND, 24, 1) & (a[:, :, 3] == 0)
    a[ombre] = ACCENTS[SHADOW]
    # Mousse sur l'eau, qui deborde du rebord.
    for x in range(10, 56, 5):
        d = m.ell(x, BAIN_REBORD - 1, 3, 2)
        a[d] = ACCENTS[WHITE]
        a[d & ~shift(d, 0, -1)] = ACCENTS[ZBLUE]
    # Touffes de mousse sur le chiot.
    for (x, y, r) in BAIN_TOUFFES[:touffes]:
        d = m.ell(x, y, r, r) | m.ell(x + r, y + 1, r - 1, r - 1)
        a[d] = ACCENTS[WHITE]
        a[d & ~shift(d, 0, -1) & ~shift(d, -1, 0)] = ACCENTS[ZBLUE]
    for k in pops:
        x, y, _ = BAIN_TOUFFES[k]
        for (dx, dy) in ((0, 0), (-2, 0), (2, 0), (0, -2), (0, 2)):
            a[y + dy, x + dx] = ACCENTS[WHITE]
    for (x, y, r) in bulles:
        bain_bulle(a, x, y, r)
    if coup is not None:
        bx, by = coup
        x0, y0 = bx - 7, by - len(BAIN_BROSSE) + 1
        for dy, row in enumerate(BAIN_BROSSE):     # le manche peut sortir de la case
            for dx, ch in enumerate(row):
                if ch != "." and 0 <= x0 + dx < S and 0 <= y0 + dy < S:
                    a[y0 + dy, x0 + dx] = SH[ch]
    return Image.fromarray(a, "RGBA")


def bain_bulles(t, n=len(BAIN_BULLES)):
    out = []
    for k, (x, depart, r) in enumerate(BAIN_BULLES[:n]):
        h = (t * 4.5 + depart) % 36
        out.append((x + round(math.sin((t + k) * 1.3)), BAIN_REBORD - 4 - h, r))
    return out


def anim_bain():
    src = strawheart_frames()
    n = len(BAIN_COUPS)
    return [bain_case(src[k % 2], "rieurs", min(len(BAIN_TOUFFES), k * len(BAIN_TOUFFES) // (n - 1) + 1),
                      coup=c, bulles=bain_bulles(k, max(0, k - 5)))
            for k, c in enumerate(BAIN_COUPS)]


def anim_bain_mousse():
    src = strawheart_frames()
    return [bain_case(src[k % 2], "fermes" if k == 5 else "rieurs", len(BAIN_TOUFFES),
                      bulles=bain_bulles(k)) for k in range(8)]


def anim_bain_fin():
    src = strawheart_frames()
    restes = [8, 7, 6, 4, 3, 2, 1, 0]   # touffes encore la, case par case
    out = []
    avant = len(BAIN_TOUFFES)
    for k, r in enumerate(restes):
        pops = range(r, avant)          # celles qui viennent de partir
        out.append(bain_case(src[k % 2], "ouverts", r, bulles=bain_bulles(k, max(0, 4 - k)),
                             pops=pops))
        avant = r
    return out



# Os (SP5f) : le chiot assis tient un os blanc en travers de la gueule, sous
# la truffe, et le croque : la machoire s'ouvre d'un pixel source, l'os
# descend avec elle, des miettes tombent. Planche os (en boucle quelques
# tours), puis os_fin : l'os se casse, disparait, il se leche les babines.
OS = [".aa.......aa.",
      "awwaaaaaaawwa",
      "awwwwwwwwwwwa",
      "agwaaaaaaawga",
      ".aa.......aa."]
OS_X, OS_Y = 3, 9                     # coin haut gauche, pixels source
OS_MACHE = [0, 1, 0, 1, 1, 0, 1, 0]   # 1 = machoire ouverte (os plus bas)
OS_MIETTES = {1: [(20, 31), (44, 32)], 2: [(19, 35), (45, 36)],
              4: [(22, 31)], 5: [(21, 36)], 6: [(43, 31)], 7: [(44, 36)]}


def os_case(src, ouverte, coupe=13, miettes=(), yeux="rieurs", lechee=None):
    """coupe = colonnes de l'os encore la (13 = entier, 0 = mange) ;
    lechee = colonne source de la langue qui sort, ou None."""
    a = np.array(src)
    dy = 1 if ouverte else 0
    if ouverte:                       # gueule ouverte entre truffe et os
        for i, ch in enumerate("apppa"):   # interieur rose : la truffe reste lisible
            a[OS_Y + 1, OS_X + 4 + i] = SH[ch]
    if coupe > 0:
        rows = [r[:coupe] for r in OS]
        if coupe < 13:                # bout casse : contour sur la cassure
            rows = [r[:-1] + ("a" if r[-1] != "." else ".") for r in rows]
        sh_stamp(a, rows, OS_X, OS_Y + dy)
    if lechee is not None:
        a[OS_Y + 2, lechee] = SH["p"]
        a[OS_Y + 3, lechee] = SH["p"]
    case = np.array(repos_yeux(strawheart_case(Image.fromarray(a, "RGBA")), yeux))
    for (x, y) in miettes:
        case[y:y + 2, x:x + 2] = ACCENTS[WHITE]
    return sh_fin(Image.fromarray(case, "RGBA"))


def anim_os():
    src = strawheart_frames()
    return [os_case(src[k % 2], o, miettes=OS_MIETTES.get(k, ()),
                    yeux="fermes" if o else "rieurs")
            for k, o in enumerate(OS_MACHE)]


def anim_os_fin():
    src = strawheart_frames()
    etapes = [  # (ouverte, coupe, miettes, yeux, langue)
        (1, 13, [(20, 31), (44, 31), (32, 33)], "fermes", None),
        (0, 8, [(42, 36), (46, 33)], "rieurs", None),
        (1, 8, [(21, 33)], "fermes", None),
        (0, 4, [(30, 34), (36, 32)], "rieurs", None),
        (1, 0, [(24, 36)], "fermes", None),
        (0, 0, [], "rieurs", 9),
        (0, 0, [], "rieurs", 10),
        (0, 0, [], "rieurs", None)]
    return [os_case(src[k % 2], o, c, m, y, l) for k, (o, c, m, y, l) in enumerate(etapes)]


def os_icone():
    """Icone de l'os (SP5f), posee par la vue sur la pastille sombre."""
    motif = ["................",
             "................",
             "................",
             "................",
             "..aa........aa..",
             ".awwa......awwa.",
             ".awwwaaaaaawwwa.",
             "..awwwwwwwwwwa..",
             "..agwwwwwwwwga..",
             ".agggaaaaaagwga.",
             ".awga......agwa.",
             "..aa........aa..",
             "................",
             "................",
             "................",
             "................"]
    a = np.zeros((16, 16, 4), np.uint8)
    for y, row in enumerate(motif):
        for x, ch in enumerate(row):
            if ch != ".":
                a[y, x] = SH[ch]
    return Image.fromarray(a, "RGBA")


# Promenade (SP5h, allures SP5k) : chiot de profil qui marche vers la droite,
# collier rouge, laisse tendue vers le haut et l'arriere (le maitre suit, hors
# champ). Dessine tourne vers la gauche comme court, puis retourne. L'allure
# suit celle du maitre, mesuree par l'IMU (logic/allure.h) :
# - promenade : pas, 4 temps (arriere proche, avant proche, arriere fond,
#   avant fond, un quart de cycle d'ecart) ;
# - promenade_trot : diagonales ensemble (avant proche + arriere fond) ;
# - promenade_course : le galop de court, en laisse ;
# Pas et trot : le corps monte quand une patte de la paire avant (ou arriere)
# est en l'air, d'ou le tangage ; la patte levee se replie (poignet vers
# l'arriere a l'avant, jarret a l'arriere) et avance vite en fin de levee.
ALLURES = {
    "pas": dict(n=8, phases={"rp": 0.0, "ap": 0.25, "rf": 0.5, "af": 0.75},
                appui=0.6, pas=6.0, lev=2.4, bob=1, queue=1),
    "trot": dict(n=8, phases={"ap": 0.0, "rf": 0.0, "af": 0.5, "rp": 0.5},
                 appui=0.45, pas=6.0, lev=2.6, bob=1, queue=2),
}
# Pied a mi-appui. Pattes arriere sous la croupe (choix F de l'auteur, 2026-11-23 :
# la croupe ne doit pas depasser derriere les pattes).
MARCHE_X0 = {"af": 14.0, "ap": 12.5, "rf": 27.5, "rp": 26.0}
# Pattes arriere : appui plus long (choix H de l'auteur, 2026-11-23) : posees sous la
# hanche, elles poussent loin derriere la croupe (sinon la patte semble bloquee).
MARCHE_PAS_ARRIERE = 8.0
EPAULE = (14, 11)                                 # haut des pattes avant, source
HANCHE_X = 24.5                                   # haut des pattes arriere, source


def marche_pied(al, nom, i):
    """(genou, pied, levee 0..1) de la patte `nom` a l'image i, coordonnees source.
    D'apres les principes d'un cycle de marche de chien en pixel art : au sol la
    patte reste droite (articulation sur la ligne epaule ou hanche -> pied, jarret
    un peu en arriere) ; en l'air seulement elle se plie (poignet replie vers
    l'arriere a l'avant, jarret plie a l'arriere)."""
    p = (i / al["n"] + al["phases"][nom]) % 1.0
    avant = nom[0] == "a"
    x0, ap = MARCHE_X0[nom], al["appui"]
    A = al["pas"] if avant else MARCHE_PAS_ARRIERE
    if p < ap:                                    # au sol, recule sous le corps
        fx, fy, lev = x0 - A / 2 + A * p / ap, 18.0, 0.0
    else:                                         # levee, revient vers l'avant
        t = (p - ap) / (1 - ap)
        lev = math.sin(math.pi * t)
        fx = x0 + A / 2 - A * (t * t * (3 - 2 * t))   # lent replie, vite tendu
        fy = 18 - al["lev"] * lev
    fx, fy = min(COURT_W - 1, int(round(fx))), int(round(fy))
    haut_x = EPAULE[0] if avant else HANCHE_X
    if lev < 0.3:                                 # appui : patte droite
        gx = (haut_x + fx) / 2 + (0 if avant else 0.8)
        genou = (int(round(gx)), 15)
    elif avant:                                   # poignet replie, patte vers l'arriere
        genou = (fx - 1, 15 - int(round(lev)))
        fx += 2
    else:                                         # jarret plie, pied leve
        genou = (min(COURT_W - 1, fx + 2), 15 - int(round(lev)))
    return genou, (fx, fy), lev


def marche_grille(al, i):
    """Grille source de l'image i d'une allure, et la montee de l'avant (dya)."""
    pieds = {n: marche_pied(al, n, i) for n in al["phases"]}
    dya = -al["bob"] if max(pieds["af"][2], pieds["ap"][2]) > 0.5 else 0
    dyr = -al["bob"] if max(pieds["rf"][2], pieds["rp"][2]) > 0.5 else 0
    q = [0, 1, 2, 1][i % 4] if al["queue"] == 2 else (i // 2) % 2
    return profil_grille(dya, dyr, 8.0, pieds["af"][:2], pieds["ap"][:2],
                         pieds["rf"][:2], pieds["rp"][:2], queue=q), dya


def en_laisse(case, x0, y0, dya):
    """Case tournee vers la droite : collier au cou (source x 11..12, y 9..12,
    suivi de la montee dya de l'avant), laisse tendue de l'anneau vers le haut
    et l'arriere."""
    c = Canvas()
    M = c.M
    a = np.array(case)
    Z = REPOS_ZOOM
    cx = S - 1 - (x0 + 12 * Z) - 1
    yc = y0 + (9 + dya) * Z
    c.paint(M.rect(cx - 1, yc, cx + 2, yc + 4 * Z - 1) & (a[:, :, 3] > 0), RED)
    ax, ay = cx - 2, yc + Z
    brin = M.seg((ax, ay), (ax - 16, 0), 1) & (a[:, :, 3] == 0)                       # elle passe derriere le chiot
    c.part(brin | M.rect(ax - 1, ay, ax, ay + 1), LEASH)
    case.alpha_composite(to_image(c.a))
    return case


def _promenade(grilles, dyas):
    cases, (x0, y0) = profil_cases(grilles)
    return [en_laisse(c.transpose(Image.FLIP_LEFT_RIGHT), x0, y0, d)
            for c, d in zip(cases, dyas)]


def anim_allure(nom):
    al = ALLURES[nom]
    gs = [marche_grille(al, i) for i in range(al["n"])]
    return _promenade([gr for gr, _ in gs], [d for _, d in gs])


def anim_promenade():
    return anim_allure("pas")


def anim_promenade_trot():
    return anim_allure("trot")


def anim_promenade_course():
    return _promenade([court_grille(i) for i in range(len(COURT))],
                      [pose[0] for pose in COURT])


def laisse_icone():
    """Icone de la laisse (SP5h) : poignee en boucle, laisse en S, mousqueton."""
    motif = ["...aaaa.........",
             "..aLLLLa........",
             ".aLa..aLa.......",
             ".aLa..aLa.......",
             "..aLLLLa........",
             "...aLLa.........",
             "....aLa.........",
             ".....aLa........",
             "......aLaa......",
             ".......aLLa.....",
             "........aaLa....",
             "..........aLa...",
             "...........agga.",
             "..........agwwga",
             "..........agwwga",
             "...........agga."]
    a = np.zeros((16, 16, 4), np.uint8)
    for y, row in enumerate(motif):
        for x, ch in enumerate(row):
            if ch != ".":
                a[y, x] = SH[ch]
    return Image.fromarray(a, "RGBA")


def pile_icone():
    """Icone batterie faible (SP5h P8) : pile vide, un trait rouge."""
    motif = ["................",
             "................",
             "................",
             "................",
             "aaaaaaaaaaaaa...",
             "awwwwwwwwwwwwa..",
             "awRRa......awaa.",
             "awRRa......awwa.",
             "awRRa......awwa.",
             "awRRa......awaa.",
             "awwwwwwwwwwwwa..",
             "aaaaaaaaaaaaa...",
             "................",
             "................",
             "................",
             "................"]
    pal = dict(SH)
    pal["R"] = ACCENTS[RED]
    pal["a"] = (40, 40, 40, 255)
    a = np.zeros((16, 16, 4), np.uint8)
    for y, row in enumerate(motif):
        for x, ch in enumerate(row):
            if ch != ".":
                a[y, x] = pal[ch]
    return Image.fromarray(a, "RGBA")


ANIMS = {
    "repos": anim_repos, "dort": anim_dort, "aboie": anim_aboie,
    "laisse": anim_laisse, "patte": anim_patte, "content": anim_content,
    "roulade": anim_roulade, "court": anim_court,
    "court_droite": anim_court_droite, "balle": anim_balle,
    "balle_droite": anim_balle_droite,
    "lache": anim_lache,
    "tournis": anim_tournis, "boude": anim_boude,
    "bain": anim_bain, "bain_mousse": anim_bain_mousse, "bain_fin": anim_bain_fin,
    "os": anim_os, "os_fin": anim_os_fin,
    "promenade": anim_promenade, "promenade_trot": anim_promenade_trot,
    "promenade_course": anim_promenade_course,
}


def sheet(frames):
    img = Image.new("RGBA", (S * len(frames), S), (0, 0, 0, 0))
    for i, case in enumerate(frames):      # cases RGBA 64 x 64
        img.paste(case, (i * S, 0))
    n = len(img.getcolors(1 << 16))
    assert n <= 16, f"{n} couleurs sur une planche (16 max)"
    return img


# --- Niche (meme cadre 64 x 64 que le chiot, dessinee a gauche) ---------------
def niche():
    c = Canvas()
    M = c.M
    # bas du mur a GROUND - 2 : avec son contour, la niche se pose sur la meme
    # ligne que le chiot (derniere ligne opaque GROUND - 1)
    wall = M.rect(4, 26, 36, GROUND - 2)
    c.part(wall, SHADE, EAR)
    # pignon : le mur monte jusque sous le toit
    gable = M.poly([(4, 27), (20, 13), (36, 27)])
    c.paint(gable, SHADE)
    for y in (20, 26, 32, 38, 44, 50):    # planches
        c.paint(M.rect(5, y, 35, y) & (gable | wall), EAR)
    door = M.ell(20, 49, 8, 11) & M.rect(0, 38, 63, GROUND - 2)
    c.part(door, OUT, outline=False)
    roof = M.poly([(0, 28), (20, 9), (40, 28), (36, 30), (20, 15), (4, 30)])
    c.part(roof, RED)
    c.paint(M.poly([(20, 9), (40, 28), (38, 29), (20, 12)]) & roof, CREAM)
    # bol devant la niche
    c.part(M.rect(38, 52, 45, GROUND - 2) & M.ell(41, 51, 5, 5), LEASH)
    c.paint(M.rect(39, 52, 44, 52), CREAM)
    sl = Canvas()
    sl.paint(M.rect(3, GROUND, 47, GROUND), SHADOW)
    c.a[(c.a == T) & (sl.a == SHADOW)] = SHADOW
    return c


# --- Fonds (dessines a 80 x 95 puis agrandis x3, recadres a 240 x 284) --------
def background(night):
    W, H = 80, 95
    if night:
        pal = [(16, 20, 48), (22, 30, 66), (30, 42, 88), (40, 56, 110),
               (240, 240, 255), (255, 220, 120), (24, 52, 50), (30, 66, 58),
               (18, 40, 40), (44, 84, 66), (230, 226, 200), (60, 80, 130)]
    else:
        pal = [(110, 176, 236), (132, 192, 242), (158, 210, 246),
               (188, 226, 250), (255, 255, 255), (255, 214, 80),
               (118, 190, 88), (96, 168, 72), (76, 140, 60), (150, 206, 110),
               (255, 160, 60), (226, 238, 248)]
    a = np.zeros((H, W), np.uint8)
    horizon = 66
    # ciel en bandes, transitions tramees
    bands = [0, 18, 36, 52, horizon]
    for k in range(4):
        a[bands[k]:bands[k + 1], :] = k
        if k > 0:
            for x in range(W):
                if x % 2 == 0:
                    a[bands[k], x] = k - 1
                if (x + 1) % 2 == 0 and bands[k] - 1 >= 0:
                    a[bands[k] - 1, x] = k
    if night:
        rng = np.random.default_rng(7)
        for _ in range(26):
            x, y = int(rng.integers(0, W)), int(rng.integers(2, 56))
            a[y, x] = 4 if rng.random() < 0.6 else 5
        # lune en croissant
        moon = np.zeros((H, W), bool)
        yy, xx = np.mgrid[0:H, 0:W]
        moon |= (xx - 62) ** 2 + (yy - 22) ** 2 <= 36
        moon &= ~((xx - 65) ** 2 + (yy - 20) ** 2 <= 30)
        a[moon] = 10
    else:
        yy, xx = np.mgrid[0:H, 0:W]
        sun = (xx - 64) ** 2 + (yy - 24) ** 2 <= 30
        halo = ((xx - 64) ** 2 + (yy - 24) ** 2 <= 49) & ~sun
        a[halo & ((xx + yy) % 2 == 0)] = 11
        a[sun] = 5
        for (cx, cy) in ((14, 30), (40, 14)):
            cl = (((xx - cx) / 9.0) ** 2 + ((yy - cy) / 3.0) ** 2 <= 1)
            cl |= (((xx - cx + 3) / 5.0) ** 2 + ((yy - cy + 3) / 3.0) ** 2 <= 1)
            a[cl] = 4
            a[cl & (yy >= cy + 2)] = 11
    # collines lointaines
    yy, xx = np.mgrid[0:H, 0:W]
    hills = yy >= horizon - 6 + np.round(4 * np.sin(xx / 9.0 + 1.2)
                                         + 2 * np.sin(xx / 4.0)).astype(int)
    a[hills & (yy < horizon)] = 9 if not night else 11
    # herbe
    a[horizon:, :] = 7
    a[horizon, :] = 6
    a[horizon + 1, ::2] = 6
    a[horizon + 8:, :] = 7
    a[horizon + 16:, :] = 8
    for x in range(0, W, 2):
        a[horizon + 16, x] = 7
    rng = np.random.default_rng(3)
    for _ in range(40):
        x, y = int(rng.integers(0, W)), int(rng.integers(horizon + 2, H - 1))
        a[y, x] = 6
        a[y - 1, x] = 6
    if not night:
        for _ in range(7):
            x = int(rng.integers(2, W - 2))
            y = int(rng.integers(horizon + 3, horizon + 14))
            a[y, x] = 10
            a[y - 1, x] = 4
    lut = np.array([p + (255,) for p in pal], np.uint8)
    im = Image.fromarray(lut[a], "RGBA").resize((W * 3, H * 3), Image.NEAREST)
    return im.crop((0, 0, 240, 284))


# --- Icones 16 x 16 ----------------------------------------------------------
ICON_PAL = list(PALETTE)
ICON_PAL[GREY] = (150, 150, 162, 255)

PAW = ["...XX..XX...",
       "..XXXX.XXXX.",
       "..XXXX.XXXX.",
       "XX.XX...XX.XX",
       "XXXX.....XXXX",
       "XXXX.XXX.XXXX",
       ".XX.XXXXX.XX.",
       "...XXXXXXX...",
       "..XXXXXXXXX..",
       "..XXXXXXXXX..",
       "...XXXXXXX...",
       "....XX.XX...."]


def disc(cx, cy, r, n=16):
    yy, xx = np.mgrid[0:n, 0:n]
    return (xx - cx) ** 2 + (yy - cy) ** 2 <= r * r


def icon(kind, level):
    c = Canvas(16, 16)
    M = c.M
    yy, xx = np.mgrid[0:16, 0:16]
    if kind == "bouger":
        m = np.zeros((16, 16), bool)
        for j, row in enumerate(PAW):
            for i, ch in enumerate(row):
                if ch == "X":
                    m[2 + j, 1 + i] = True
        color, light = FUR, CREAM
    elif kind == "jouer":
        m = disc(7.5, 7.5, 6.6)
        color, light = BALL, WHITE
    else:  # dormir : croissant de lune
        m = disc(7.5, 7.5, 6.6) & ~disc(10.5, 5, 5.2)
        color, light = STAR, WHITE
    c.a[dilate4(m) & ~m] = OUT
    if level == 0:
        c.a[m] = GREY
    else:
        c.a[m] = color
        if level == 1:      # moitie basse en couleur, le haut en gris
            top = m & M.rect(0, 0, 15, 7)
            c.a[top] = GREY
    if kind == "jouer" and level > 0:
        d = np.sqrt((xx - 7.5) ** 2 + (yy + 3.5) ** 2)
        seam = m & (np.abs(d - 10.5) < 0.6)
        c.a[seam & (c.a == color)] = WHITE
    if level == 2:
        ys, xs = np.nonzero(m)
        c.px(int(xs.min()) + 2, int(ys.min()) + 1, light)
    return Image.fromarray(np.array(ICON_PAL, np.uint8)[c.a], "RGBA")


def horloge_barree():
    c = Canvas(16, 16)
    M = c.M
    face = M.ell(7.5, 7.5, 6, 6)
    c.a[dilate4(face) & ~face] = OUT
    c.a[face] = WHITE
    c.paint(M.rect(7, 3, 8, 8), OUT)       # grande aiguille
    c.paint(M.rect(8, 7, 11, 8), OUT)      # petite aiguille
    bar = M.seg((2, 13), (13, 2), 3)
    c.a[dilate4(bar) & ~bar & (c.a == T)] = OUT
    c.a[bar] = RED
    return Image.fromarray(np.array(ICON_PAL, np.uint8)[c.a], "RGBA")


def lien(barre=False):
    """Icone de liaison avec le PC (SP5d2) : symbole Bluetooth blanc, dessine
    pixel par pixel, sur une pastille bleu Bluetooth ; barree de rouge hors
    ligne, comme horloge_barree."""
    rune = ["..X..",
            "..XX.",
            "X.X.X",
            ".XXX.",
            "..X..",
            ".XXX.",
            "X.X.X",
            "..XX.",
            "..X.."]
    c = Canvas(16, 16)
    M = c.M
    pastille = M.ell(7.5, 7.5, 5.5, 7)
    c.a[dilate4(pastille) & ~pastille] = OUT
    c.a[pastille] = ZBLUE
    for y, ligne in enumerate(rune):
        for x, ch in enumerate(ligne):
            if ch == "X":
                c.a[3 + y, 5 + x] = WHITE
    if barre:
        bar = M.seg((2, 13), (13, 2), 3)
        c.a[dilate4(bar) & ~bar & (c.a == T)] = OUT
        c.a[bar] = RED
    pal = list(ICON_PAL)
    pal[ZBLUE] = (0, 98, 220, 255)  # bleu Bluetooth, propre a cette icone
    return Image.fromarray(np.array(pal, np.uint8)[c.a], "RGBA")

def volume(niveau):
    """Icone de volume (SP5e) : haut-parleur blanc dessine pixel par pixel,
    1 a 3 ondes selon le niveau ; niveau 0 = croix rouge, son coupe. Posee
    par la vue sur une pastille sombre (Theme.clock-bg)."""
    hp = ["................",
          "...........3....",
          "......X.....3...",
          ".....XX..2...3..",
          "....XXX...2..3..",
          ".XXXXXX.1..2..3.",
          ".XXXXXX..1.2..3.",
          ".XXXXXX..1.2..3.",
          ".XXXXXX..1.2..3.",
          ".XXXXXX.1..2..3.",
          "....XXX...2..3..",
          ".....XX..2...3..",
          "......X.....3...",
          "...........3....",
          "................",
          "................"]
    croix = ["X...X",
             ".X.X.",
             "..X..",
             ".X.X.",
             "X...X"]
    c = Canvas(16, 16)
    blanc = np.zeros((16, 16), bool)
    for y, ligne in enumerate(hp):
        for x, ch in enumerate(ligne):
            if ch == "X" or (ch.isdigit() and int(ch) <= niveau):
                blanc[y, x] = True
    rouge = np.zeros((16, 16), bool)
    if niveau == 0:
        for y, ligne in enumerate(croix):
            for x, ch in enumerate(ligne):
                if ch == "X":
                    rouge[5 + y, 9 + x] = True
    # sans contour : l'icone est posee sur la pastille sombre de l'horloge
    c.a[blanc] = WHITE
    c.a[rouge] = RED
    return Image.fromarray(np.array(ICON_PAL, np.uint8)[c.a], "RGBA")


def brosse_icone():
    """Icone du bain (SP5e) : la brosse du bain, avec deux bulles, posee par
    la vue sur la pastille sombre."""
    motif = ["............aa..",
             "...........adda.",
             "..........adda..",
             ".........adda...",
             "..aaaaaaadda....",
             ".addddddddda....",
             "adbbbbbbbbbda...",
             "adddddddddddda..",
             ".aaaaaaaaaaaa...",
             "..f.f.f.f.f.f...",
             "..f.f.f.f.f.f...",
             "..............ww",
             "...ww........w.w",
             "..w..w........ww",
             "..w..w..........",
             "...ww..........."]
    a = np.zeros((16, 16, 4), np.uint8)
    for y, row in enumerate(motif):
        for x, ch in enumerate(row):
            if ch != ".":
                a[y, x] = SH[ch]
    return Image.fromarray(a, "RGBA")


# --- Sons --------------------------------------------------------------------
RATE = 16000


def envelope(n, attack_s, release_s):
    env = np.ones(n)
    na, nr = int(attack_s * RATE), int(release_s * RATE)
    env[:na] = 0.5 - 0.5 * np.cos(np.linspace(0, math.pi, na))
    env[n - nr:] = 0.5 + 0.5 * np.cos(np.linspace(0, math.pi, nr))
    return env


def sweep(f0, f1, dur, vib_hz=0.0, vib_depth=0.0):
    n = int(dur * RATE)
    t = np.arange(n) / RATE
    f = f0 * (f1 / f0) ** (t / dur)               # glissando exponentiel
    if vib_hz:
        f = f + vib_depth * np.sin(2 * math.pi * vib_hz * t)
    phase = 2 * math.pi * np.cumsum(f) / RATE
    return phase, t


def bandpass_noise(n, lo, hi, seed):
    rng = np.random.default_rng(seed)
    spec = np.fft.rfft(rng.standard_normal(n))
    freqs = np.fft.rfftfreq(n, 1 / RATE)
    spec[(freqs < lo) | (freqs > hi)] = 0
    x = np.fft.irfft(spec, n)
    return x / (np.abs(x).max() + 1e-9)


def wouf(seed=1):
    dur = 0.200
    phase, t = sweep(700, 450, dur)
    # fondamentale haute + harmoniques 2 et 3 marquees : l'energie reste
    # dans 500-2000 Hz, audible sur de petits haut-parleurs
    tone = np.sin(phase) + 0.9 * np.sin(2 * phase) + 0.7 * np.sin(3 * phase)
    tone /= np.abs(tone).max()
    # attaque de bruit brillant, tres courte
    noise = bandpass_noise(len(t), 1500, 4000, seed) * np.exp(-t / 0.015)
    x = 0.8 * tone + 0.3 * noise
    # attaque franche puis decroissance, relache douce
    x *= np.exp(-t / 0.25) * envelope(len(t), 0.006, 0.050)
    return x


def silence(s):
    return np.zeros(int(s * RATE))


def sounds_v1():
    """Premiere synthese (sinus purs), gardee pour comparer."""
    out = {}
    out["wouf"] = wouf()
    out["aboie"] = np.concatenate([wouf(1), silence(0.120), wouf(2)])
    phase, t = sweep(900, 600, 0.600, vib_hz=6.0, vib_depth=30.0)
    g = np.sin(phase) + 0.2 * np.sin(2 * phase)
    out["gemit"] = g * envelope(len(t), 0.040, 0.120) * (0.8 + 0.2 * np.cos(
        2 * math.pi * t / 0.6))
    yips = []
    for k in range(3):
        phase, t = sweep(800, 500, 0.100)
        y = np.sin(phase) + 0.3 * np.sin(2 * phase) + 0.1 * np.sin(3 * phase)
        y *= envelope(len(t), 0.005, 0.040)
        yips.append(y)
        if k < 2:
            yips.append(silence(0.060))
    out["jappe"] = np.concatenate(yips)
    return out



# Synthese source-filtre (SP5e, « plus fidele ») : une source vocale (impulsions
# glottiques a bande limitee, hauteur qui tremble un peu) passe dans les
# resonances d'une petite gueule (formants, filtres passe-bande qui bougent
# avec l'ouverture), plus un souffle. Ordre de grandeur d'un chiot : hauteur
# 450-1300 Hz, formants 700-3500 Hz.
def _bruit_lisse(n, seed, coupure_hz):
    rng = np.random.default_rng(seed)
    x = rng.standard_normal(n)
    k = max(1, int(RATE / coupure_hz))
    x = np.convolve(x, np.ones(k) / k, mode="same")
    return x / (np.abs(x).max() + 1e-9)


def _courbe(points, n):
    """Courbe lineaire par morceaux : points = [(t_relatif 0..1, valeur), ...]."""
    t = np.linspace(0, 1, n)
    xs, ys = zip(*points)
    return np.interp(t, xs, ys)


def _source(f0, seed, jitter=0.02, pente=1.1):
    """Impulsions glottiques : somme d'harmoniques en 1/k^pente jusqu'a Nyquist,
    hauteur f0 (tableau par echantillon) avec un tremblement lent."""
    n = len(f0)
    f = f0 * (1 + jitter * _bruit_lisse(n, seed, 40))
    phase = 2 * math.pi * np.cumsum(f) / RATE
    x = np.zeros(n)
    for k in range(1, 40):
        actif = (k * f) < RATE / 2 * 0.9
        if not actif.any():
            break
        x += actif * np.sin(k * phase) / k ** pente
    return x


def _resonance(x, fc, q):
    """Passe-bande biquad (RBJ) dont la frequence centrale fc suit un tableau ;
    coefficients mis a jour tous les 32 echantillons."""
    y = np.zeros_like(x)
    x1 = x2 = y1 = y2 = 0.0
    for i0 in range(0, len(x), 32):
        w0 = 2 * math.pi * float(fc[i0]) / RATE
        alpha = math.sin(w0) / (2 * q)
        a0 = 1 + alpha
        b0, b2 = alpha / a0, -alpha / a0
        a1, a2 = -2 * math.cos(w0) / a0, (1 - alpha) / a0
        for i in range(i0, min(i0 + 32, len(x))):
            v = x[i]
            o = b0 * v + b2 * x2 - a1 * y1 - a2 * y2
            x2, x1 = x1, v
            y2, y1 = y1, o
            y[i] = o
    return y


def _voix(duree, f0_pts, formants, souffle, seed, env):
    """formants = [(points_frequence, q, gain), ...] ; env = enveloppe (n,)."""
    n = int(duree * RATE)
    src = _source(_courbe(f0_pts, n), seed)
    src = src / (np.abs(src).max() + 1e-9)
    src = src + souffle * _bruit_lisse(n, seed + 7, 6000)
    y = np.zeros(n)
    for pts, q, g in formants:
        y += g * _resonance(src, _courbe(pts, n), q)
    y = y / (np.abs(y).max() + 1e-9)
    return y * env


def _env_aboi(n, attaque_s, tau_s):
    t = np.arange(n) / RATE
    e = np.minimum(1, t / attaque_s) * np.exp(-np.maximum(0, t - attaque_s) / tau_s)
    return e * envelope(n, 0.002, 0.025)


def wouf2(seed=1, hauteur=1.0):
    """Aboiement de chiot : attaque franche, la hauteur monte puis retombe,
    la gueule s'ouvre (F1 monte) puis se referme."""
    d = 0.19
    n = int(d * RATE)
    env = _env_aboi(n, 0.012, 0.07)
    f0 = [(0, 520 * hauteur), (0.18, 720 * hauteur), (1, 430 * hauteur)]
    formants = [([(0, 650), (0.25, 1050), (1, 700)], 5.0, 1.0),
                ([(0, 1700), (0.25, 2100), (1, 1800)], 6.0, 0.7),
                ([(0, 3100), (1, 3000)], 7.0, 0.35)]
    x = _voix(d, f0, formants, 0.22, seed, env)
    # claquement de la gueule a l'attaque : bruit bref et brillant
    t = np.arange(n) / RATE
    x += 0.35 * bandpass_noise(n, 1200, 5000, seed + 3) * np.exp(-t / 0.008)
    return x * envelope(n, 0.002, 0.025)   # commence et finit a zero


def gemit2(seed=4):
    """Gemissement : son nasal tenu, hauteur qui monte puis redescend, vibrato
    et petite cassure de voix vers la fin."""
    d = 0.62
    n = int(d * RATE)
    t = np.arange(n) / RATE
    f0 = _courbe([(0, 820), (0.35, 1120), (0.8, 980), (1, 760)], n)
    f0 = f0 + 28 * np.sin(2 * math.pi * 6.5 * t)
    src = _source(f0, seed, jitter=0.015, pente=1.6)
    src = src / (np.abs(src).max() + 1e-9) + 0.08 * _bruit_lisse(n, seed + 1, 6000)
    y = (1.0 * _resonance(src, np.full(n, 1250.0), 7.0)
         + 0.6 * _resonance(src, np.full(n, 2600.0), 8.0)
         + 0.25 * src)
    y = y / (np.abs(y).max() + 1e-9)
    trem = 0.85 + 0.15 * np.cos(2 * math.pi * 3.0 * t)
    return y * envelope(n, 0.06, 0.16) * trem


def jappe2(seed=6):
    """Trois jappements aigus et brefs, chacun un peu different."""
    out = []
    for k, h in enumerate((1.0, 1.08, 0.95)):
        d = 0.095
        n = int(d * RATE)
        env = _env_aboi(n, 0.006, 0.035)
        f0 = [(0, 900 * h), (0.3, 1300 * h), (1, 820 * h)]
        formants = [([(0, 1100), (0.3, 1500), (1, 1200)], 5.0, 1.0),
                    ([(0, 2600), (1, 2900)], 7.0, 0.6)]
        out.append(_voix(d, f0, formants, 0.15, seed + k, env))
        if k < 2:
            out.append(silence(0.065))
    return np.concatenate(out)


def sounds():
    """Sons du chiot (SP5e) : synthese source-filtre."""
    return {"wouf": wouf2(1),
            "aboie": np.concatenate([wouf2(1), silence(0.11), wouf2(2, hauteur=0.94)]),
            "gemit": gemit2(),
            "jappe": jappe2()}


TARGET_RMS = 11000.0       # niveau commun (RMS de gemit avant correction)
PEAK_MAX = 32000


def to_pcm(x):
    x = x / np.sqrt(np.mean(x ** 2)) * TARGET_RMS
    if np.abs(x).max() > PEAK_MAX:
        # limiteur doux (tanh) ; le gain est releve jusqu'a retrouver le RMS
        # cible apres limitation (dichotomie), sans ecretage dur
        lo, hi = 1.0, 3.0
        for _ in range(40):
            g = (lo + hi) / 2
            y = PEAK_MAX * np.tanh(x * g / PEAK_MAX)
            if np.sqrt(np.mean(y ** 2)) < TARGET_RMS:
                lo = g
            else:
                hi = g
        x = PEAK_MAX * np.tanh(x * hi / PEAK_MAX)
    return np.round(x).astype("<i2")


def write_wav(path, x):
    pcm = to_pcm(x)
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())


JAPPE_RMS = 3800  # niveau commun des sons importes (importer_son.RMS_DEFAUT)


def ecrire_jappe():
    """jappe de synthese (choisi par l'auteur le 2026-11-21) : meme PCM que
    write_wav, puis ramene au RMS des vrais sons, sans limiteur."""
    pcm = to_pcm(sounds()["jappe"]).astype(float)
    pcm = np.round(pcm * JAPPE_RMS / np.sqrt(np.mean(pcm ** 2))).astype("<i2")
    with wave.open(os.path.join(SOUNDS, "jappe.wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())


def verify_sounds():
    ok = True
    for name in ("wouf", "aboie", "gemit", "jappe"):
        with wave.open(os.path.join(SOUNDS, name + ".wav")) as w:
            n, rate, ch, sw = (w.getnframes(), w.getframerate(),
                               w.getnchannels(), w.getsampwidth())
            x = np.frombuffer(w.readframes(n), "<i2").astype(float)
        spec = np.abs(np.fft.rfft(x)) ** 2
        fr = np.fft.rfftfreq(n, 1 / rate)
        hi = spec[fr > 400].sum() / spec.sum()
        rms, pk = math.sqrt(np.mean(x ** 2)), np.abs(x).max()
        good = (rate == 16000 and ch == 1 and sw == 2 and 0.1 <= n / rate <= 2.0
                and x[0] == 0 and x[-1] == 0 and pk < 32000 and hi >= 0.60)
        ok &= good
        print(f"{'OK  ' if good else 'FAIL'} {name}.wav {n * 1000 // rate} ms "
              f"RMS={rms:.0f} crete={pk:.0f} energie>400Hz={hi * 100:.1f}%")
    return ok


# --- Planche contact ----------------------------------------------------------
def contact_sheet(sheets, singles, backgrounds, mockups):
    font = ImageFont.load_default()
    Z = 4
    pad = 8
    label_h = 14
    rows = []
    for name, img in sheets.items():
        n = img.width // S
        rows.append((f"{name} ({n} cases)", img.resize(
            (img.width * Z, S * Z), Image.NEAREST), n))
    width = max(r[1].width for r in rows) + 2 * pad
    icons_w = sum(i.width * Z + pad for _, i in singles)
    bg_w = sum(b.width + pad for _, b in backgrounds + mockups)
    width = max(width, icons_w + pad, bg_w + pad)
    height = (sum(label_h + r[1].height + pad for r in rows)
              + 2 * (label_h + pad) + 16 * Z + 284 + 2 * pad)
    out = Image.new("RGBA", (width, height), (60, 60, 68, 255))
    d = ImageDraw.Draw(out)
    checker = Image.new("RGBA", (S * Z, S * Z), (200, 200, 205, 255))
    cd = ImageDraw.Draw(checker)
    for y in range(0, S * Z, 16):
        for x in range(0, S * Z, 16):
            if (x // 16 + y // 16) % 2:
                cd.rectangle([x, y, x + 15, y + 15], fill=(230, 230, 235, 255))
    y = pad
    for label, big, n in rows:
        d.text((pad, y), label, fill=(255, 255, 255, 255), font=font)
        y += label_h
        for i in range(n):
            out.paste(checker, (pad + i * S * Z, y))
            d.text((pad + i * S * Z + 3, y + 2), str(i),
                   fill=(40, 40, 40, 255), font=font)
        out.alpha_composite(big, (pad, y))
        for i in range(1, n):
            d.line([(pad + i * S * Z, y), (pad + i * S * Z, y + S * Z - 1)],
                   fill=(90, 90, 100, 255))
        y += big.height + pad
    d.text((pad, y), "icones x4 : " + "  ".join(n for n, _ in singles),
           fill=(255, 255, 255, 255), font=font)
    y += label_h
    x = pad
    for _, ic in singles:
        out.alpha_composite(ic.resize((ic.width * Z, ic.height * Z),
                                      Image.NEAREST), (x, y))
        x += ic.width * Z + pad
    y += 16 * Z + pad
    d.text((pad, y), "fonds x1 et maquettes ecran (chiot x3, 192 x 192 centre"
           " bas)", fill=(255, 255, 255, 255), font=font)
    y += label_h
    x = pad
    for _, b in backgrounds + mockups:
        out.alpha_composite(b, (x, y))
        x += b.width + pad
    return out


def mockup(bg, layers):
    m = bg.copy()
    for img in layers:
        big = img.resize((S * 3, S * 3), Image.NEAREST)
        m.alpha_composite(big, ((240 - 192) // 2, 284 - 192))
    return m


def main():
    os.makedirs(SPRITES, exist_ok=True)
    os.makedirs(SOUNDS, exist_ok=True)
    sheets = {}
    for name, fn in ANIMS.items():
        img = sheet(fn())
        img.save(os.path.join(SPRITES, name + ".png"))
        sheets[name] = img
    n_img = to_image(niche().a)
    n_img.save(os.path.join(SPRITES, "niche.png"))
    sheets["niche"] = n_img
    jour, nuit = background(False), background(True)
    jour.save(os.path.join(SPRITES, "fond_jour.png"))
    nuit.save(os.path.join(SPRITES, "fond_nuit.png"))
    singles = []
    for kind in ("bouger", "jouer", "dormir"):
        for level in range(3):
            name = f"besoin_{kind}_{level}"
            ic = icon(kind, level)
            ic.save(os.path.join(SPRITES, name + ".png"))
            singles.append((name, ic))
    hb = horloge_barree()
    hb.save(os.path.join(SPRITES, "horloge_barree.png"))
    singles.append(("horloge_barree", hb))
    for name, barre in (("lien", False), ("lien_barre", True)):
        im = lien(barre)
        im.save(os.path.join(SPRITES, name + ".png"))
        singles.append((name, im))
    oi = os_icone()
    oi.save(os.path.join(SPRITES, "os_icone.png"))
    singles.append(("os_icone", oi))
    br = brosse_icone()
    br.save(os.path.join(SPRITES, "brosse.png"))
    singles.append(("brosse", br))
    for name, fn in (("laisse_icone", laisse_icone), ("pile", pile_icone)):
        im = fn()
        im.save(os.path.join(SPRITES, name + ".png"))
        singles.append((name, im))
    for n in range(4):
        im = volume(n)
        im.save(os.path.join(SPRITES, f"volume_{n}.png"))
        singles.append((f"volume_{n}", im))
    # wouf, aboie et gemit viennent de vrais enregistrements (importer_son.py,
    # voir assets/third_party/freesound/SOURCE.md) : verifies, jamais ecrases.
    # jappe reste de synthese, ramene au niveau des vrais sons.
    ecrire_jappe()
    first = {k: v.crop((0, 0, S, S)) for k, v in sheets.items()}
    mocks = [("jour", mockup(jour, [first["repos"]])),
             ("nuit", mockup(nuit, [first["niche"], first["dort"]])),
             ("jour", mockup(jour, [sheets["content"].crop((S, 0, 2 * S, S))])),
             ("nuit", mockup(nuit, [sheets["boude"].crop((0, 0, S, S))]))]
    contact_sheet(sheets, singles, [("jour", jour), ("nuit", nuit)],
                  mocks).save(CONTACT)
    print("ok")
    ok = verify_sounds()
    print("TOUT OK" if ok else "ECHEC")
    return ok


if __name__ == "__main__":
    raise SystemExit(0 if main() else 1)
