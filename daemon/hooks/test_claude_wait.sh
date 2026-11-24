#!/bin/sh
# Test du hook : cd daemon && sh hooks/test_claude_wait.sh
h="$(dirname "$0")/claude_wait.sh"
export XDG_RUNTIME_DIR=$(mktemp -d)
d="$XDG_RUNTIME_DIR/osampo/claude_wait"
f="$d/abc"
ko() { echo "ECHEC: $1"; rm -rf "$XDG_RUNTIME_DIR"; exit 1; }
echo '{"session_id":"abc","x":1}' | "$h" 1 || ko "exit 1"
[ "$(cat "$f")" = "1" ] || ko "contenu 1"
# Meme contenu : le fichier n est pas reecrit (meme inode, pas de mv).
i1=$(ls -i "$f" | cut -d' ' -f1)
echo '{"session_id": "abc"}' | "$h" 1
[ "$(ls -i "$f" | cut -d' ' -f1)" = "$i1" ] || ko "reecrit alors qu inchange"
echo '{"session_id":"abc"}' | "$h" 0
[ "$(cat "$f")" = "0" ] || ko "contenu 0"
printf '{\n  "hook_event_name": "Stop",\n  "session_id" : "abc"\n}\n' | "$h" 1
[ "$(cat "$f")" = "1" ] || ko "json sur plusieurs lignes"
echo '{"session_id":"abc"}' | "$h" fin
[ ! -e "$f" ] || ko "fin doit supprimer"
echo 'pas du json' | "$h" 1 || ko "json invalide doit sortir 0"
"$h" 1 </dev/null || ko "stdin vide doit sortir 0"
[ -z "$(ls -A "$d" 2>/dev/null)" ] || ko "rien ecrit sans session_id"
echo '{"session_id":"../../evil"}' | "$h" 1
[ ! -e "$XDG_RUNTIME_DIR/evil" ] || ko "traversee"
[ "$(cat "$d/evil")" = "1" ] || ko "evil doit etre cree dans claude_wait"
[ -z "$(ls -A "$d" | grep '^\.')" ] || ko "temporaire laisse"
rm -rf "$XDG_RUNTIME_DIR"; echo "OK"
