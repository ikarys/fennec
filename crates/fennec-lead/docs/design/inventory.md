# Inventaire : fennec-harness + fennec-os face aux scénarios de lead

Date : 2026-10-15. Base : [lead-scenarios.md](lead-scenarios.md). Lecture des en-têtes de modules et des tailles, pas du code ligne à ligne.

## Décision

**Nouveau repo, fin, qui dépend de fennec-os.** Pas de reprise de fennec-harness.

- fennec-harness = 13 000 lignes Rust + 2 700 lignes JS construites autour du modèle VISION (tâche signée, négociation, permissions, limites, successeur, SQLite, SSE, cockpit). Les scénarios suppriment ce vocabulaire côté humain : le garder coûte plus cher que de le réécrire.
- fennec-os = 3 500 lignes, 2 dépendances (`serde_json`, `ureq`), synchrone, déjà testé en live. Il couvre déjà l'exécution (monde, solve, règles, revue). C'est la bonne fondation.
- Le nouveau repo ne contient que ce qui manque : la couche « lead » (demande → terminé quand → preuve → vue de retour). Quelques fichiers de harness y sont portés.

## Par module

Légende scénarios : 1 lancer et partir · 2 revenir · 3 preuve · 4 règle · 5 blocage.

### fennec-os (devient la lib)

| Module | Lignes | Sert | Verdict |
|---|---|---|---|
| world | 286 | 1, 5 (projet intact) | Garder |
| solve | 446 | 1, 3 | Garder ; ajouter l'étape preuve après les commandes |
| agent | 293 | 1 | Garder ; ajouter détection de boucle / timeout outil (5) |
| provider | 97 | 1 | Garder |
| spec | 300 | 1 (terminé quand) | Garder, réorienter : critères produit d'abord, tests ensuite |
| rules | 173 | 4 | Garder ; ajouter niveau utilisateur (`~/.config/fennec/rules`) |
| review | 165 | 2, 4 | Garder ; devient la source de « À moi » |
| grant | 198 | — | Garder en lib, invisible côté humain |
| recipe | 84 | — | Garder, non prioritaire |
| sleep | 179 | 4 (partiel) | Garder, non prioritaire |
| schedule | 76 | 5 (modèle local partagé) | Garder |
| main (CLI) | 304 | — | Garder comme outil de debug de la lib, jamais comme surface humaine |

### fennec-harness

| Module | Lignes | Sert | Verdict |
|---|---|---|---|
| server/product (lancer le produit) | 297 | 3 | **Porter** : cœur de la preuve |
| checks/rules (6 types de règles) | 281 | 4 | **Porter** les types manquants dans `fennec-os/rules`, sans `harness-core` |
| checks/exec (sortie bornée) | 60 | 3 | **Porter** |
| agent/protocol (JSON tolérant, modèle 27B) | 81 | 5 | **Porter** dans fennec-os |
| agent/workspace (worktree par run) | 402 | 1 | Supprimer : doublon de `world` |
| agent/shell + permissions | 938 | — | Supprimer : le bac à sable de fennec-os remplace le filtrage préventif |
| agent/runner, tools, provider | 1 161 | 1 | Supprimer : doublon de `solve`/`agent`/`provider` |
| agent/negotiate | 461 | 1 | Supprimer ; idée reprise dans `spec` |
| agent/successor, parking | 377 | 5 | Supprimer ; remplacé par « question dans À moi » |
| agent/produced, server/inspect | 389 | 2 | Réécrire petit : « voir le résultat » = une commande |
| agent/review | 116 | 2 | Supprimer : doublon de `fennec-os/review` |
| core/model, limits, paths | 1 294 | — | Supprimer : vocabulaire VISION |
| core/store (SQLite, migrations, stats) | 1 704 | 2 | Supprimer ; état en fichiers comme fennec-os |
| core/events (bus SSE) | 69 | 2 | **Porter** : la vue se met à jour seule |
| server/service, config, folders | 440 | 2 | **Porter** (lancement du serveur, choix de projet en quelques clics) |
| server/api, main, settings | 2 100 | 2 | Réécrire : API minuscule taillée pour la vue |
| tests (e2e, store) | 2 303 | — | Supprimer avec le code testé |
| ui (17 fichiers JS) | 2 701 | 1, 2, 3 | Réécrire de zéro, mêmes choix techniques (JS sans build, servi par le binaire) |

Bilan : ~1 200 lignes portées sur ~15 700, soit moins de 10 %.

## Contenu du nouveau repo (première version)

- Un binaire `fennec` qui sert une UI web locale ; dépendance `fennec-os` par chemin. Toute interaction humaine passe par l'UI.
- Écran unique au départ : la vue À moi / Prêt / En cours / Échoué, tous projets (2), avec un champ « nouvelle demande ».
- Demande → fennec propose « terminé quand » dans l'UI → une validation → il tourne seul (1).
- Après solve : lance le produit, capture sortie et capture d'écran, affiche `prêt + preuve` ou `non vérifié : X` sur la carte (3).
- Pas de base de données : état en fichiers, comme fennec-os.

## Risques

- ⚡ Quatrième dépôt fennec : le motif « on recommence » se répète. Garde-fou : le nouveau repo reste une couche mince ; toute logique d'exécution va dans fennec-os.
- ⚡ La preuve « comme moi » sur une UI (clic, drag) est le point dur, et aucun des deux repos ne la sait faire aujourd'hui. `product.rs` ne fait que lancer ; capture et interaction sont à écrire.
- ⚡ Perte des tests e2e de harness : ils protégeaient des comportements qu'on supprime, donc perte acceptable.
- ✅ fennec-os comme fondation : petit, testé, déjà validé en live sur le modèle local.
- ⚡ L'UI de harness a échoué parce qu'elle exposait les concepts internes. Garde-fou : six mots à l'écran (projet, demande, terminé quand, preuve, question, règle), rien d'autre.
- ✅ Première tranche = un seul écran qui montre une demande aller jusqu'à « prêt + preuve » : scénarios 1, 2 et 3 d'un coup, testables en vrai.
