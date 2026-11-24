# ADR-008 : un seul chien au rappel, celui posé sur la barre

## Statut
Accepté (l'auteur, 2026-11-22, soir). Remplace ADR-005.

## Contexte
ADR-005 avait gardé deux chiens au rappel de sortie : la fenêtre qui sort du coin de l'écran en
aboyant, et le chien posé sur la barre qui prend la pose de la board (ADR-006, ADR-007). À
l'usage, l'auteur voit « 2 chiens qui apparaissent » et préfère n'en garder qu'un : « le chien posé
sur la barre qui fait la bonne animation ».

## Décision
- Au rappel (`sortie`, `sport`, `dodo`, jauge sage à 0), aucune fenêtre ne sort du coin. Le chien
  posé sur la barre (REQ-011) est le seul chien affiché ; il suit la pose de la board (laisse,
  « aboie » quelques secondes à chaque relance, « aboie » si la board est hors ligne).
- La notification accepter/reporter (REQ-004) est inchangée. Si elle a disparu, le menu de
  l'icône et celui du chien proposent `Accepter le rappel` et `Reporter N min` pendant le rappel
  (SPEC-004).

## Conséquences
- REQ-005 perd sa fenêtre de sortie ; ses tests vérifient qu'aucune fenêtre `OsampoSortie`
  n'apparaît et que le chien posé suit la pose de la board.
- `crates/osampo-pc/src/sortie.rs` est supprimé ; SPEC-006 ne décrit plus que le chien posé.
