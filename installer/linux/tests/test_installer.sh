#!/usr/bin/env bash
# Teste installer.sh et desinstaller.sh dans un HOME jetable, avec un faux depot
# osampo (binaires factices), sans compiler ni toucher a systemd.
set -euo pipefail
ICI=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
INST=$ICI/..
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
ok=0
echec() { printf 'ECHEC : %s\n' "$*" >&2; exit 1; }
verifier() { if eval "$1"; then ok=$((ok + 1)); else echec "$1"; fi; }

# Faux depot
S=$T/osampo
mkdir -p "$S/target/release" "$S/daemon/hooks" "$S/widgets/osampo/sounds" \
         "$S/widgets/osampo/ui/sprites" "$S/crates/osampo-pc/icons/hicolor/22x22/status"
for b in target/release/osampo-pc target/release/osampo-reglages target/release/osampo-daemon; do
    printf '#!/bin/sh\necho %s\n' "$(basename "$b")" > "$S/$b"; chmod +x "$S/$b"
done
printf '#!/bin/sh\nexit 0\n' > "$S/daemon/hooks/claude_wait.sh"
echo 'id = "osampo"' > "$S/widgets/osampo/widget.toml"
touch "$S/widgets/osampo/sounds/wouf.wav" "$S/widgets/osampo/ui/sprites/aboie.png" \
      "$S/crates/osampo-pc/icons/hicolor/22x22/status/osampo-assis.png"

export HOME=$T/home XDG_CONFIG_HOME= XDG_STATE_HOME=
unset XDG_CONFIG_HOME XDG_STATE_HOME
mkdir -p "$HOME/.config/systemd/user" "$HOME/.config/autostart"
U=$HOME/.config/systemd/user
C=$HOME/.config
ST=$HOME/.local/state
# Ancienne installation depuis le depot, avec un port serie ; ancienne installation par
# l'installeur sous le nom de travail ; leur configuration et leur etat.
printf '[Service]\nExecStart=/depot/daemon --widgets-dir /depot/widgets --serial /dev/serial/by-id/ancien-port --ble\n' \
    > "$U/esp32-daemon.service"
printf '[Service]\nExecStart=/x/tagamoshi-daemon\n' > "$U/tagamoshi-daemon.service"
printf '[Desktop Entry]\nExec=/x/tagamoshi-pc\n' > "$C/autostart/tagamoshi-pc.desktop"
mkdir -p "$C/esp32-daemon" "$ST/esp32-daemon/activite" "$ST/tagamoshi-pc" "$ST/tagamoshi"
echo 'ancienne = 1' > "$C/esp32-daemon/tagamoshi.toml"
echo 42 > "$ST/esp32-daemon/jauge_tagamoshi"
echo r > "$ST/esp32-daemon/rappels_tagamoshi_pc"
echo AA:BB > "$ST/esp32-daemon/ble_tagamoshi"
echo jour > "$ST/esp32-daemon/activite/2026-11-23.csv"
echo 'x = 1' > "$ST/tagamoshi-pc/position.toml"
echo etat > "$ST/tagamoshi/state.txt"
mkdir -p "$HOME/.claude"
printf '{"hooks":{"Stop":[{"hooks":[{"command":"/vieux/daemon/hooks/claude_wait.sh 1"}]}]}}\n' > "$HOME/.claude/settings.json"
avant=$(sha256sum "$HOME/.claude/settings.json")

lancer() { bash "$INST/installer.sh" --source "$S" --sans-compiler --sans-demarrer "$@" > "$T/sortie" 2>&1 \
    || { cat "$T/sortie"; echec "installer.sh $*"; }; }

lancer
B=$HOME/.local/bin
D=$HOME/.local/share/osampo
verifier '[ -x "$B/osampo-pc" ] && [ -x "$B/osampo-reglages" ] && [ -x "$B/osampo-daemon" ]'
verifier '[ "$("$B/osampo-daemon")" = osampo-daemon ]'
verifier '[ -f "$D/widgets/osampo/widget.toml" ] && [ -f "$D/sounds/wouf.wav" ] && [ -f "$D/aboie.png" ]'
verifier '[ -f "$D/icons/hicolor/22x22/status/osampo-assis.png" ]'
verifier 'grep -q "^source=$S$" "$D/VERSION"'
unite=$U/osampo-daemon.service
verifier 'grep -q "^ExecStart=$B/osampo-daemon --widgets-dir $D/widgets" "$unite"'
if ! ls /dev/serial/by-id/*Espressif* >/dev/null 2>&1; then
    # sans board branchee, auto reprend le port de l'ancienne installation
    verifier 'grep -q -- "--serial /dev/serial/by-id/ancien-port --ble$" "$unite"'
fi
verifier 'grep -q "^Exec=$B/osampo-pc$" "$C/autostart/osampo-pc.desktop"'
verifier '[ -f "$U/esp32-daemon.service" ] && [ -f "$U/tagamoshi-daemon.service" ]' # --sans-demarrer : intacts
verifier '! ls "$B"/*.nouveau "$D.nouveau" >/dev/null 2>&1'

# Reprise de la configuration et de l'etat
verifier 'grep -q "ancienne = 1" "$C/osampo/osampo.toml"'
verifier '[ "$(cat "$ST/osampo/jauge_osampo")" = 42 ]'
verifier '[ "$(cat "$ST/osampo/rappels_osampo_pc")" = r ]'
verifier '[ "$(cat "$ST/osampo/ble_osampo")" = AA:BB ]'
verifier '[ -f "$ST/osampo/activite/2026-11-23.csv" ]'
verifier '[ -f "$ST/osampo-pc/position.toml" ] && [ "$(cat "$ST/osampo/state.txt")" = etat ]'
verifier '[ -f "$C/esp32-daemon/tagamoshi.toml" ]' # l'ancien reste en place
# Une cible existante n'est jamais ecrasee
echo 'nouvelle = 1' > "$C/osampo/osampo.toml"; echo 7 > "$ST/osampo/jauge_osampo"
lancer
verifier 'grep -q "nouvelle = 1" "$C/osampo/osampo.toml" && [ "$(cat "$ST/osampo/jauge_osampo")" = 7 ]'

# Hook Claude Code : installe, signale, settings.json intact
verifier '[ -x "$D/hooks/claude_wait.sh" ]'
verifier 'grep -qF "$D/hooks/claude_wait.sh" "$T/sortie"'
verifier 'grep -q "appelle un autre claude_wait.sh" "$T/sortie"'
verifier '[ "$(sha256sum "$HOME/.claude/settings.json")" = "$avant" ]'

# Relance (mise a jour) : idempotente ; le port vient maintenant de osampo-daemon.service
lancer --sans-ble
verifier 'grep -q -- "--serial /dev/serial/by-id/ancien-port$" "$unite" || ls /dev/serial/by-id/*Espressif* >/dev/null 2>&1'
lancer --serial aucun
verifier '! grep -q -- "--serial" "$unite"'
lancer --serial /dev/ttyACM7
verifier 'grep -q -- "--serial /dev/ttyACM7 --ble$" "$unite"'

# Vrai demarrage, avec de faux systemctl / pkill / setsid qui notent leurs appels (sans
# DISPLAY : osampo-pc n'est pas lance) : les anciens services sont arretes et mis de cote.
F=$T/faux && mkdir -p "$F"
for c in systemctl pkill setsid; do
    printf '#!/bin/sh\necho "%s $*" >> "%s/appels"\n' "$c" "$T" > "$F/$c"; chmod +x "$F/$c"
done
(unset DISPLAY; PATH=$F:$PATH bash "$INST/installer.sh" --source "$S" --sans-compiler --serial aucun) > "$T/sortie" 2>&1 \
    || { cat "$T/sortie"; echec "installer.sh avec demarrage"; }
verifier 'grep -q "^systemctl --user disable --now esp32-daemon$" "$T/appels"'
verifier 'grep -q "^systemctl --user disable --now tagamoshi-daemon$" "$T/appels"'
verifier 'grep -q "^systemctl --user enable --now osampo-daemon$" "$T/appels"'
verifier 'grep -q "^systemctl --user is-active --quiet osampo-daemon$" "$T/appels"'
verifier '[ ! -f "$U/esp32-daemon.service" ] && [ -f "$U/esp32-daemon.service.avant-osampo" ]'
verifier '[ ! -f "$U/tagamoshi-daemon.service" ] && [ -f "$U/tagamoshi-daemon.service.avant-osampo" ]'
verifier '[ ! -f "$C/autostart/tagamoshi-pc.desktop" ]'

# Erreurs
verifier '! bash "$INST/installer.sh" --source "$T/nulle-part" --sans-compiler --sans-demarrer >/dev/null 2>&1'
verifier '! bash "$INST/installer.sh" --option-inconnue >/dev/null 2>&1'

# Desinstallation : tout part, reglages gardes, anciens services remis en place
bash "$INST/desinstaller.sh" --sans-arreter > "$T/sortie"
verifier '[ ! -e "$B/osampo-pc" ] && [ ! -e "$D" ] && [ ! -e "$unite" ]'
verifier '[ ! -e "$C/autostart/osampo-pc.desktop" ]'
verifier '[ -f "$C/osampo/osampo.toml" ]'
verifier '[ -f "$U/esp32-daemon.service" ] && [ -f "$U/tagamoshi-daemon.service" ]'
verifier '[ -f "$C/autostart/tagamoshi-pc.desktop" ]'
verifier 'grep -q "enable --now esp32-daemon" "$T/sortie"'

echo "test_installer OK ($ok verifications)"
