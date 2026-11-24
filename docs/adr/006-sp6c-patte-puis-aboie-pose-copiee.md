# ADR-006 : patte puis aboiement sur la board, pose copiée par le PC

## Statut
Accepté (l'auteur, 2026-11-22). Étend ADR-004.

## Contexte
Quand le chiot réclame, la board choisit sa pose selon la raison (`controller.cpp`, `anim_name`) :
patte levée quand Claude attend, laisse dans la gueule pour la jauge vide et les rappels sortie,
sport et dodo, aboie pour le rappel repas. ADR-004 ne faisait copier au PC que la patte ; le PC
aboyait encore quand la board tient la laisse. L'auteur veut un seul chien cohérent (option A), et
une escalade : « le chien lève la patte, puis aboie si ignoré trop longtemps ».

## Décision
1. Le chien du PC (icône et chien posé sur la barre) copie toujours la pose de réclamation de la
   board : patte, laisse ou aboie. La pose « réclame avec la laisse » est distincte de « en
   promenade ».
2. Sur la board, une réclamation « Claude attend » commence patte levée, puis passe à l'aboiement
   si elle n'est pas traitée après un délai réglable.

Choix de Claude, à vérifier : l'escalade ne concerne que la raison `claude` (la jauge vide et les
rappels ont déjà leurs relances qui aboient) ; le délai est une clé de `osampo.toml`, 2 min par
défaut, avec son aide (REQ-010).

## Conséquences
- Firmware modifié (logique du chiot) : la board est à reflasher.
- La board doit envoyer au PC sa pose courante (ou de quoi la recalculer sans ambiguïté).
- Nouvelle icône et nouvelle animation PC « réclame avec la laisse ».
