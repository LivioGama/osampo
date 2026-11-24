#!/usr/bin/env bash
# Installe Osampo pour l'utilisateur courant, sans sudo :
#   - compile le daemon, osampo-pc et osampo-reglages depuis ce depot ;
#   - copie binaires et ressources sous ~/.local (independants du depot) ;
#   - pose le service systemd utilisateur du daemon et le demarrage de osampo-pc avec la
#     session graphique ;
#   - reprend la configuration et l'etat d'une installation precedente (nom de travail du
#     projet : esp32-daemon / tagamoshi) sans ecraser l'existant ;
#   - installe le hook Claude Code et affiche comment le brancher ;
#   - (re)demarre les deux.
# A relancer a chaque mise a jour : le script est idempotent.
#
# Usage : installer.sh [--source <depot osampo>] [--serial <port>|auto|aucun]
#                      [--sans-ble] [--sans-compiler] [--sans-demarrer] [--prefixe <dir>]
set -euo pipefail

ICI=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SOURCE=${OSAMPO_SOURCE:-$ICI/../..}
PREFIXE=$HOME/.local
SERIAL=auto
BLE=1
COMPILER=1
DEMARRER=1
SERVICE=osampo-daemon
# Services d'installations precedentes : depuis le depot (esp32-daemon), puis par
# l'installeur sous le nom de travail du projet (tagamoshi-daemon).
ANCIENS_SERVICES="esp32-daemon tagamoshi-daemon"

usage() { sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }
dire() { printf '%s\n' "$*"; }
mourir() { printf 'erreur : %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case $1 in
        --source) SOURCE=${2:?--source attend un chemin}; shift 2 ;;
        --serial) SERIAL=${2:?--serial attend un port, auto ou aucun}; shift 2 ;;
        --sans-ble) BLE=0; shift ;;
        --sans-compiler) COMPILER=0; shift ;;
        --sans-demarrer) DEMARRER=0; shift ;;
        --prefixe) PREFIXE=${2:?--prefixe attend un chemin}; shift 2 ;;
        -h|--help) usage ;;
        *) printf 'option inconnue : %s\n' "$1" >&2; usage 2 ;;
    esac
done

SOURCE=$(cd "$SOURCE" 2>/dev/null && pwd) || mourir "depot osampo introuvable (--source)"
[ -f "$SOURCE/widgets/osampo/widget.toml" ] || mourir "$SOURCE n'est pas un depot osampo"
BIN=$PREFIXE/bin
DATA=$PREFIXE/share/osampo
UNITES=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
AUTOSTART=${XDG_CONFIG_HOME:-$HOME/.config}/autostart
CONFIG=${XDG_CONFIG_HOME:-$HOME/.config}
STATE=${XDG_STATE_HOME:-$HOME/.local/state}
ETAT=$STATE/osampo

# --- 1. Compilation -----------------------------------------------------------------
if [ "$COMPILER" = 1 ]; then
    command -v cargo >/dev/null || mourir "cargo introuvable (https://rustup.rs, sans sudo)"
    dire "Compilation (release) depuis $SOURCE ..."
    (cd "$SOURCE" && cargo build --release -q -p osampo-pc -p osampo-reglages -p osampo-daemon)
fi
for f in "$SOURCE/target/release/osampo-pc" "$SOURCE/target/release/osampo-reglages" \
         "$SOURCE/target/release/osampo-daemon"; do
    [ -x "$f" ] || mourir "binaire absent : $f (relancer sans --sans-compiler)"
done

# --- 2. Port serie ------------------------------------------------------------------
# auto : le port Espressif branche, sinon celui de l'installation precedente, sinon aucun
# (Bluetooth seul). Le chemin by-id survit a une renumerotation des ttyACM*.
port_precedent() {
    local u
    for u in "$UNITES/$SERVICE.service" $(for a in $ANCIENS_SERVICES; do echo "$UNITES/$a.service"; done); do
        [ -f "$u" ] && sed -n 's/^ExecStart=.*--serial \([^ ]*\).*/\1/p' "$u" | head -n1 && return
    done
    return 0
}
if [ "$SERIAL" = auto ]; then
    SERIAL=$(ls /dev/serial/by-id/*Espressif* 2>/dev/null | head -n1 || true)
    [ -n "$SERIAL" ] || SERIAL=$(port_precedent)
    [ -n "$SERIAL" ] || SERIAL=aucun
fi

# --- 3. Arret de ce qui tourne ------------------------------------------------------
if [ "$DEMARRER" = 1 ]; then
    systemctl --user stop "$SERVICE" 2>/dev/null || true
    pkill -x osampo-pc 2>/dev/null || true
    pkill -x tagamoshi-pc 2>/dev/null || true
fi

# --- 3b. Reprise d'une installation precedente ----------------------------------------
# Copie seulement si la cible n'existe pas : une relance ne revient jamais en arriere.
reprendre() { # ancien nouveau
    [ -e "$1" ] && [ ! -e "$2" ] || return 0
    mkdir -p "$(dirname "$2")" && cp -a "$1" "$2" && dire "Repris : $1 -> $2"
}
reprendre "$CONFIG/esp32-daemon/tagamoshi.toml" "$CONFIG/osampo/osampo.toml"
reprendre "$STATE/esp32-daemon/activite" "$STATE/osampo/activite"
reprendre "$STATE/esp32-daemon/jauge_tagamoshi" "$STATE/osampo/jauge_osampo"
reprendre "$STATE/esp32-daemon/rappels_tagamoshi_pc" "$STATE/osampo/rappels_osampo_pc"
reprendre "$STATE/esp32-daemon/ble_tagamoshi" "$STATE/osampo/ble_osampo"
reprendre "$STATE/tagamoshi/state.txt" "$STATE/osampo/state.txt"
reprendre "$STATE/tagamoshi-pc/position.toml" "$STATE/osampo-pc/position.toml"

# --- 4. Copie : binaires et ressources ----------------------------------------------
# install -T vers un fichier temporaire puis mv : remplace un binaire en cours
# d'execution sans « Text file busy ».
copier() { # source destination mode
    install -D -m "$3" "$1" "$2.nouveau" && mv -f "$2.nouveau" "$2"
}
copier "$SOURCE/target/release/osampo-pc" "$BIN/osampo-pc" 755
copier "$SOURCE/target/release/osampo-reglages" "$BIN/osampo-reglages" 755
copier "$SOURCE/target/release/osampo-daemon" "$BIN/osampo-daemon" 755
rm -rf "$DATA.nouveau"
mkdir -p "$DATA.nouveau/sounds" "$DATA.nouveau/widgets/osampo" "$DATA.nouveau/hooks"
cp -r "$SOURCE/crates/osampo-pc/icons" "$DATA.nouveau/icons"
cp "$SOURCE"/widgets/osampo/sounds/*.wav "$DATA.nouveau/sounds/"
cp "$SOURCE/widgets/osampo/ui/sprites/aboie.png" "$DATA.nouveau/aboie.png"
cp "$SOURCE/widgets/osampo/widget.toml" "$DATA.nouveau/widgets/osampo/"
install -m 755 "$SOURCE/daemon/hooks/claude_wait.sh" "$DATA.nouveau/hooks/claude_wait.sh"
version=$(git -C "$SOURCE" describe --always --dirty 2>/dev/null || echo inconnue)
printf 'source=%s\nversion=%s\ninstalle=%s\n' "$SOURCE" "$version" "$(date -Iseconds)" \
    > "$DATA.nouveau/VERSION"
rm -rf "$DATA" && mv "$DATA.nouveau" "$DATA"

# --- 5. Service systemd du daemon ---------------------------------------------------
args="--widgets-dir $DATA/widgets"
[ "$SERIAL" != aucun ] && args="$args --serial $SERIAL"
[ "$BLE" = 1 ] && args="$args --ble"
mkdir -p "$UNITES"
cat > "$UNITES/$SERVICE.service" <<EOF
# Pose par osampo/installer/linux/installer.sh ($version) : relancer le script plutot que
# modifier ce fichier. Un seul programme a la fois sur le port serie : arreter l'unite
# avant idf.py flash (systemctl --user stop $SERVICE).
#
# DISPLAY et XAUTHORITY sont figes ici : le service demarre avec la connexion, avant que
# la session graphique n'exporte son environnement (activite clavier/souris par X).
[Unit]
Description=Daemon Osampo (nourrit le chiot par USB et Bluetooth)
StartLimitIntervalSec=300
StartLimitBurst=5

[Service]
Type=simple
ExecStart=$BIN/osampo-daemon $args
Environment=DISPLAY=${DISPLAY:-:0}
Environment=XAUTHORITY=${XAUTHORITY:-%h/.Xauthority}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# --- 6. Demarrage de osampo-pc avec la session --------------------------------------
mkdir -p "$AUTOSTART"
cat > "$AUTOSTART/osampo-pc.desktop" <<EOF
[Desktop Entry]
# Pose par osampo/installer/linux/installer.sh ($version).
Type=Application
Name=Osampo
Comment=Le chiot dans la zone de notification
Exec=$BIN/osampo-pc
Terminal=false
X-GNOME-Autostart-enabled=true
EOF

# --- 7. Installations precedentes (services et autostart) ----------------------------
# Seulement quand on demarre : sinon les anciens services restent en place et actifs.
if [ "$DEMARRER" = 1 ]; then
    for a in $ANCIENS_SERVICES; do
        ancien=$UNITES/$a.service
        [ -f "$ancien" ] || continue
        systemctl --user disable --now "$a" 2>/dev/null || true
        mv -f "$ancien" "$ancien.avant-osampo"
        if [ -d "$ancien.d" ]; then mv -f "$ancien.d" "$ancien.d.avant-osampo"; fi
        dire "Ancien service $a arrete, garde en $ancien.avant-osampo"
    done
    ancien=$AUTOSTART/tagamoshi-pc.desktop
    if [ -f "$ancien" ]; then mv -f "$ancien" "$ancien.avant-osampo"; fi
fi

# --- 8. Demarrage ------------------------------------------------------------------
if [ "$DEMARRER" = 1 ]; then
    systemctl --user daemon-reload
    systemctl --user enable --now "$SERVICE" >/dev/null 2>&1
    systemctl --user restart "$SERVICE"
    mkdir -p "$ETAT"
    if [ -n "${DISPLAY:-}" ]; then
        setsid nohup "$BIN/osampo-pc" >"$ETAT/osampo-pc.log" 2>&1 < /dev/null &
    fi
    sleep 2
    systemctl --user is-active --quiet "$SERVICE" || mourir "le service $SERVICE ne demarre pas : journalctl --user -u $SERVICE"
    if [ -n "${DISPLAY:-}" ]; then
        pgrep -x osampo-pc >/dev/null || mourir "osampo-pc ne demarre pas : $ETAT/osampo-pc.log"
    fi
fi

dire "Osampo installe ($version)"
dire "  binaires   : $BIN/osampo-{pc,reglages,daemon}"
dire "  ressources : $DATA"
dire "  service    : $SERVICE (port serie : $SERIAL, Bluetooth : $([ "$BLE" = 1 ] && echo oui || echo non))"
dire "  reglages   : $CONFIG/osampo/osampo.toml"
[ "$DEMARRER" = 1 ] || dire "  rien n'a ete demarre ni active (--sans-demarrer) : relancer sans cette option"

# --- 9. Hook Claude Code -------------------------------------------------------------
# settings.json appartient a l'utilisateur : on montre quoi y mettre, on n'y touche pas.
HOOK=$DATA/hooks/claude_wait.sh
REGLAGES_CLAUDE=$HOME/.claude/settings.json
dire ""
dire "Hook Claude Code (la patte du chiot quand Claude attend) : dans $REGLAGES_CLAUDE,"
dire "les commandes des hooks appellent  $HOOK <0|1|fin>"
dire "  (exemple complet : $SOURCE/daemon/hooks/README.md)"
if [ -f "$REGLAGES_CLAUDE" ] && grep -q 'daemon/hooks/claude_wait.sh' "$REGLAGES_CLAUDE" \
        && ! grep -qF "$HOOK" "$REGLAGES_CLAUDE"; then
    dire "  $REGLAGES_CLAUDE appelle un autre claude_wait.sh : remplacer son chemin par $HOOK"
fi
