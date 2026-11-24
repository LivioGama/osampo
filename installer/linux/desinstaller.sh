#!/usr/bin/env bash
# Desinstalle Osampo (ce qu'a pose installer.sh). Garde les reglages
# (~/.config/osampo/) et l'historique (~/.local/state/osampo/). Remet en place les
# services d'une installation precedente mis de cote par installer.sh (sans les demarrer).
#
# Usage : desinstaller.sh [--sans-arreter] [--prefixe <dir>]
set -euo pipefail

PREFIXE=$HOME/.local
ARRETER=1
while [ $# -gt 0 ]; do
    case $1 in
        --sans-arreter) ARRETER=0; shift ;;
        --prefixe) PREFIXE=${2:?--prefixe attend un chemin}; shift 2 ;;
        -h|--help) sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) printf 'option inconnue : %s\n' "$1" >&2; exit 2 ;;
    esac
done
UNITES=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
AUTOSTART=${XDG_CONFIG_HOME:-$HOME/.config}/autostart

if [ "$ARRETER" = 1 ]; then
    systemctl --user disable --now osampo-daemon 2>/dev/null || true
    pkill -x osampo-pc 2>/dev/null || true
fi
rm -f "$UNITES/osampo-daemon.service" "$AUTOSTART/osampo-pc.desktop"
rm -f "$PREFIXE"/bin/osampo-{pc,reglages,daemon}
rm -rf "$PREFIXE/share/osampo"

echo "Osampo desinstalle (reglages et historique gardes)."
for f in "$UNITES"/*.service.avant-osampo "$UNITES"/*.service.d.avant-osampo \
         "$AUTOSTART"/*.desktop.avant-osampo; do
    [ -e "$f" ] || continue
    mv -f "$f" "${f%.avant-osampo}"
    echo "Remis en place : ${f%.avant-osampo}"
    case $f in
        *.service.avant-osampo)
            s=$(basename "${f%.service.avant-osampo}")
            echo "  pour le relancer : systemctl --user enable --now $s (un seul : ils partagent le port serie)" ;;
    esac
done
[ "$ARRETER" = 1 ] && systemctl --user daemon-reload 2>/dev/null || true
exit 0
