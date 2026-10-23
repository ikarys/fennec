# Audit — la page tâche, à ses trois moments

Date : 2026-10-23. Périmètre : la page d'une tâche (la colonne de droite,
`#pick`), lue aux trois moments de sa vie — au début, au cours des échanges, à
la fin — pour dire si le design est cohérent, logique, moderne et efficace.
Méthode : lecture de `ui/app.js` (`showDetail`, `card`, `decision`, `tabbed`,
`underway`, `past`) et du CSS dans `ui/index.html`, pas une passe de captures.

## La page, une fois pour toutes

Une page tâche est toujours les mêmes trois parties, dans le même ordre :

1. **L'en-tête** (`reqHead`) : le titre et le statut. Rien d'autre.
2. **Le split**, deux colonnes :
   - **Le principal (le milieu)** : ce dont la tâche est — ce qui a été demandé,
     la conversation, ce qui a été prouvé. Quand il y a une décision à prendre
     (ou que le travail tourne), on le lit par onglets (Summary / Changes /
     Journal / Details) ; sinon la carte s'affiche directement.
   - **L'aside (la droite)** : les boutons de décision (`.decide`) au-dessus du
     coût et de l'isolation (`.facts`). Quand il n'y a rien à décider, l'aside
     garde le coût seul, pour que la barre ne disparaisse jamais.

La règle que cela sert (AGENTS.md, « Mail-client layout ») : les boutons de
décision à droite, au-dessus du coût, le reste derrière des onglets.

## Moment 1 — le début (Backlog, `later`)

- En-tête : titre + « Backlog ».
- Principal : la carte, affichée directement (pas d'onglets — rien n'a été fait
  encore) : ce qui a été demandé (la description), l'état du cadrage (« fennec
  restates it when the model is free… »), ce que fennec a compris, les skills
  qu'il utilisera.
- Aside : la décision (Validate / Start now / Split / Edit / Save / Delete)
  au-dessus du coût (« No figures yet… »).

Cohérent : la décision est à droite, le coût en dessous, rien à lire derrière
des onglets. La carte dit ce qu'est la tâche ; l'aside dit quoi en faire.

## Moment 2 — au cours des échanges

Deux formes, selon que fennec travaille ou demande.

**En cours (`run`)** — en-tête : titre + l'étape et depuis combien de temps ça
tourne. Principal : la carte en direct par onglets (Summary / Journal), les
étapes et le bouton Stop. Aside : le coût, qui défile. Cohérent : on regarde le
travail au milieu, le coût à droite, la seule décision (Stop) là où est le
travail.

**Une question (`blocked`, et de même pour `talk`, `validate`, `rule`)** —
en-tête : titre + « Question ». Principal : la carte, affichée directement
(pas d'onglets) : la conversation, la question, le champ de réponse, les
boutons. Aside : le coût seul — une conversation garde ses boutons sous ce qui
a été dit, donc il n'y a pas de panneau de décision à droite.

C'est le moment où l'audit a trouvé le trou. La carte se lisait, de haut en bas :

```
conversation
question
champ de réponse
[stats — « Details »]
boutons (Answer, Drop, Journal)
```

La question est au milieu (par choix : une conversation garde ses boutons sous
ce qui a été dit), mais **les stats étaient entre la question et les boutons**.
Pour répondre, l'humain lisait la question, faisait défiler au-delà du tableau
de coût, et ne trouvait les boutons qu'après. La décision — la seule raison
d'être sur la page — était la dernière chose, sous les chiffres.

## Moment 3 — la fin

**Prêt (`ready`)** — en-tête : titre + « Ready ». Principal : la carte par
onglets (Summary / Changes / Journal / Details) : ce qui a été prouvé, ce qui
ne l'a pas été, où est le travail. Aside : la décision (Commit / Open the page
/ See the changes / Open the code / Correct / Pass / Undo) au-dessus du coût.
Les boutons sont déjà en haut de la carte, donc la décision n'attend pas les
chiffres.

**Échoué (`failed`)** — en-tête : titre + « Failed ». Principal : la carte par
onglets : les étapes, pourquoi c'a échoué, la conversation. Aside : la décision
(Back to the Backlog / Retry) au-dessus du coût.

**Terminé (`past`)** — en-tête : titre + « Done ». Principal : la carte du
passé par onglets. Aside : le coût seul — plus rien à décider.

Cohérent : la décision est à droite, le coût en dessous, la lecture derrière
des onglets. La carte prête est le modèle — décision d'abord, chiffres après.

## Verdict

| | |
|---|---|
| Cohérent | Oui — un en-tête, un split, le coût toujours à droite, la décision toujours au-dessus du coût. La conversation est la seule exception délibérée (boutons sous ce qui a été dit), et elle est dite dans le code. |
| Logique | Oui — ce qui a été demandé d'abord, la conversation ensuite, la décision là où elle est nécessaire ; le coût est toujours à part, à droite. |
| Moderne | Oui — onglets, icônes, sombre et clair, une mise en page type client de messagerie ; pas de poids mort. |
| Efficace | **Non, à la question.** Les boutons étaient sous les stats, donc répondre à une question voulait dire faire défiler au-delà du tableau de coût. |

Le trou était un : à la question, l'ordre dans la carte était
question → stats → boutons. La carte prête le faisait déjà bien (boutons en
haut, chiffres après) ; la conversation non.

## La correction

Dans `card()`, les cartes non « prêtes » mettent désormais les boutons et
l'erreur au-dessus des stats, comme la carte prête le faisait déjà :

```
conversation
question
champ de réponse
boutons (Answer, Drop, Journal)
[stats — « Details »]
```

Une ligne, au point de passage commun de toutes les cartes non « prêtes »,
pour que la question, la discussion, la validation et la règle lisent toutes
le même : la décision d'abord, les chiffres après. La carte prête n'est pas
touchée (ses boutons étaient déjà en haut), et les cartes dont les boutons
partent dans l'aside de droite (later, ready, failed) ne le sont pas non plus —
seul l'ordre dans la carte a changé, et pour elles les boutons quittent la
carte de toute façon.

Test : `ui/card.test.js` construit une carte « question » et vérifie que les
boutons précèdent les stats.
