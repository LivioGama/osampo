# ADR-005 : le chien qui sort du coin est gardé, en plus du chien posé

## Statut
Remplacé par ADR-008 (l'auteur, 2026-11-22, soir). Accepté (l'auteur, 2026-11-22). Remplace, dans ADR-003, la phrase « Au rappel de sortie, il aboie
sur place : il ne sort plus du coin et ne rentre plus ».

## Contexte
ADR-003 a rendu le chien permanent sur la barre des tâches et a supprimé le chien qui sortait du
coin au rappel de sortie (REQ-005 première version). Après l'avoir vu, l'auteur veut garder les
deux : le chien qui sort du coin **et** la notification accepter/reporter (REQ-004).

## Décision
- Au rappel de sortie (REQ-004), le chien sort du coin bas droit de l'écran en aboyant, comme dans
  la première version de REQ-005, puis rentre quand le rappel est accepté ou reporté (sur le PC ou
  sur la board).
- Pendant ce temps, le chien posé sur la barre (REQ-011) aboie aussi ; il ne disparaît jamais.
- La notification de bureau accepter/reporter (REQ-004) est inchangée.

## Conséquences
- REQ-005 retrouve son critère de première version ; ses tests sont à réécrire une nouvelle fois.
- Deux fenêtres du chien au rappel : la fenêtre de sortie (temporaire) et le chien posé
  (permanent). Elles ne doivent pas se recouvrir de façon gênante.
