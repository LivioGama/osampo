#!/usr/bin/env python3
"""Regle l'horloge de la board Osampo 1.83 par l'USB.

Usage : sync_time.py [--port /dev/ttyACM0] [--heure HH:MM] [--suivre SECONDES]

Ouvre le port avec DTR/RTS a false (cela relance quand meme la puce sur
l'USB-Serial/JTAG), attend `chiot lance`, envoie {"epoch":E,"tz_s":T,"force":1}
et attend `heure reglee epoch=E`. "force":1 oblige la board a ecrire et
accuser meme si son heure est deja juste (sans lui, elle ignore un ecart <= 2 s). 3 essais ; code de sortie 0 si accuse.
Sans --heure : heure reelle du PC. Avec --heure : la date locale du jour a
HH:MM. tz_s est le decalage UTC actuel du PC (heure d'ete comprise).
--suivre N : continue d'afficher le journal N secondes apres l'accuse.
"""
import argparse
import json
import sys
import time

import serial

ATTENTE_DEMARRAGE_S = 10.0
ATTENTE_ACCUSE_S = 5.0
ESSAIS = 3


def calcule_trame(heure, debut):
    """Trame a envoyer maintenant. `debut` = time.monotonic() du lancement :
    avec --heure, HH:MM:00 avance des secondes ecoulees depuis le lancement."""
    if heure is None:
        epoch = round(time.time())
    else:
        now = time.localtime()
        hh, mm = (int(x) for x in heure.split(":"))
        if not (0 <= hh < 24 and 0 <= mm < 60):
            raise ValueError("heure hors bornes")
        # tm_isdst = -1 : mktime decide de l'heure d'ete pour cette date.
        base = int(time.mktime((now.tm_year, now.tm_mon, now.tm_mday, hh, mm, 0, 0, 0, -1)))
        epoch = base + int(time.monotonic() - debut)
    # Decalage de l'instant vise, pas de maintenant.
    tz_s = time.localtime(epoch).tm_gmtoff
    return epoch, tz_s


def attend(port, motif, duree):
    """Lit jusqu'a voir `motif` (None : lit toute la duree) ; True si vu.
    Imprime tout ce qui arrive."""
    fin = time.monotonic() + duree
    tampon = ""
    while time.monotonic() < fin:
        data = port.read(4096)
        if not data:
            continue
        txt = data.decode("utf-8", errors="replace")
        sys.stdout.write(txt)
        sys.stdout.flush()
        if motif is None:
            continue
        tampon += txt
        if motif in tampon:
            return True
        tampon = tampon[-200:]
    return False


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--port", default="/dev/ttyACM0")
    ap.add_argument("--heure", metavar="HH:MM")
    ap.add_argument("--suivre", type=float, default=0.0, metavar="SECONDES")
    args = ap.parse_args()

    debut = time.monotonic()
    try:
        calcule_trame(args.heure, debut)
    except ValueError as err:
        ap.error(f"--heure : {err}")

    s = serial.Serial()
    s.port = args.port
    s.baudrate = 115200
    s.timeout = 0.2
    s.dtr = False
    s.rts = False
    try:
        s.open()
    except serial.SerialException as err:
        print(f"[sync_time] port inutilisable : {err}", file=sys.stderr)
        return 2
    try:
        ok = False
        # La puce redemarre a l'ouverture : on attend son message de fin d'init.
        if not attend(s, "chiot lance", ATTENTE_DEMARRAGE_S):
            print(f"\n[sync_time] pas de `chiot lance` en {ATTENTE_DEMARRAGE_S:g} s, j'envoie quand meme", file=sys.stderr)
        for essai in range(1, ESSAIS + 1):
            epoch, tz_s = calcule_trame(args.heure, debut)
            trame = json.dumps({"epoch": epoch, "tz_s": tz_s, "force": 1}, separators=(",", ":"))
            print(f"\n[sync_time] essai {essai}/{ESSAIS} envoi : {trame}")
            s.write((trame + "\n").encode())
            s.flush()
            if attend(s, f"heure reglee epoch={epoch}", ATTENTE_ACCUSE_S):
                ok = True
                break
        if ok:
            print(f"\n[sync_time] accuse recu : heure reglee epoch={epoch}")
            if args.suivre > 0:
                attend(s, None, args.suivre)
        else:
            print("\n[sync_time] ECHEC : pas d'accuse", file=sys.stderr)
        return 0 if ok else 1
    except serial.SerialException as err:
        print(f"\n[sync_time] erreur serie : {err}", file=sys.stderr)
        return 2
    finally:
        s.close()


if __name__ == "__main__":
    sys.exit(main())
