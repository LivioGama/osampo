#!/usr/bin/env python3
"""Enregistre les démos du README avec le simulateur desktop, sans écran ni son.

Pour chaque scène : un serveur X jetable (Xvfb), un faux daemon sur le socket du widget
(trames JSON comme osampo-daemon), le simulateur `widgets/osampo/desktop/build/osampo`,
des gestes injectés (XTest), une capture ffmpeg. Sorties dans docs/media/ :
<scene>.mp4 (×2) et <scene>.webp (animé, pour le README).
Plus `allures.webp` / `allures.mp4` : pas, trot et galop depuis les planches.

Prérequis : Xvfb, ffmpeg, python3-xlib, Pillow, simulateur compilé
(`cmake -B build -S widgets/osampo/desktop && cmake --build build`).
Usage : python3 tools/demos.py [scene ...]
"""
import json
import os
import pathlib
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

RACINE = pathlib.Path(__file__).resolve().parent.parent
SIM = RACINE / "widgets/osampo/desktop/build/osampo"
SPRITES = RACINE / "widgets/osampo/ui/sprites"
MEDIA = RACINE / "docs/media"
ECRAN = ":97"
L, H = 240, 284
TZ = 3600


class FauxDaemon:
    """Socket du widget : lit le hello, envoie une trame toutes les 500 ms."""

    def __init__(self, chemin, **etat):
        self.etat = dict(sage=80, actif=1, absence_min=0, absence_finie_min=0,
                         absence_finie_age_min=-1, claude_wait=0, age=3, heure=(10, 30))
        self.etat.update(etat)
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.srv.bind(str(chemin))
        self.srv.listen(1)
        self.fin = threading.Event()
        threading.Thread(target=self._servir, daemon=True).start()

    def trame(self):
        e = dict(self.etat)
        heure = e.pop("heure")
        now = int(time.time())
        if heure:  # heure locale imposée (HH, MM)
            jour = (now + TZ) // 86400 * 86400 - TZ
            now = jour + heure[0] * 3600 + heure[1] * 60 + int(time.monotonic()) % 60
        return dict(epoch=now, tz_s=TZ, **e)

    def _servir(self):
        try:
            c, _ = self.srv.accept()
        except OSError:
            return
        with c:
            c.recv(256)  # {"hello":"osampo"}
            c.sendall(b'{"ack":"osampo","cadence_ms":500,"stale_s":30}\n')
            while not self.fin.is_set():
                try:
                    c.sendall((json.dumps(self.trame()) + "\n").encode())
                except OSError:
                    return
                time.sleep(0.5)

    def arreter(self):
        self.fin.set()
        self.srv.close()


class Ecran:
    def __init__(self):
        self.d = display.Display(ECRAN)

    def aller(self, x, y):
        xtest.fake_input(self.d, X.MotionNotify, x=int(x), y=int(y))
        self.d.sync()

    def clic(self, x, y, duree=0.08):
        self.aller(x, y)
        xtest.fake_input(self.d, X.ButtonPress, 1)
        self.d.sync()
        time.sleep(duree)
        xtest.fake_input(self.d, X.ButtonRelease, 1)
        self.d.sync()

    def glisser(self, pts, pas_s=0.02):
        self.aller(*pts[0])
        xtest.fake_input(self.d, X.ButtonPress, 1)
        self.d.sync()
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            for k in range(1, 11):
                self.aller(x0 + (x1 - x0) * k / 10, y0 + (y1 - y0) * k / 10)
                time.sleep(pas_s)
        xtest.fake_input(self.d, X.ButtonRelease, 1)
        self.d.sync()

    def focus(self):
        """Sans gestionnaire de fenetres, personne ne donne le focus clavier : on le donne."""
        for w in self.d.screen().root.query_tree().children:
            if w.get_attributes().map_state == X.IsViewable:
                w.set_input_focus(X.RevertToParent, X.CurrentTime)
        self.d.sync()

    def touche(self, c):
        self.focus()
        maj = c.isupper()
        code = self.d.keysym_to_keycode(XK.string_to_keysym(c.lower()))
        shift = self.d.keysym_to_keycode(XK.string_to_keysym("Shift_L"))
        if maj:
            xtest.fake_input(self.d, X.KeyPress, shift)
        xtest.fake_input(self.d, X.KeyPress, code)
        xtest.fake_input(self.d, X.KeyRelease, code)
        if maj:
            xtest.fake_input(self.d, X.KeyRelease, shift)
        self.d.sync()


def caresser(e):
    for _ in range(3):
        e.glisser([(70, 190), (170, 190)])
        time.sleep(0.6)


# scène : (durée s, réglages du faux daemon, actions [(t s, fonction)])
SCENES = {
    "vie": (6, {}, []),
    "caresse": (7, {}, [(1.0, caresser)]),
    "balle": (9, {}, [(1.0, lambda e: e.touche("t")), (5.0, lambda e: e.touche("T"))]),
    "bain": (9, {}, [(1.0, lambda e: e.clic(104, 22)), (2.5, caresser)]),
    "os": (8, {}, [(1.0, lambda e: e.clic(144, 22))]),
    "promenade": (7, {}, [(1.0, lambda e: e.clic(184, 22))]),
    "claude": (7, {"claude_wait": 1}, []),
    "nuit": (6, {"heure": (23, 30), "actif": 0}, []),
}


def enregistrer(nom, duree, reglages, actions, tmp):
    run, etat = tmp / "run", tmp / "state"
    for d in (run, etat):
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(mode=0o700)
    faux_bin = tmp / "bin"
    faux_bin.mkdir(exist_ok=True)
    (faux_bin / "paplay").write_text("#!/bin/sh\nexit 0\n")  # pas de son pendant l'enregistrement
    (faux_bin / "paplay").chmod(0o755)
    env = dict(os.environ, DISPLAY=ECRAN, XDG_RUNTIME_DIR=str(run), XDG_STATE_HOME=str(etat),
               PATH=f"{faux_bin}:{os.environ['PATH']}")
    daemon = FauxDaemon(run / "osampo.sock", **reglages)
    sim = subprocess.Popen([str(SIM)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        time.sleep(2.5)  # fenêtre affichée, lien établi
        brut = tmp / f"{nom}.mkv"
        cap = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab", "-draw_mouse", "0",
                                "-framerate", "25", "-video_size", f"{L}x{H}", "-i", ECRAN,
                                "-t", str(duree), "-c:v", "ffv1", str(brut)])
        e = Ecran()
        t0 = time.monotonic()
        for t, f in actions:
            time.sleep(max(0, t - (time.monotonic() - t0)))
            f(e)
        cap.wait()
    finally:
        sim.terminate()
        sim.wait()
        daemon.arreter()
    sortir(brut, MEDIA / nom)


def sortir(brut, base):
    x2 = "scale=iw*2:ih*2:flags=neighbor"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(brut), "-vf", x2,
                    "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "20", f"{base}.mp4"], check=True)
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(brut), "-vf", f"fps=15,{x2}",
                    "-c:v", "libwebp_anim", "-lossless", "0", "-q:v", "70", "-loop", "0", f"{base}.webp"],
                   check=True)


def allures(tmp):
    """Pas, trot et galop côte à côte, depuis les planches, sur le fond de jour."""
    from PIL import Image
    fond = Image.open(SPRITES / "fond_jour.png").convert("RGBA")
    planches = [("promenade", 130), ("promenade_trot", 110), ("promenade_course", 120)]
    cases = []
    for nom, ms in planches:
        im = Image.open(SPRITES / f"{nom}.png").convert("RGBA")
        cases.append(([im.crop((k * 64, 0, k * 64 + 64, 64)) for k in range(im.width // 64)], ms))
    images = tmp / "allures"
    images.mkdir()
    for k in range(150):  # 6 s à 25 i/s
        t = k * 40
        out = Image.new("RGBA", (3 * L, H))
        for j, (cs, ms) in enumerate(cases):
            p = fond.copy()
            p.alpha_composite(cs[(t // ms) % len(cs)].resize((192, 192), Image.NEAREST), (24, 92))
            out.alpha_composite(p, (j * L, 0))
        out.convert("RGB").save(images / f"{k:04d}.png")
    brut = tmp / "allures.mkv"
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-framerate", "25", "-i", str(images / "%04d.png"),
                    "-c:v", "ffv1", str(brut)], check=True)
    sortir(brut, MEDIA / "allures")


def main():
    choix = sys.argv[1:] or list(SCENES) + ["allures"]
    if not SIM.exists():
        sys.exit(f"simulateur absent : {SIM}")
    MEDIA.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as t:
        tmp = pathlib.Path(t)
        if "allures" in choix:
            allures(tmp)
            print("allures")
        scenes = [s for s in choix if s in SCENES]
        if not scenes:
            return
        xvfb = subprocess.Popen(["Xvfb", ECRAN, "-screen", "0", f"{L}x{H}x24", "-nolisten", "tcp"],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            time.sleep(1)
            for nom in scenes:
                duree, reglages, actions = SCENES[nom]
                enregistrer(nom, duree, reglages, actions, tmp)
                print(nom)
        finally:
            xvfb.terminate()
            xvfb.wait()


if __name__ == "__main__":
    main()
