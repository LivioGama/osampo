# ADR-003 : le chien en desktop pet au-dessus de la barre des tâches

## Statut
Accepté (l'auteur, 2026-11-22). Remplace la partie « sort puis rentre » de REQ-005. Partiellement remplacé par ADR-005 (le chien du coin revient).

## Contexte
Après l'essai de SP6 sur le PC, l'auteur constate que la zone de notification ne montre que la tête
du chiot (icône 22 px), et que le chien animé n'apparaît qu'au rappel de sortie (REQ-005).
Ce qu'il voulait : un « desktop pet », un animal posé en permanence sur la barre des tâches, en
bas à droite, qui joue ses animations.

## Décision
- Le chien est **toujours visible**, posé au-dessus de la barre des tâches, au-dessus de la zone
  de notification (bas droit). Il joue les animations de l'état du chiot de la board : assis,
  dort dans sa niche, aboie, joue, mange l'os ; pendant une promenade, il est absent (la laisse
  reste à sa place).
- Au rappel de sortie, il aboie sur place : il ne « sort » plus du coin et ne « rentre » plus
  (remplace REQ-005).
- Souris : clic gauche = chatouiller (la board réagit aussi), clic droit = le menu de l'icône
  (os, jauge, réglages), glisser-déposer = le déplacer le long de la barre ; la position est
  retenue d'une session à l'autre. Il ne prend jamais le focus du clavier.
- Taille réglable dans `osampo.toml`, 64 px par défaut.
- L'icône de la zone de notification reste (tête du chiot).

## Conséquences
- La fenêtre du chien (`crates/osampo-pc/src/fenetre.rs`) devient permanente ; REQ-005 et ses
  tests changent.
- Nouvelle clé de réglage côté PC, que le daemon doit accepter (il refuse un fichier inconnu en
  entier) et que la fenêtre de réglages doit afficher avec son aide (REQ-010).
- Ajoute REQ-011 au [besoin](../requirements.md) ; complète [ADR-001](001-sp6-chien-pc-hors-ligne-et-declencheurs.md)
  et [ADR-002](002-sp6b-jauge-reglable-et-aide-des-reglages.md).
