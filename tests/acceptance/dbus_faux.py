"""Bus de session prive avec un faux StatusNotifierWatcher et un faux serveur de notifications."""
import re
import subprocess
import threading

import dbus
import dbus.mainloop.glib
import dbus.service
from gi.repository import GLib

dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
_boucle = None


def _demarrer_boucle():
    global _boucle
    if _boucle is None:
        _boucle = GLib.MainLoop()
        threading.Thread(target=_boucle.run, daemon=True).start()


class Bus:
    def __init__(self):
        _demarrer_boucle()
        self.proc = subprocess.Popen(
            ["dbus-daemon", "--session", "--nofork", "--print-address=1"],
            stdout=subprocess.PIPE, text=True)
        self.adresse = self.proc.stdout.readline().strip()
        self.conn = dbus.bus.BusConnection(self.adresse)

    def fermer(self):
        self.conn.close()
        self.proc.terminate()
        self.proc.wait(5)


class FauxWatcher(dbus.service.Object):
    """org.kde.StatusNotifierWatcher (fourni en vrai par xapp-sn-watcher)."""
    IFACE = "org.kde.StatusNotifierWatcher"

    def __init__(self, conn):
        self.items = []  # (service, chemin)
        nom = dbus.service.BusName(self.IFACE, conn)
        super().__init__(conn, "/StatusNotifierWatcher", nom)
        self._nom = nom

    @dbus.service.method(IFACE, in_signature="s", sender_keyword="sender")
    def RegisterStatusNotifierItem(self, service, sender=None):
        if str(service).startswith("/"):
            self.items.append((sender, str(service)))
        elif "/" in str(service):
            s, _, p = str(service).partition("/")
            self.items.append((s, "/" + p))
        else:
            self.items.append((str(service), "/StatusNotifierItem"))

    @dbus.service.method(IFACE, in_signature="s")
    def RegisterStatusNotifierHost(self, service):
        pass

    _PROPS = {"IsStatusNotifierHostRegistered": dbus.Boolean(True),
              "ProtocolVersion": dbus.Int32(0)}

    @dbus.service.method("org.freedesktop.DBus.Properties", in_signature="ss",
                         out_signature="v")
    def Get(self, iface, prop):
        if prop == "RegisteredStatusNotifierItems":
            return dbus.Array([s for s, _ in self.items], signature="s")
        return self._PROPS[prop]

    @dbus.service.method("org.freedesktop.DBus.Properties", in_signature="s",
                         out_signature="a{sv}")
    def GetAll(self, iface):
        d = dict(self._PROPS)
        d["RegisteredStatusNotifierItems"] = dbus.Array([s for s, _ in self.items],
                                                        signature="s")
        return d

    @dbus.service.signal(IFACE, signature="s")
    def StatusNotifierItemRegistered(self, s):
        pass

    def liberer(self):
        self.remove_from_connection()
        self._nom = None


class FauxNotifications(dbus.service.Object):
    """org.freedesktop.Notifications : enregistre les appels, emet les actions."""
    IFACE = "org.freedesktop.Notifications"

    def __init__(self, conn):
        self.recues = []    # dicts des appels a Notify
        self.fermees = []   # ids passes a CloseNotification
        self._n = 0
        nom = dbus.service.BusName(self.IFACE, conn)
        super().__init__(conn, "/org/freedesktop/Notifications", nom)
        self._nom = nom

    @dbus.service.method(IFACE, out_signature="as")
    def GetCapabilities(self):
        return ["actions", "body", "persistence"]

    @dbus.service.method(IFACE, out_signature="ssss")
    def GetServerInformation(self):
        return "faux", "tests", "1", "1.2"

    @dbus.service.method(IFACE, in_signature="susssasa{sv}i", out_signature="u")
    def Notify(self, app_name, replaces_id, app_icon, summary, body, actions, hints,
               expire_timeout):
        self._n += 1
        self.recues.append({
            "id": self._n, "app_name": str(app_name), "replaces_id": int(replaces_id),
            "app_icon": str(app_icon), "summary": str(summary), "body": str(body),
            "actions": [str(a) for a in actions],
            "hints": {str(k): (bool(v) if isinstance(v, dbus.Boolean) else
                               int(v) if isinstance(v, (dbus.Byte, dbus.Int32, dbus.UInt32,
                                                        dbus.Int64, dbus.UInt64)) else v)
                      for k, v in hints.items()},
            "expire_timeout": int(expire_timeout)})
        return dbus.UInt32(self._n)

    @dbus.service.method(IFACE, in_signature="u")
    def CloseNotification(self, nid):
        self.fermees.append(int(nid))
        self.NotificationClosed(nid, dbus.UInt32(3))

    @dbus.service.signal(IFACE, signature="uu")
    def NotificationClosed(self, nid, raison):
        pass

    @dbus.service.signal(IFACE, signature="us")
    def ActionInvoked(self, nid, cle):
        pass

    # --- cote test
    def invoquer(self, nid, cle):
        self.ActionInvoked(dbus.UInt32(nid), cle)

    def fermer_sans_action(self, nid):
        self.NotificationClosed(dbus.UInt32(nid), dbus.UInt32(2))


class Item:
    """Lecture de l'icone StatusNotifierItem et de son menu dbusmenu."""
    IFACE = "org.kde.StatusNotifierItem"

    def __init__(self, conn, service, chemin):
        self.conn = conn
        self.service = service
        self.obj = conn.get_object(service, chemin)
        self.props = dbus.Interface(self.obj, "org.freedesktop.DBus.Properties")

    def get(self, nom):
        return self.props.Get(self.IFACE, nom)

    def tooltip(self):
        _icone, _pix, titre, texte = self.get("ToolTip")
        return str(titre), _nettoyer(str(texte))

    def activer(self):
        dbus.Interface(self.obj, self.IFACE).Activate(0, 0)

    def _menu(self):
        return dbus.Interface(self.conn.get_object(self.service, str(self.get("Menu"))),
                              "com.canonical.dbusmenu")

    def entrees(self):
        """Liste des entrees du menu : dicts id, label, enabled, separator."""
        m = self._menu()
        try:
            m.AboutToShow(0)
        except dbus.DBusException:
            pass
        _rev, (_id, _p, enfants) = m.GetLayout(0, -1, [])
        sortie = []
        for e in enfants:
            eid, p, _ = e
            sortie.append({"id": int(eid), "label": str(p.get("label", "")),
                           "enabled": bool(p.get("enabled", True)),
                           "separator": str(p.get("type", "")) == "separator",
                           "visible": bool(p.get("visible", True))})
        return sortie

    def cliquer(self, label):
        e = next((e for e in self.entrees() if e["label"] == label), None)
        assert e, f"entree de menu {label!r} absente ; menu : {self.entrees()}"
        self._menu().Event(e["id"], "clicked", dbus.Int32(0, variant_level=1),
                           dbus.UInt32(0))


def _nettoyer(t):
    t = re.sub(r"<br\s*/?>", "\n", t)
    return re.sub(r"<[^>]+>", "", t)


def lignes_statut(item):
    """Lignes de statut du menu (entrees non cliquables avant le premier separateur)."""
    out = []
    for e in item.entrees():
        if e["separator"]:
            break
        out.append(e["label"])
    return out
