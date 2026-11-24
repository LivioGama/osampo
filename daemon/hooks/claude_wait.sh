#!/bin/sh
# Hook Claude Code : signale au daemon Osampo qu une session attend l utilisateur.
# Usage : claude_wait.sh <0|1|fin>   (JSON du hook sur stdin, champ session_id)
# Ne bloque jamais Claude Code : sort toujours en 0. Pas de python ni de jq :
# ce hook tourne a chaque appel d outil.

etat="$1"
# Lance a la main depuis un terminal : rien a lire, on ne bloque pas sur cat.
[ -t 0 ] && exit 0
json=$(cat 2>/dev/null)

# Repertoire a nous seul : existe (ou cree en 700), pas un lien, proprietaire = nous.
dir_sur() {
    mkdir -m 700 "$1" 2>/dev/null
    [ ! -L "$1" ] && [ -d "$1" ] && [ -O "$1" ]
}

if [ -n "$XDG_RUNTIME_DIR" ]; then
    dir="$XDG_RUNTIME_DIR/osampo/claude_wait"
    mkdir -p "$dir" 2>/dev/null || exit 0
else
    # /tmp est partage : un autre utilisateur a pu y poser le repertoire avant nous.
    base="/tmp/osampo-daemon-$(id -u)"
    dir="$base/claude_wait"
    dir_sur "$base" || exit 0
    dir_sur "$dir" || exit 0
fi

# session_id par sed ; on ne garde que [A-Za-z0-9_-] (pas de traversee de chemin).
sid=$(printf '%s' "$json" | tr '\n' ' ' \
    | sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | tr -cd 'A-Za-z0-9_-')
[ -n "$sid" ] || exit 0

case "$etat" in
    0|1)
        # Contenu deja identique : pas de reecriture (ni de nouveau mtime).
        [ "$(cat "$dir/$sid" 2>/dev/null)" = "$etat" ] && exit 0
        tmp="$dir/.$sid.$$"
        if printf '%s\n' "$etat" > "$tmp" 2>/dev/null; then
            mv -f "$tmp" "$dir/$sid" 2>/dev/null || rm -f "$tmp"
        else
            rm -f "$tmp"
        fi
        ;;
    fin)
        rm -f "$dir/$sid" 2>/dev/null
        ;;
esac
exit 0
