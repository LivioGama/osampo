# ADR-002 : jauge sage réglable depuis le PC et aide des réglages

## Statut
Accepté (l'auteur, 2026-11-22).

## Contexte
Après le build SP6 (PR #18), l'auteur voit le niveau de la jauge « sage » dans l'infobulle du PC,
mais ne peut ni le remplir, ni le vider, ni le fixer. La jauge est calculée par le daemon
(`daemon/src/sources/jauge.rs`) et envoyée à la board, qui en tire l'humeur du chiot et la
réclamation de promenade. Il voit aussi les réglages de `osampo.toml` dans la fenêtre
`osampo-reglages`, mais sans explication : seulement le nom de la clé, ses bornes et son
défaut (aide détaillée hors périmètre de SPEC-007).

## Décision
1. REQ-009 : depuis le PC, on peut remplir la jauge (100), la vider (0) ou la fixer à une valeur
   de 0 à 100. La valeur réglée est celle du daemon, et le chiot de la board est synchronisé en
   conséquence (« si on touche la valeur du daemon, il faut synchroniser le chiot », l'auteur).
2. REQ-010 : chaque réglage de la fenêtre porte un libellé en français et une phrase d'aide
   (rôle, unité, effet d'une valeur haute ou basse).

Choix de Claude, à vérifier : après un réglage, la jauge reprend son évolution normale à partir
de la valeur réglée, et se comporte comme si elle avait atteint cette valeur seule (une jauge
vidée à 0 déclenche donc la réclamation de promenade de ADR-001, QB-3).

## Conséquences
- Le daemon accepte une commande de réglage de la jauge sur son socket ; la valeur réglée doit
  survivre à un redémarrage du daemon comme la jauge actuelle.
- Les textes d'aide vivent dans la table des clés (`crates/osampo-config`), pas dans les
  commentaires du fichier, qu'un utilisateur peut supprimer.
- Précise REQ-009 et REQ-010 de [`docs/requirements.md`](../requirements.md) ; complète
  [ADR-001](001-sp6-chien-pc-hors-ligne-et-declencheurs.md).
