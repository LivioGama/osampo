import pytest

import outils
from outils import Monde


@pytest.fixture(scope="session")
def bins():
    b = outils._construire()
    for nom in ("osampo-pc", "osampo-reglages", "__daemon__"):
        if nom not in b:
            pytest.fail(f"binaire {nom} introuvable apres cargo build (SPEC-002, 004, 007)")
    return b


@pytest.fixture(scope="session")
def xvfb():
    import xfaux
    x = xfaux.Xvfb()
    yield x
    x.fermer()


@pytest.fixture
def monde(bins, request):
    """Fabrique de mondes : monde(config=..., pc=True, ...). Tout est arrete en fin de test.

    pc=False : daemon + faux BLE seulement (pas de D-Bus). pc=True : osampo-pc lance
    sur un bus de session prive (faux watcher, faux serveur de notifications) et un Xvfb.
    """
    crees = []

    def fabrique(pc=False, **kw):
        if pc:
            kw["x"] = request.getfixturevalue("xvfb")
        else:
            kw.setdefault("watcher", False)
            kw.setdefault("notif", False)
        m = Monde(bins, pc=pc, **kw)
        crees.append(m)
        return m
    yield fabrique
    for m in crees:
        m.fermer()
