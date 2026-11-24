# ADR-007 : la board aboie quelques secondes à chaque relance d'un rappel en laisse

## Statut
Accepté (l'auteur, 2026-11-22). Précise ADR-006, REQ-005 et REQ-012.

## Contexte
Pendant un rappel sortie, sport ou dodo (et la jauge vide), la board réclame laisse dans la
gueule du début à la fin ; ses relances ne sont que le son `Aboie`. Le chien du PC copie la pose
de la board (ADR-006) : il n'aboierait donc jamais aux relances, alors que l'auteur a dit « rappel =>
laisse, puis aboie dans les relances ». Options proposées : aboie dès la première relance et le
garde ; aboie quelques secondes à chaque relance puis reprend la laisse ; garder la laisse.

## Décision
À chaque relance d'une réclamation en laisse, la board prend la pose « aboie » pendant quelques
secondes, avec le son, puis reprend la laisse. Le chien du PC suit (icône et chien posé).

Choix de Claude, à vérifier : la pose « aboie » dure 3 s, fixe (pas de clé de réglage).

## Conséquences
- Firmware modifié (`anim_name` et la logique des relances) ; la board envoie la pose courante,
  le PC la suit donc sans nouvelle règle.
- SPEC-012 : retirer « escalade pour les rappels » du hors périmètre pour cette pose temporaire.
