# ADR-001 : chien du PC hors ligne et déclencheurs du rappel de sortie
* Statut : accepté
* Décideurs : l'auteur
* Date : 2026-11-22
* Ticket technique : blocage `besoin` de l'analyste, mode specs (SPEC-002, SPEC-003)

## Contexte
SP6 fait du chien du PC et de celui de la board un seul chien (REQ-007). Le dodo, la promenade et
les besoins ne sont calculés que par la board ; le daemon ne calcule que la jauge sage, l'absence
et les rappels. Le cas courant de board hors ligne est la promenade, board emportée.

## Enoncé du problème
1. QA (REQ-001, REQ-007) : board hors ligne, quel état l'icône du PC affiche-t-elle ?
2. QB (REQ-004, REQ-005) : quelles réclamations déclenchent la notification de bureau et le chien
   qui sort du coin ?

## Options envisagées
QA :
* QA-1 : figer le dernier état reçu de la board, avec la mention « board hors ligne », sauf
  « aboie » quand un rappel du daemon réclame.
* QA-2 : le daemon fait tourner sa propre copie de la logique du chiot.
* QA-3 : règle simple du daemon (aboie si rappel, laisse si absence après sortie acceptée, niche
  pendant `[heures] nuit_*`, assis sinon).

QB :
* QB-1 : seuls les rappels de catégorie sortie.
* QB-2 : les rappels sortie, sport et dodo (tous ceux qui partent en laisse).
* QB-3 : QB-2, plus la réclamation de promenade de la jauge sage à 0.

## Résultat de la décision
Option choisie : « QA-1 », car elle est simple et juste dans le cas courant (promenade board
emportée : le dernier état reçu est la laisse).

Option choisie : « QB-3 », car la demande d'origine est que le chien vienne notifier et aboyer
quand il faut sortir, et la jauge vide est un tel moment.

Choix de détail de l'analyste confirmés en même temps (SPEC-001, SPEC-006) : sur la board, un
appui long reporte une sortie ; la fenêtre du chien sort du coin bas droit de l'écran ; les
réactions à la chatouille et à l'os se jouent dans cette fenêtre.

### Conséquences positives
* Aucune logique du chiot à dupliquer dans le daemon.

### Conséquences négatives
* Hors ligne, un dodo ou un réveil sur la board n'apparaît sur le PC qu'au retour du lien.

## Avantages et inconvénients des options

### QA-2
* Positif, car le PC resterait fidèle hors ligne.
* Négatif, car la logique est en C++ côté board : portage ou liaison C++ → Rust, et deux copies
  qui divergent dès que la board change de version.

### QA-3
* Positif, car le PC évolue même hors ligne.
* Négatif, car l'état affiché peut contredire celui de la board au retour du lien.

## Liens
* Précise REQ-001, REQ-004, REQ-005, REQ-007 de [`docs/requirements.md`](../requirements.md)
