#!/usr/bin/env bash
# Le verificateur de publication refuse chaque forme de fuite connue, et accepte un depot propre.
# Toutes les valeurs ci-dessous sont inventees.
set -euo pipefail
ICI=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
git -C "$T" init -q
mkdir -p "$T/tools" && cp "$ICI/tools/verifier_publication.sh" "$T/tools/"
printf 'Jeanne\nDupont\n' > "$T/.git/info/publication-identite"
echo propre > "$T/ok.txt"
bash "$T/tools/verifier_publication.sh" >/dev/null || { echo "ECHEC : depot propre refuse"; exit 1; }
n=0
refuse() { # <ligne> <categorie attendue>
    printf '%s\n' "$1" > "$T/fuite.txt"
    local sortie
    if sortie=$(bash "$T/tools/verifier_publication.sh" 2>&1); then
        echo "ECHEC : fuite acceptee : $1"; exit 1
    fi
    # un plantage du verificateur ne doit pas passer pour une fuite detectee
    case "$sortie" in *"publication refusee"*) ;; *) echo "ECHEC : sortie inattendue pour $1 : $sortie"; exit 1 ;; esac
    case "$sortie" in *": $2 :"*) ;; *) echo "ECHEC : categorie $2 absente pour $1 : $sortie"; exit 1 ;; esac
    n=$((n + 1))
}
refuse "usb-Espressif_USB_JTAG_serial_debug_unit_12:34:56:78:9A:BC-if00" "adresse materielle"
refuse "mac 12:34:56:78:9a:bc fin" "adresse materielle"
refuse "addr 12:34:56:78:9A:BC: connecte" "adresse materielle"
refuse "/home/quelquun/x" "chemin personnel"
refuse "/workspace/git/x" "chemin personnel"
refuse "https://claude.ai/code/session_x" "lien de session"
refuse "merci Jeanne" "identite"
refuse "DUPONT" "identite"
refuse "x@gmail.com" "identite"
printf 'AA:BB:CC:DD:EE:FF\naa:bb:cc:dd:ee:ff\n/home/UTILISATEUR/.local\n12:34:56:78:9A:BC:DE\n' > "$T/fuite.txt"
bash "$T/tools/verifier_publication.sh" >/dev/null || { echo "ECHEC : exceptions refusees"; exit 1; }
echo "test_verifier_publication OK ($n fuites refusees)"
