#!/usr/bin/env bash
# Refuse la publication si un fichier suivi (nom ou contenu texte) contient une donnee
# personnelle : chemins de machine, identite, adresses materielles, liens de session.
# Usage : tools/verifier_publication.sh   (depuis n'importe ou dans le depot)
#
# Les noms propres a refuser (prenom, nom...) ne sont pas dans le depot : un par ligne
# dans .git/info/publication-identite (jamais suivi). Sans ce fichier, un avertissement.
set -euo pipefail
cd "$(dirname "$0")" && cd "$(git rev-parse --show-toplevel)"
identite_locale="$(git rev-parse --git-dir)/info/publication-identite"
IDENTITE_LOCALE="$identite_locale" python3 - <<'PY'
import os, re, subprocess, sys
noms = []
try:
    noms = [l.strip() for l in open(os.environ["IDENTITE_LOCALE"], encoding="utf-8") if l.strip()]
except FileNotFoundError:
    print("avertissement : pas de .git/info/publication-identite, noms propres non verifies",
          file=sys.stderr)
identite = "|".join([r"\b" + re.escape(n) + r"\b" for n in noms] + ["@gmail"])
motifs = {
    "chemin personnel": r"/home/(?!UTILISATEUR\b|user\b)[A-Za-z0-9_.-]+|/workspace/|CHEMIN/",
    "identite": r"(?i)" + identite,
    # AA:BB:CC:DD:EE:FF (toute casse) : adresse factice des tests
    # pas de \b : une adresse collee a un "_" (chemin /dev/serial/by-id) doit etre vue ;
    # une adresse suivie de ":" aussi, mais pas une suite hexa plus longue.
    "adresse materielle": r"(?<![0-9A-Fa-f:])(?!(?i:AA:BB:CC:DD:EE:FF))(?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}(?!:?[0-9A-Fa-f])",
    "lien de session": r"claude\.ai/code/session",
}
fichiers = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], capture_output=True,
                          text=True, check=True).stdout.splitlines()
fautes = []
for f in fichiers:
    if f in ("tools/verifier_publication.sh", "tests/test_verifier_publication.sh"): continue  # motifs et fuites de test
    for nom, m in motifs.items():
        if re.search(m, f): fautes.append(f"{f} : nom de fichier ({nom})")
    try:
        texte = open(f, encoding="utf-8").read()
    except (UnicodeDecodeError, IsADirectoryError, FileNotFoundError):
        continue
    for i, ligne in enumerate(texte.splitlines(), 1):
        for nom, m in motifs.items():
            if re.search(m, ligne): fautes.append(f"{f}:{i} : {nom} : {ligne.strip()[:100]}")
if fautes:
    print("\n".join(fautes)); print(f"publication refusee : {len(fautes)} occurrence(s)"); sys.exit(1)
print("publication ok")
PY
