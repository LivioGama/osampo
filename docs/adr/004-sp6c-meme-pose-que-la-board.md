# ADR-004 : le chien du PC prend la même pose que la board quand il réclame

## Statut
Accepté (l'auteur, 2026-11-22). Précise REQ-001 et REQ-011.

## Contexte
À l'essai réel, l'auteur voit le chien du PC aboyer alors que le chiot de la board ne fait que lever
la patte. La board réclame avec une raison (`rappel`, `promenade`, `claude`) et montre la patte
levée quand la raison est `claude` (Claude attend une réponse). Elle envoie cette raison au PC
(`"raison"` de la ligne d'état, SPEC-001), mais SPEC-002 traduit toute réclamation en « aboie ».

## Décision
Le chien du PC (icône de la zone de notification et chien posé sur la barre) prend la même pose
que la board : quand elle réclame pour `claude`, il lève la patte ; pour une sortie, un rappel ou
la promenade, il aboie. Une icône « patte » 22/24 px s'ajoute aux quatre existantes.

## Conséquences
- SPEC-002 (choix de l'icône), SPEC-004 et SPEC-011 (animations) à corriger ; nouvelle icône et
  nouvelle animation « patte » côté PC.
- La notification de rappel de sortie (REQ-004) ne change pas : elle ne concerne pas la raison `claude`.
