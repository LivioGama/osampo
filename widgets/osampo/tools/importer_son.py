"""Importe un enregistrement reel comme son du chiot (SP5g, voir assets/third_party/freesound/SOURCE.md).

usage : python3 importer_son.py SOURCE NOM [--debut S] [--duree S] [--passe-haut HZ] [--crete N]
                                [--grave DEMI_TONS] [--eq-board]

SOURCE : tout format lu par ffmpeg (mp3, wav, flac, ogg...). Le passage
[debut, debut + duree] est converti en 16 kHz mono 16 bits, debarrasse des
graves que le petit haut-parleur ne rend pas (passe-haut), ramene a un niveau
commun sans limiteur (--rms, ou --crete pour un passage avec des silences), avec des
fondus de 5 ms aux bords, puis ecrit dans sounds/NOM.wav et verifie avec les
memes regles que les sons de synthese.
"""
import argparse
import os
import subprocess
import sys

import wave

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_assets as g  # noqa: E402

RMS_DEFAUT = 3800.0  # le plus haut niveau ou aucun des quatre sons ne depasse CRETE_MAX
CRETE_MAX = 30000.0


def decoder(source, debut, duree, passe_haut):
    cmd = ["ffmpeg", "-nostdin", "-v", "error", "-ss", str(debut), "-i", source]
    if duree:
        cmd += ["-t", str(duree)]
    if passe_haut > 0:  # 0 : pas de filtre
        cmd += ["-af", f"highpass=f={passe_haut}:poles=2,highpass=f={passe_haut}:poles=2"]
    cmd += ["-ac", "1", "-ar", str(g.RATE), "-f", "s16le", "-"]
    brut = subprocess.run(cmd, check=True, capture_output=True).stdout
    return np.frombuffer(brut, "<i2").astype(float)


def biquad(x, b, a):
    """Filtre RBJ du second ordre, forme directe I."""
    b = [v / a[0] for v in b]
    a = [v / a[0] for v in a]
    y = np.zeros_like(x)
    x1 = x2 = y1 = y2 = 0.0
    for i, v in enumerate(x):
        o = b[0] * v + b[1] * x1 + b[2] * x2 - a[1] * y1 - a[2] * y2
        x2, x1, y2, y1 = x1, v, y1, o
        y[i] = o
    return y


def eq_board(x):
    """Egaliseur du haut-parleur de la 1.83" (SP5j, a l'ecoute de l'auteur) : coupe
    sous 300 Hz (il ne les rend pas), +6 dB vers 1 kHz (le plus grave qu'il
    rend), aigus durs -5 dB au-dessus de 4 kHz."""
    import math
    r = g.RATE
    w = 2 * math.pi * 300 / r
    al, c = math.sin(w) / (2 * 0.707), math.cos(w)
    x = biquad(x, [(1 + c) / 2, -(1 + c), (1 + c) / 2], [1 + al, -2 * c, 1 - al])
    A, w = 10 ** (6 / 40), 2 * math.pi * 1000 / r
    al, c = math.sin(w) / (2 * 0.8), math.cos(w)
    x = biquad(x, [1 + al * A, -2 * c, 1 - al * A], [1 + al / A, -2 * c, 1 - al / A])
    A, w = 10 ** (-5 / 40), 2 * math.pi * 4000 / r
    c, s = math.cos(w), 2 * math.sqrt(A) * math.sin(w) / 2 * math.sqrt(2)
    return biquad(x, [A * ((A + 1) + (A - 1) * c + s), -2 * A * ((A - 1) + (A + 1) * c),
                      A * ((A + 1) + (A - 1) * c - s)],
                  [(A + 1) - (A - 1) * c + s, 2 * ((A - 1) - (A + 1) * c), (A + 1) - (A - 1) * c - s])


def grave(x, demi_tons):
    """Baisse de demi_tons (negatif = plus grave) en ralentissant : le son
    s'allonge d'autant (chien plus gros)."""
    f = 2 ** (-demi_tons / 12)
    return np.interp(np.arange(int(len(x) * f)) / f, np.arange(len(x)), x)


def porte(x, seuil_db):
    """Porte de bruit douce : niveau par tranche de 10 ms, gain 1 au-dessus
    du seuil, 0,1 en dessous, lisse sur 20 ms pour ne pas claquer."""
    w = g.RATE // 100
    n = len(x) // w
    niv = np.array([np.sqrt(np.mean(x[i * w:(i + 1) * w] ** 2)) for i in range(n)])
    seuil = niv.max() * 10 ** (-abs(seuil_db) / 20)
    gain = np.where(niv >= seuil, 1.0, 0.1)
    gain = np.convolve(np.repeat(gain, w), np.ones(2 * w) / (2 * w), mode="same")
    out = x.copy()
    out[:len(gain)] *= gain
    out[len(gain):] *= gain[-1] if len(gain) else 1.0
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("source")
    p.add_argument("nom")
    p.add_argument("--debut", type=float, default=0.0)
    p.add_argument("--duree", type=float, default=0.0)
    p.add_argument("--passe-haut", type=float, default=350.0)
    p.add_argument("--porte", type=float, default=0.0,
                   help="seuil en dB sous la crete : le souffle plus faible est attenue de 20 dB")
    p.add_argument("--rms", type=float, default=RMS_DEFAUT,
                   help="niveau commun ; les vrais sons ont une crete 3 a 8 fois le RMS")
    p.add_argument("--crete", type=float, default=0.0,
                   help="ramene la crete a cette valeur au lieu du RMS (passage avec des silences)")
    p.add_argument("--grave", type=float, default=0.0,
                   help="hauteur en demi-tons (negatif = plus grave, et plus long)")
    p.add_argument("--eq-board", action="store_true",
                   help="egaliseur du haut-parleur de la 1.83\"")
    p.add_argument("--sortie", default=g.SOUNDS)
    a = p.parse_args()
    x = decoder(a.source, a.debut, a.duree, a.passe_haut)
    if len(x) < g.RATE // 10:
        sys.exit("passage trop court (< 100 ms)")
    if a.porte:
        x = porte(x, a.porte)
    if a.grave:
        x = grave(x, a.grave)
    if a.eq_board:
        x = eq_board(x)
    x = x * g.envelope(len(x), 0.005, 0.005)
    x[0] = x[-1] = 0.0
    os.makedirs(a.sortie, exist_ok=True)
    chemin = os.path.join(a.sortie, a.nom + ".wav")
    # Niveau commun sans limiteur (le limiteur de gen_assets ecrasait les
    # pointes des vrais aboiements de 7 a 9 dB : son sature) ; une crete
    # trop haute fait baisser le niveau plutot que de raboter.
    if a.crete:
        x = x / np.abs(x).max() * min(a.crete, CRETE_MAX)
    else:
        x = x / np.sqrt(np.mean(x ** 2)) * a.rms
    if np.abs(x).max() > CRETE_MAX:
        x *= CRETE_MAX / np.abs(x).max()
    with wave.open(chemin, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(g.RATE)
        w.writeframes(np.round(x).astype("<i2").tobytes())
    print("ecrit", chemin, f"{len(x) / g.RATE:.2f} s")


if __name__ == "__main__":
    main()
