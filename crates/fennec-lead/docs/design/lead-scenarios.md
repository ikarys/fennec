# Scénarios de lead

Date : 2026-10-15. Base : [usage-analysis.md](usage-analysis.md) (721 interventions, OpenFox + Claude Code).
Rôle : je dirige une équipe sur plusieurs projets. Je ne surveille pas, je ne gère pas de tickets.
Hors périmètre : intervenir sur le PC (mode 3), la réflexion pure sans livrable (mode 2).

## 1. Je lance et je pars

**Aujourd'hui** : bouton « Add the acceptance criteria… » à chaque tâche, puis `go`, `continue`, `commit`, `push` (~20 % de mes messages). Je reste devant.

**Avec fennec** :
1. Je décris ce que je veux, en une phrase ou en discutant si c'est flou.
2. Fennec reformule et propose **terminé quand** en termes de produit (« la fenêtre se déplace à la souris », « `capy list` affiche les barres »), pas en termes de code.
3. Je valide ou corrige, une fois.
4. Fennec travaille sans me solliciter, sauf décision réelle. Branche, commits et push suivent les règles du projet ; je ne les demande plus.

**Réussi quand** : entre la validation et le résultat, zéro message de ma part n'est nécessaire.

## 2. Je reviens

**Aujourd'hui** : « on en est où ? », « tu as fini ou tu attends ? », « comment je le lance ? », « que dois-je tester ? » (~8 %).

**Avec fennec** : une seule vue, tous projets confondus, triée par ce qui a besoin de moi :
- **À moi** : questions et décisions en attente, chacune avec le contexte pour y répondre sans relire l'historique.
- **Prêt** : terminé et prouvé, avec la commande unique pour voir le résultat moi-même.
- **En cours** : ce qui avance, sans détail.
- **Échoué** : pourquoi, en une ligne, et ce que fennec propose.

**Réussi quand** : je sais quoi faire en moins d'une minute après mon retour, sans poser de question.

## 3. Le résultat est faux, fennec l'a vu avant moi

**Aujourd'hui** : « tu mens » + traceback, « belle régression », « RIEN DANS CETTE UI », « aucune fenêtre ne peut être drag » (~8 %). L'agent avait annoncé « vérifié ».

**Avec fennec** : avant de dire « prêt », fennec lance vraiment le produit, l'utilise comme moi (clic, commande, capture) et joint la **preuve**. Ce qu'il ne peut pas prouver est écrit « non vérifié : X », jamais passé sous silence. Il vérifie aussi que ce qui marchait avant marche encore.

**Réussi quand** : je ne découvre plus de régression ou de fonction cassée qu'un essai de 30 secondes aurait montrée.

## 4. Je le dis une fois

**Aujourd'hui** : identité git, pas de co-signature Claude, pas toucher `develop`, uv, sorties courtes… répétés d'outil en outil (« 50 fois que je dis »). Peu nombreux (~5 %), mais les plus irritants.

**Avec fennec** : quand je corrige quelque chose qui vaut au-delà de la tâche, fennec propose d'en faire une **règle**, pour moi (tous projets) ou pour ce projet. Je confirme d'un mot. La règle s'applique ensuite partout, y compris aux autres outils qui lisent le même fichier. Je vois quand une règle a servi.

**Réussi quand** : aucune préférence n'est répétée deux fois.

## 5. Ça bloque, et je ne suis pas là

**Aujourd'hui** : commandes bloquées 20 min, « 10min tu fou quoi ?! », je colle moi-même la sortie des commandes (~10 % avec OpenFox et le modèle local).

**Avec fennec** : fennec détecte qu'il tourne en rond ou qu'un outil ne répond pas, coupe, essaie autrement ou abandonne proprement en laissant le projet intact. Il ne me notifie que s'il a besoin de moi, avec une question précise.

**Réussi quand** : un blocage ne me coûte jamais plus qu'une question dans « À moi ».

## Vocabulaire qui en découle

Six mots, rien d'autre côté humain : **projet**, **demande**, **terminé quand**, **preuve**, **question**, **règle**.
Absents volontairement : tâche signée, run, permissions, grant, limites globales, checkpoints. Ils restent internes (fennec-os) ou disparaissent.
