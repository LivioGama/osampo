"""Outils communs des tests d'acceptation SP6 (chiot sur le bureau du PC).

Les tests pilotent les binaires par leurs entrees : socket du daemon, faux BLE
(`--faux-ble`), fichier TOML, D-Bus de session simule, serveur X (Xvfb).
"""
import datetime
import json
import os
import pathlib
import shutil
import socket
import subprocess
import tempfile
import threading
import time

RACINE = pathlib.Path(__file__).resolve().parents[2]


# ---------------------------------------------------------------- generalites
def attendre(pred, timeout=5.0, pas=0.05, msg="condition"):
    """Rend la premiere valeur vraie de pred(), sinon echoue apres timeout s."""
    fin = time.monotonic() + timeout
    while True:
        v = pred()
        if v:
            return v
        if time.monotonic() > fin:
            raise AssertionError(f"delai de {timeout} s depasse : {msg}")
        time.sleep(pas)


def maintenant():
    """Heure locale, en evitant de franchir une minute pendant le test."""
    d = datetime.datetime.now()
    if d.second >= 56:
        time.sleep(61 - d.second)
        d = datetime.datetime.now()
    return d


def hhmm(d):
    return d.strftime("%H:%M")


def id_rappel(categorie, heure):
    return f"{categorie},{heure.replace(':', '')}"


JOURS = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"]
JOURS_COURTS = ["lun", "mar", "mer", "jeu", "ven", "sam", "dim"]


def config_toml(rappels=(), calme=("00:00", "00:00"), report_min=10,
                rattrapage_min=60, extra=""):
    """Un osampo.toml valide. calme debut = fin : aucune heure calme."""
    if not rappels:
        # evite que les rappels par defaut du modele (12:00, 12:30...) sonnent
        rappels = [("repas", "03:17", None)]
    t = (f"[rappels]\nreport_min = {report_min}\nrattrapage_min = {rattrapage_min}\n\n"
         f"[heures]\ncalme_debut = \"{calme[0]}\"\ncalme_fin = \"{calme[1]}\"\n\n{extra}\n")
    for cat, heure, jours in rappels:
        t += f"[[rappel]]\ncategorie = \"{cat}\"\nheure = \"{heure}\"\n"
        if jours:
            t += "jours = [" + ", ".join(f'"{j}"' for j in jours) + "]\n"
        t += "\n"
    return t


def ligne_etat(etat="repos", raison="-", rappel="-", promenade_debut="-", pile=83,
               besoins=None, pose=None):
    """Ligne d'etat de la board. pose=None : champ absent (board pas encore reflashee, SPEC-012)."""
    o = {"etat": etat, "raison": raison, "rappel": rappel,
         "promenade_debut": promenade_debut, "pile": pile,
         "besoins": besoins or {"bouger": 70, "jouer": 70, "dormir": 70}}
    if pose is not None:
        o["pose"] = pose
    return o


# ------------------------------------------------------------ clients socket
class Client:
    """Client de socket Unix, lignes JSON, lecture en tache de fond."""

    def __init__(self, chemin, timeout=15):
        self._lock = threading.Lock()
        self._lignes = []
        self._brut = b""
        self.ferme = False
        self.sock = None
        fin = time.monotonic() + timeout
        while True:
            try:
                s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                s.connect(str(chemin))
                self.sock = s
                break
            except OSError:
                s.close()
                if time.monotonic() > fin:
                    raise AssertionError(f"socket {chemin} injoignable")
                time.sleep(0.1)
        threading.Thread(target=self._lire, daemon=True).start()

    def _lire(self):
        while True:
            try:
                d = self.sock.recv(65536)
            except OSError:
                d = b""
            if not d:
                self.ferme = True
                return
            self._brut += d
            while b"\n" in self._brut:
                l, self._brut = self._brut.split(b"\n", 1)
                try:
                    o = json.loads(l)
                except ValueError:
                    continue
                with self._lock:
                    self._lignes.append(o)

    def envoyer(self, obj):
        self.sock.sendall((json.dumps(obj) + "\n").encode())

    def recus(self):
        with self._lock:
            return list(self._lignes)

    def fermer(self):
        try:
            self.sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        self.sock.close()


class ClientPc(Client):
    """Client du socket du daemon : voie `pc` de SPEC-002."""

    def __init__(self, chemin, **kw):
        super().__init__(chemin, **kw)
        self.envoyer({"hello": "osampo-pc"})

    def etat(self):
        for o in reversed(self.recus()):
            if "pc" in o:
                return o["pc"]
        return None

    def attendre_etat(self, pred, timeout=5.0, msg="etat"):
        def f():
            e = self.etat()
            return e if e is not None and pred(e) else None
        return attendre(f, timeout, msg=msg + f" ; dernier etat : {self.etat()}")

    def commande(self, cmd, timeout=5.0, **kw):
        n = len([o for o in self.recus() if "reponse" in o])
        self.envoyer({"cmd": cmd, **kw})

        def f():
            r = [o for o in self.recus() if "reponse" in o]
            return r[n] if len(r) > n else None
        return attendre(f, timeout, msg=f"reponse a {cmd}")


class FausseBoard(Client):
    """La board simulee : client du socket `--faux-ble` du daemon."""

    def etat(self, **kw):
        self.envoyer(ligne_etat(**kw))

    def evenement(self, evt, **kw):
        self.envoyer({"evt": evt, **kw})

    def commandes(self):
        return [o for o in self.recus() if "cmd" in o]

    def attendre_commande(self, pred, timeout=5.0, msg="commande"):
        return attendre(lambda: next((c for c in self.commandes() if pred(c)), None),
                        timeout, msg=msg + f" ; recu : {self.commandes()}")


# ------------------------------------------------------------------ binaires
def _construire():
    """Compile tout, rend {nom de binaire: chemin}.

    Les specs ne fixent pas l'organisation Cargo : le daemon peut etre membre du
    workspace racine ou en etre exclu. On compile donc le workspace racine (s'il
    existe) ET le manifeste du daemon, et on fusionne les binaires produits.
    """
    base = ["cargo", "build", "--message-format=json"]
    manif_daemon = RACINE / "daemon" / "Cargo.toml"
    cmds = []
    if (RACINE / "Cargo.toml").exists():
        cmds.append(base + ["--workspace"])
    cmds.append(base + ["--manifest-path", str(manif_daemon)])
    bins = {}
    for cmd in cmds:
        r = subprocess.run(cmd, cwd=RACINE, capture_output=True, text=True)
        if r.returncode != 0:
            raise AssertionError(f"{' '.join(cmd)} a echoue :\n" + r.stderr[-3000:])
        for l in r.stdout.splitlines():
            try:
                o = json.loads(l)
            except ValueError:
                continue
            if o.get("reason") == "compiler-artifact" and o.get("executable") \
                    and "bin" in o["target"]["kind"]:
                bins[o["target"]["name"]] = o["executable"]
                if pathlib.Path(o["manifest_path"]).resolve() == manif_daemon.resolve():
                    bins.setdefault("__daemon__", o["executable"])
    return bins


# ------------------------------------------------------------------- daemon
class Monde:
    """Un daemon (+ faux BLE) et, selon les besoins, osampo-pc avec D-Bus et X."""

    def __init__(self, bins, config=None, watcher=True, notif=True, pc=False,
                 board=True, x=None):
        self.bins = bins
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="tg"))
        self.procs = []
        self.clients = []
        self.cfg_dir = self.tmp / "config" / "osampo"
        self.cfg_dir.mkdir(parents=True)
        self.cfg = self.cfg_dir / "osampo.toml"
        self.cfg.write_text(config if config is not None else config_toml())
        self.run = self.tmp / "run"
        self.run.mkdir()
        self.sock = self.tmp / "d.sock"
        self.ble = self.tmp / "b.sock"
        self.env = {**os.environ, "XDG_CONFIG_HOME": str(self.tmp / "config"),
                    "XDG_STATE_HOME": str(self.tmp / "state"),
                    "XDG_RUNTIME_DIR": str(self.run), "HOME": str(self.tmp / "home")}
        (self.tmp / "home").mkdir()
        self.bus = self.faux_watcher = self.faux_notif = None
        self.x = x
        if watcher or notif or pc:
            import dbus_faux
            self.bus = dbus_faux.Bus()
            self.env["DBUS_SESSION_BUS_ADDRESS"] = self.bus.adresse
            if notif:
                self.faux_notif = dbus_faux.FauxNotifications(self.bus.conn)
            if watcher:
                self.faux_watcher = dbus_faux.FauxWatcher(self.bus.conn)
        self.demarrer_daemon()
        self.board = self.connecter_board() if board else None
        self.client = ClientPc(self.sock)
        self.clients.append(self.client)
        self.pc = None
        if pc is True:   # pc="manuel" : tout est pret, le test lance osampo-pc lui-meme
            self.demarrer_pc()

    # --- daemon
    def demarrer_daemon(self):
        self.daemon_err = open(self.tmp / "daemon.err", "wb")
        self.daemon = subprocess.Popen(
            [self.bins["__daemon__"], "--socket", str(self.sock), "--faux-ble",
             str(self.ble), "--widgets-dir", str(RACINE / "widgets")],
            env=self.env, cwd=RACINE, stderr=self.daemon_err, stdout=subprocess.DEVNULL)
        self.procs.append(self.daemon)
        attendre(lambda: self.sock.exists() and self.ble.exists(), 20,
                 msg="sockets du daemon (--socket, --faux-ble)")

    def arreter_daemon(self):
        self.daemon.terminate()
        self.daemon.wait(10)

    def stderr_daemon(self):
        self.daemon_err.flush()
        return (self.tmp / "daemon.err").read_text(errors="replace")

    def connecter_board(self):
        b = FausseBoard(self.ble)
        self.clients.append(b)
        self.board = b
        return b

    def couper_board(self):
        self.board.fermer()
        self.board = None

    def config(self, texte):
        """Reecrit osampo.toml (le daemon relit toutes les 2 s)."""
        tmp = self.cfg.with_suffix(".tmp")
        tmp.write_text(texte)
        os.replace(tmp, self.cfg)

    # --- programme du PC
    def demarrer_pc(self, binaire=None, env_extra=None):
        env = {**self.env, **(env_extra or {})}
        env["DISPLAY"] = self.x.display if self.x else ""
        self.pc_err = open(self.tmp / "pc.err", "wb")
        self.pc = subprocess.Popen([binaire or self.bins["osampo-pc"], "--socket",
                                    str(self.sock)], env=env, cwd=RACINE,
                                   stderr=self.pc_err, stdout=subprocess.DEVNULL)
        self.procs.append(self.pc)
        return self.pc

    def stderr_pc(self):
        self.pc_err.flush()
        return (self.tmp / "pc.err").read_text(errors="replace")

    def item(self, timeout=20):
        import dbus_faux
        w = self.faux_watcher
        attendre(lambda: w.items, timeout,
                 msg="inscription de l'icone aupres du StatusNotifierWatcher")
        return dbus_faux.Item(self.bus.conn, *w.items[-1])

    def fermer(self):
        for c in self.clients:
            try:
                c.fermer()
            except OSError:
                pass
        for p in reversed(self.procs):
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(5)
                except subprocess.TimeoutExpired:
                    p.kill()
        if self.bus:
            self.bus.fermer()
        shutil.rmtree(self.tmp, ignore_errors=True)


# --------------------------------------------------------------- programme CLI
def reglages(bins, config, *args, timeout=30):
    return subprocess.run([bins["osampo-reglages"], "--config", str(config), *args],
                          capture_output=True, text=True, timeout=timeout, cwd=RACINE)
