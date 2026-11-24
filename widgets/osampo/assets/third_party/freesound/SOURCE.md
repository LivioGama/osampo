# Freesound : enregistrements de chiots pour les sons du widget

`wouf.wav`, `aboie.wav` et `gemit.wav` viennent de vrais enregistrements de chiots
publiés sur [Freesound](https://freesound.org), tous sous licence
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) (domaine public,
aucune attribution exigée ; on cite quand même les auteurs). Licences lues par l'API
Freesound le 2026-11-21. Fichiers téléchargés par l'auteur le 2026-11-21, non commités
(sources lourdes) : retrouvables par l'URL, vérifiables par le sha256.

| Son | Freesound | Auteur | Fichier source | sha256 |
|---|---|---|---|---|
| `wouf.wav`, `aboie.wav`, `gemit.wav` | [583142](https://freesound.org/s/583142/) | AntumDeluge | `583142__antumdeluge__puppy-bark.flac` | `1122eb451c9e341f032f1ff4b009592d3884aa3ea3fec9c59cef2a45b1944383` |

## Refaire les sons

Sources dans un dossier `SRC`, depuis `widgets/osampo/tools` (ffmpeg 6.1.1) :

```sh
python3 importer_son.py SRC/583142__antumdeluge__puppy-bark.flac wouf --passe-haut 0 --crete 26860 --grave -3 --eq-board
python3 importer_son.py SRC/583142__antumdeluge__puppy-bark.flac aboie --debut 0 --duree 1.00 --passe-haut 0 --crete 29577 --grave -3 --eq-board
python3 importer_son.py SRC/583142__antumdeluge__puppy-bark.flac gemit --debut 0 --duree 0.50 --passe-haut 0 --crete 26860 --grave -3 --eq-board
python3 -c "import gen_assets as g; raise SystemExit(0 if g.verify_sounds() else 1)"
```

`importer_son.py` découpe, coupe les graves (passe-haut 350 Hz, que le haut-parleur ne
rend pas), passe en 16 kHz mono 16 bits, atténue le souffle entre les cris (`--porte`),
ramène à un niveau commun sans limiteur (RMS 3800, crête sous 30000) et pose des fondus de 5 ms. Ces commandes redonnent les WAV
commités à l'octet près (vérifié le 2026-11-21).

`wouf.wav` est l'enregistrement entier (1,48 s, trois aboiements), sans passe-haut, ramené
à la crête de l'ancien extrait (`--crete 26860`) : à l'écoute sur la board (l'auteur,
2026-11-21), l'extrait de 0,3 s sonnait abîmé, comme saturé, et le son entier mieux.

`gemit.wav` (patte levée, attente de Claude) n'est plus un gémissement : le cri de 559685
(belen_abo) était trop aigu pour une notification répétée (l'auteur). C'est le premier
aboiement de 583142, entier jusqu'au silence (0,5 s), même traitement que `wouf.wav`.
`aboie.wav` (relances) garde les deux premiers aboiements entiers (1 s), même traitement.

`jappe.wav` reste de synthèse : le candidat découpé dans 728030 (johnnypanic) sonnait
comme un gémissement. `gen_assets.py` (`ecrire_jappe`) le réécrit au même niveau que les
trois autres.

## Écartés

- 264833 (18hiltc) : CC-BY-NC, usage commercial interdit.
- 712675 (charcarbonetti) : CC-BY 4.0, utilisable avec crédit, pas retenu.
- 610540 (alejandrodiaz17), 396312 (cribbler) : candidats B, moins bons à l'écoute.
- 728030 (johnnypanic, CC0) : jappement A, écarté (un gémissement).
- 559685 (belen_abo, CC0) : gémissement de la patte levée, trop aigu à la longue.

## Rendu sur le haut-parleur de la board (SP5j)

Sur le petit haut-parleur (ampli NS4150B), les aboiements bruts sonnaient secs et très
courts comparés au PC : chaque aboiement ne dure que 0,1 s et le haut-parleur ne rend pas le
grave qui fait le corps du son. Comparés à l'écoute sur la board (l'auteur, 2026-11-21) :
égaliseur seul, égaliseur + petite pièce, égaliseur + 3 demi-tons plus grave. Retenu : 3
demi-tons plus grave (`--grave -3`, le son s'allonge d'autant, « chien plus gros ») et
l'égaliseur du haut-parleur (`--eq-board` : coupe sous 300 Hz, +6 dB vers 1 kHz, -5 dB
au-dessus de 4 kHz), pour wouf, aboie et gemit. La compression a été écartée à la mesure :
l'enregistrement est presque silencieux entre les aboiements, il n'y a rien à relever.

