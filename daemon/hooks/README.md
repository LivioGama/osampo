# Hook Claude Code pour le chiot

*In English: the JSON block below is what goes into `~/.claude/settings.json` (merge it with
your existing hooks); `UTILISATEUR` is your user name. The rest of this page is in French.*

`claude_wait.sh <0|1|fin>` lit le JSON du hook sur stdin (champ `session_id`) et
tient `$XDG_RUNTIME_DIR/osampo/claude_wait/<session_id>` (`0` ou `1`;
repli `/tmp/osampo-daemon-$UID/claude_wait/`, cree en 700 et refuse s il est un
lien ou appartient a un autre). Le daemon lit ce repertoire : un fichier a `1`
de moins de 7200 s = une session attend. Le script sort toujours en 0, ne
demande que `sh`, `sed` et `tr`, et ne reecrit pas un fichier deja a la bonne
valeur.

Chaque nouvelle attente d une session (passage a `1`) re-arme la patte du
chiot, meme si une autre session attendait deja.

## A ajouter dans `~/.claude/settings.json`

A **fusionner** avec les hooks deja presents (ajouter chaque entree au tableau
de l evenement correspondant, sans remplacer l existant). `matcher` ne compte
que pour les evenements d outils (`PreToolUse`, `PostToolUse`) ; vide = tous.
`installer/linux/installer.sh` pose le hook dans `~/.local/share/osampo/hooks/` ;
remplacer `UTILISATEUR` par son nom d utilisateur (`command` n expanse pas `~`).

`PostToolUse` remet a 0 apres un outil : une permission accordee
(`Notification` -> 1) ne laisse pas le chiot reclamer pendant que l outil
tourne. `"timeout": 5` borne le hook a 5 s quoi qu il arrive.

```json
{
  "hooks": {
    "Notification": [
      { "matcher": "", "hooks": [ { "type": "command", "command": "/home/UTILISATEUR/.local/share/osampo/hooks/claude_wait.sh 1", "timeout": 5 } ] }
    ],
    "Stop": [
      { "matcher": "", "hooks": [ { "type": "command", "command": "/home/UTILISATEUR/.local/share/osampo/hooks/claude_wait.sh 1", "timeout": 5 } ] }
    ],
    "UserPromptSubmit": [
      { "matcher": "", "hooks": [ { "type": "command", "command": "/home/UTILISATEUR/.local/share/osampo/hooks/claude_wait.sh 0", "timeout": 5 } ] }
    ],
    "PreToolUse": [
      { "matcher": "", "hooks": [ { "type": "command", "command": "/home/UTILISATEUR/.local/share/osampo/hooks/claude_wait.sh 0", "timeout": 5 } ] }
    ],
    "PostToolUse": [
      { "matcher": "", "hooks": [ { "type": "command", "command": "/home/UTILISATEUR/.local/share/osampo/hooks/claude_wait.sh 0", "timeout": 5 } ] }
    ],
    "SessionEnd": [
      { "matcher": "", "hooks": [ { "type": "command", "command": "/home/UTILISATEUR/.local/share/osampo/hooks/claude_wait.sh fin", "timeout": 5 } ] }
    ]
  }
}
```

Test : `cd daemon && sh hooks/test_claude_wait.sh`.

## Lancer le daemon pour le chiot

La board (Waveshare ESP32-S3 1.83", firmware `widgets/osampo/esp32`) recoit
une ligne JSON par l'USB ou le Bluetooth (`epoch`, `tz_s`, `sage`, `actif`,
`absence_min`, `absence_finie_min`, `absence_finie_age_min`, `claude_wait`, `age`). Un seul programme a la fois sur le port : arreter le
daemon avant `idf.py flash` ou `widgets/osampo/tools/sync_time.py`. Ouvrir le port
relance la puce (normal).

A la main, depuis `daemon/` :

```sh
cargo build --release
../target/release/osampo-daemon --widgets-dir ../widgets \
  --serial /dev/serial/by-id/<port de la board>   # ls /dev/serial/by-id/
```

Options utiles pour un essai : `--unite-s 1` (une minute de la jauge dure 1 s :
elle se vide en 90 s d activite), `--echo-serie` (recopie sur stderr, prefixees
`[board] `, les lignes que la board ecrit), `--socket <chemin>` (socket de
controle hors du chemin par defaut).

## Reglages du chiot : `osampo.toml`

Le daemon lit `$XDG_CONFIG_HOME/osampo/osampo.toml` (repli
`~/.config/osampo/osampo.toml`), ou le chemin donne par
`--config <chemin>`. S il manque, il le cree depuis le modele commente
`widgets/osampo/config.toml` : chaque cle y est a sa valeur par defaut, avec
son unite et ses bornes ; supprimer une ligne revient au defaut.

- **Relecture a chaud** : la date et la taille du fichier sont regardees
  toutes les 2 s. Un fichier valide est applique tout de suite, sans relancer
  le daemon ; un fichier invalide est refuse en entier (le journal du daemon
  nomme la cle fautive) et les reglages precedents restent en place.
  Un fichier vide est ignore de meme (etat transitoire d un editeur qui
  ecrit sur place) : pour revenir aux defauts, supprimer les lignes en
  gardant au moins un commentaire, ou supprimer le fichier et relancer.
- **Board** : les reglages partent en une ligne `{"cfg":1,...}` a chaque
  ouverture du port et a chaque changement. La board les applique, les garde
  en NVS (`config appliquee` sur la console) et les retrouve sans le daemon.
- **Desktop** : les clients socket du widget `osampo` recoivent la meme
  ligne (touches de la section `[clavier]` comprises).
- **Promenade** (SP5h) : la section `[promenade]` regle la jauge « sage ». Les
  anciennes cles `[poste] laisse_apres_min` et `pause_min` restent acceptees
  (memes valeurs que `vide_apres_min` et `courte_min`), pas les deux sections a la
  fois. L etat de la jauge est garde dans `$XDG_STATE_HOME/osampo/jauge_osampo`,
  l historique de la journee (une ligne `HH:MM,actif,sage` par minute) dans
  `$XDG_STATE_HOME/osampo/activite/AAAA-MM-JJ.csv`, 30 jours.
- **Gestes** : la section `[gestes]` regle les seuils de lancer, secousse et
  retournement (calibres sur la board le 2026-11-20). `journal = 1` imprime une
  ligne par salve sur la console de la board, `journal = 2` aussi les
  echantillons bruts (pour calibrer ; l impression peut alors faire perdre des
  echantillons). Pour les lire, lancer le daemon a la main avec
  `--echo-serie` (l unite systemd ne le passe pas).

La board ne reecrit son RTC (et n imprime `heure reglee`) que si l heure recue
s ecarte de plus de 2 s de la sienne, si `tz_s` change, ou si la ligne porte
`"force":1` (ce que fait `widgets/osampo/tools/sync_time.py`).

En service : `daemon/systemd/osampo-daemon.service` (unite utilisateur, chemins
a adapter, mode d installation en tete du fichier). L activite vient de X :
l unite fixe `DISPLAY=:0` et `XAUTHORITY=%h/.Xauthority` (valeurs d une session
lightdm ; a adapter). Un `systemctl --user import-environment` en debut de
session arrive trop tard : le service, lance des la connexion, a deja fige son
environnement. Le chemin `/dev/serial/by-id/...` est celui de cette board :
le remplacer par la sortie de `ls /dev/serial/by-id/`.
