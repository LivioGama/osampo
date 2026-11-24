"""Serveur X virtuel (Xvfb) et lecture des fenetres de osampo-pc."""
import os
import subprocess
import time

from Xlib import X, display as xdisplay, Xatom

LARGEUR, HAUTEUR = 800, 600
ZONE = (0, 0, LARGEUR, HAUTEUR - 40)   # _NET_WORKAREA simulee : un tableau de bord de 40 px


class Xvfb:
    """Serveur X virtuel : aucune fenetre n'apparait sur l'ecran de l'utilisateur."""

    def __init__(self):
        r, w = os.pipe()
        try:
            self.proc = subprocess.Popen(
                ["Xvfb", "-displayfd", str(w), "-screen", "0", f"{LARGEUR}x{HAUTEUR}x24",
                 "-ac", "-noreset", "-nolisten", "tcp"],
                stderr=subprocess.DEVNULL, pass_fds=(w,))
        except FileNotFoundError:
            raise AssertionError("env : Xvfb introuvable")
        finally:
            os.close(w)
        # Xvfb ecrit le numero d'ecran libre qu'il a pris, suivi d'un retour a la ligne
        lu = b""
        with os.fdopen(r, "rb") as f:
            fin = time.monotonic() + 10
            while not lu.endswith(b"\n") and time.monotonic() < fin:
                morceau = f.read1(16) if hasattr(f, "read1") else f.read(1)
                if not morceau:
                    break
                lu += morceau
        if not lu.strip().isdigit():
            self.proc.kill()
            raise AssertionError("env : Xvfb ne demarre pas")
        self.display = f":{int(lu)}"
        for _ in range(100):
            try:
                self.d = xdisplay.Display(self.display)
                break
            except Exception:
                time.sleep(0.1)
        else:
            raise AssertionError("env : Xvfb ne repond pas")
        self.root = self.d.screen().root
        a = self.d.intern_atom
        self.root.change_property(a("_NET_WORKAREA"), Xatom.CARDINAL, 32, list(ZONE))
        self.fond = self.root.create_window(0, 0, 10, 10, 0, self.d.screen().root_depth)
        self.root.change_property(a("_NET_ACTIVE_WINDOW"), Xatom.WINDOW, 32,
                                  [self.fond.id])
        self.d.sync()

    def fermer(self):
        self.proc.terminate()
        self.proc.wait(5)

    # --- lecture
    def active(self):
        p = self.root.get_full_property(self.d.intern_atom("_NET_ACTIVE_WINDOW"),
                                        Xatom.WINDOW)
        return list(p.value) if p else None

    def _parcours(self, w):
        try:
            for e in w.query_tree().children:
                yield e
                yield from self._parcours(e)
        except Exception:
            return

    def fenetres(self, classe=("osampo-pc", "Osampo")):
        """Fenetres dont WM_CLASS est `classe` (le chien par defaut)."""
        res = []
        for w in self._parcours(self.root):
            try:
                c = w.get_wm_class()
            except Exception:
                continue
            if c and tuple(c) == tuple(classe):
                res.append(w)
        return res

    def visibles(self, classe=("osampo-pc", "Osampo")):
        out = []
        for w in self.fenetres(classe):
            try:
                if w.get_attributes().map_state == X.IsViewable:
                    out.append(w)
            except Exception:
                pass
        return out

    def geometrie(self, w):
        g = w.get_geometry()
        p = self.root.translate_coords(w, 0, 0)
        return {"x": p.x, "y": p.y, "w": g.width, "h": g.height}

    def propriete(self, w, nom, type_=X.AnyPropertyType):
        p = w.get_full_property(self.d.intern_atom(nom), type_)
        return list(p.value) if p else None

    def etat_wm(self, w):
        v = self.propriete(w, "_NET_WM_STATE") or []
        return {self.d.get_atom_name(a) for a in v}

    def texte(self, w, nom):
        """Propriete UTF8_STRING (ou STRING) de la fenetre w, None si absente."""
        try:
            p = w.get_full_property(self.d.intern_atom(nom), X.AnyPropertyType)
        except Exception:
            return None
        if not p:
            return None
        v = p.value
        return v.decode("utf-8", "replace") if isinstance(v, (bytes, bytearray)) else str(v)

    def anim(self, w):
        """Valeur de _OSAMPO_ANIM (SPEC-011), None si absente."""
        return self.texte(w, "_OSAMPO_ANIM")

    def menus(self):
        """Fenetres visibles du menu du chien (WM_CLASS osampo-pc / OsampoMenu)."""
        return self.visibles(("osampo-pc", "OsampoMenu"))

    def entrees_menu(self, w):
        """Entrees de _OSAMPO_MENU : dicts y, h, actif, texte (SPEC-011)."""
        t = self.texte(w, "_OSAMPO_MENU")
        out = []
        for l in (t or "").split("\n"):
            if not l.strip():
                continue
            y, h, actif, texte = l.split("\t", 3)
            out.append({"y": int(y), "h": int(h), "actif": actif == "1", "texte": texte})
        return out

    def workarea(self, zone):
        """Change _NET_WORKAREA (x, y, largeur, hauteur) ; a retablir avec ZONE en fin de test."""
        self.root.change_property(self.d.intern_atom("_NET_WORKAREA"), Xatom.CARDINAL, 32,
                                  list(zone))
        self.d.sync()

    # --- pointeur (XTest)
    def _fake(self, type_, detail=0, x=0, y=0):
        from Xlib.ext import xtest
        xtest.fake_input(self.d, type_, detail, x=x, y=y)
        self.d.sync()

    def aller(self, x, y):
        self._fake(X.MotionNotify, 0, x, y)

    def clic(self, x, y, bouton=1):
        self.aller(x, y)
        time.sleep(0.05)
        self._fake(X.ButtonPress, bouton)
        time.sleep(0.05)
        self._fake(X.ButtonRelease, bouton)
        time.sleep(0.05)

    def glisser(self, x0, y0, x1, y1, pas=12):
        """Bouton gauche enfonce en (x0, y0), deplace par etapes jusqu'a (x1, y1), relache."""
        self.aller(x0, y0)
        time.sleep(0.1)
        self._fake(X.ButtonPress, 1)
        time.sleep(0.1)
        for i in range(1, pas + 1):
            self.aller(x0 + (x1 - x0) * i // pas, y0 + (y1 - y0) * i // pas)
            time.sleep(0.03)
        time.sleep(0.1)
        self._fake(X.ButtonRelease, 1)
        time.sleep(0.1)

    def centre(self, w):
        g = self.geometrie(w)
        return g["x"] + g["w"] // 2, g["y"] + g["h"] // 2
