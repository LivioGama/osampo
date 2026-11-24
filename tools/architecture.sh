#!/usr/bin/env bash
# Regenere docs/architecture.md depuis le modele LikeC4 (docs/architecture/osampo.c4) :
# LikeC4 -> Mermaid, converti en syntaxe Mermaid classique (affichee par GitHub).
# Necessite Node.js (npx). Usage : tools/architecture.sh
set -euo pipefail
racine=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
(cd "$tmp" && npx -y likec4@latest validate "$racine/docs/architecture" >/dev/null \
  && npx -y likec4@latest gen mermaid --outdir "$tmp/mmd" "$racine/docs/architecture" >/dev/null)
python3 - "$tmp/mmd" "$racine/docs/architecture.md" <<'PY'
import re, sys, pathlib
src, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
ordre = [("index", "Context"), ("containers", "Inside the board and the PC"),
         ("daemon", "The daemon"), ("claudeWaits", "When Claude Code waits")]
def classique(t):
    t = re.sub(r"^---\ntitle:.*\n---\n", "", t)
    ident = lambda m: m.group(0).replace(".", "_")
    t = re.sub(r"\b[A-Z][A-Za-z0-9]*(?:\.[A-Z][A-Za-z0-9]*)+\b", ident, t)
    forme = {"disk": '[("{}")]', "rounded": '(["{}"])'}
    def noeud(m):
        nom, attrs = m.group(1), m.group(2)
        lab = re.search(r'label: "([^"]*)"', attrs).group(1)
        sh = re.search(r"shape: (\w+)", attrs)
        return nom + forme.get(sh.group(1) if sh else "", '["{}"]').format(lab)
    t = re.sub(r"(\w+)@\{([^}]*)\}", noeud, t)
    t = re.sub(r'\["`([^`]*)`"\]', r'["\1"]', t)
    t = t.replace('"`', '"').replace('`"', '"')
    return t.replace(' -. "[...]" .-> ', ' -.-> ')  # relations regroupees, sans titre
parts = ["# Architecture\n",
         "Generated from the LikeC4 model [`docs/architecture/osampo.c4`](architecture/osampo.c4) "
         "by `tools/architecture.sh`. Do not edit by hand.\n",
         "*Généré depuis le modèle LikeC4 par `tools/architecture.sh` : ne pas modifier à la main.*\n"]
for nom, titre in ordre:
    parts.append(f"## {titre}\n\n```mermaid\n{classique((src / (nom + '.mmd')).read_text()).strip()}\n```\n")
out.write_text("\n".join(parts))
print(f"ecrit {out}")
PY
