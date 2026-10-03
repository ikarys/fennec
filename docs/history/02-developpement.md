# R&D : développement de fennec-os

Transcription extraite de la session Claude Code : messages de l'utilisateur et réponses texte de Claude, sans les appels d'outils ni leurs sorties. Horodatages UTC.
L'état du projet est dans `docs/roadmap.md`.

## Utilisateur — 2026-10-02 13:13

ok, creer un folder/repo et on conmmence dedans ! GO

## Claude — 2026-10-02 13:14

Je crée le dépôt et j'attaque P0 (le monde). Je vérifie d'abord l'outillage.

Les trois tests passent et clippy ne signale rien. Mais `main.rs` n'a pas été écrit : `cargo init` l'avait déjà créé avec le « hello world » par défaut. Je le remplace.

Le dépôt `~/workspace/projects/fennec-os` est créé et P0 est fait, sauf les snapshots btrfs. Les 3 tests d'intégration passent contre un vrai bac à sable bubblewrap, clippy ne signale rien, et j'ai testé la CLI de bout en bout. Je n'ai pas commité.

**Ce qu'il contient :**
- `src/world.rs` : la primitive monde. Un monde est un worktree git sur une branche `world/<id>`, créé à partir d'un projet ou d'un autre monde. Opérations : `fork`, `diff`, `commit` (point de sauvegarde fait par le harness), `rollback`, `drop`, `list`.
- `src/main.rs` : la commande `fennec-os`.
- `tests/world.rs` : les tests d'intégration.
- `AGENTS.md`, `CLAUDE.md` en symlink, et `docs/roadmap.md`.
- Aucune dépendance externe : bibliothèque std seulement.

**Ce que garantit le bac à sable** (tout est couvert par les tests) :
- les écritures hors du monde n'atteignent jamais le système hôte ;
- le contenu de `$HOME` est caché ; seules les toolchains restent visibles en lecture seule, et `cargo` fonctionne bien à l'intérieur ;
- `/tmp` et `/run` sont privés ;
- le réseau est coupé (seulement l'interface `lo`) ;
- l'agent ne peut pas commiter, car le stockage git du projet est en lecture seule ;
- `rollback` restaure l'état exact du dernier point de sauvegarde, et un fork part de ce point.

**Faille trouvée et corrigée pendant le travail :** sans masquer `/run`, le bus D-Bus de session (`/run/user/1000/bus`) restait joignable depuis le bac à sable, ce qui permettait d'en sortir avec `systemd-run --user`. Couper le réseau ne le bloquait pas. Un test le couvre maintenant.

**Non fait ou non vérifié :**
- les snapshots btrfs pour les projets sans git ;
- l'intégration d'un monde dans la branche du projet (prévue en P1) ;
- aucune limite de CPU ou de mémoire : un agent peut encore saturer la machine, il faudra des cgroups ;
- aucun filtre seccomp ;
- seulement Linux.

Prochaine étape, P1 : une boucle d'agent dans un monde, branchée sur ton endpoint local compatible OpenAI, avec en sortie le diff du monde plus `verify`, et la réutilisation de `harness-checks`.

## Utilisateur — 2026-10-02 13:17

developpe le projet, je te fais confiance, tarretet pas sauf si je dois prendre une decision

## Claude — 2026-10-02 13:17

Je vais jusqu'au bout de P1, puis P2. Je commence par regarder ce qui est réutilisable dans fennec-harness (le provider et les checks).

Le provider configuré dans fennec-harness pointe sur `http://192.168.1.174:8000/v1` (qwen3.8-27b). Je vérifie qu'il répond, et que cargo peut récupérer des crates.

P0 sandbox validé pour cargo (build offline OK en ro-bind, overlay inutile pour l'instant). J'écris P1 : provider, boucle agent, verify, solve/land.

Écriture de `solve.rs` : vérif déterministe, sélection parmi N mondes, landing.

7/7 verts, clippy propre. Smoke test live contre qwen3.8-27b, 3 tentatives en parallèle sur un vrai projet Rust.

P1+P2 bout en bout validés en live (3 mondes, 5,4 s, land OK). Mise à jour docs.

P3 (queue/promotion) demande des choix de design ; avant ça, un trou bloquant pour l'usage multi-agents : `land` échoue si le projet a bougé. J'implémente rebase + re-verify au landing.

Escape confirmée (hook écrit dans un autre repo). Correction de `world.rs` :

12/12 verts. Repro manuelle de l'escape sur le nouveau binaire :

Fix confirmé. Avant P3, je relis ce que les notes disent de "correction → règle" pour ne pas inventer la sémantique.

P3 en cours : module `rules.rs` écrit (glob maison, pas de dep), branchement dans `solve` puis file de revue.

Règles branchées, 13/13 verts. File de revue maintenant.

15/15, clippy propre. Docs, puis P4 v0.

17/17, clippy propre. Smoke live P3/P4 : règle projet `paired_change src→tests` commitée, tâche qui l'exige implicitement.

Règle respectée par les 3 agents. Rejeté avec correction ; re-solve pour voir si l'agenda la transmet :

Correction transmise et appliquée (`""`, `IIII` testés). Land + docs P4.

P0→P4 v0 sont faits et vérifiés en live. Je m'arrête ici parce que P5 dépend d'un choix de design qui te revient. Rien n'est commité.

**Fait depuis P0**
- **P1 + P2, `solve`** : N mondes en parallèle (modèles × températures). Dans chaque monde, un agent dont tous les outils tournent dans le sandbox. Le verdict vient uniquement des exit codes ; on garde le plus petit diff qui passe et on jette les autres.
- **`land`** : fast-forward si le projet n'a pas bougé. Sinon, rebase du monde puis nouvelle vérification avant d'intégrer.
- **P3, règles projet** : `forbidden_path`, `paired_change`, `standing_command`, en `block` ou `warn`. Elles sont lues dans `.fennec-os/rules.json` au HEAD du projet, et un run n'a jamais le droit de modifier ce fichier. Commandes : `queue`, `show`, `reject <run> [correction]` (une correction donnée deux fois est signalée), `promote`.
- **P4 v0, sans second modèle** : un agenda construit à partir de l'historique des runs (landés, corrections, échecs) est injecté dans le prompt. Le contexte est borné par élision des vieux outputs.
- **Faille de sandbox, corrigée** : le harness lisait le fichier `.git` du monde, que l'agent peut écrire, pour choisir quoi monter en écriture. Un agent pouvait donc obtenir le `.git` d'un autre repo en écriture. Je l'ai reproduit : un hook a été planté dans un autre repo. Désormais les chemins git viennent des métadonnées du harness, le dossier admin est en lecture seule et les hooks sont toujours désactivés. Un test de non-régression couvre le cas.
- **Dépendances ajoutées** : `ureq` (pas de client HTTP dans la stdlib) et `serde_json`.

**Vérifié**
- 17 tests passent et clippy ne signale rien.
- En live avec qwen3.8-27b :
  - `roman` : 3/3 tentatives réussies en 5,4 s au total.
  - `from_roman` : les 3 tentatives ont respecté la règle `paired_change` du projet.
  - J'ai rejeté ce run avec « test invalid inputs like IIII and empty string ». Le run suivant a reçu la correction dans son prompt et a testé `""` et `IIII`.

**Pas fait**
- Snapshots btrfs pour les projets sans git.
- Limites CPU/mémoire (cgroups) et seccomp.
- Builds qui ont besoin du réseau : les dépendances doivent déjà être dans le cache local.
- Deux corrections ne comptent comme « la même » que si le texte est identique après normalisation ; une paraphrase est comptée à part.

**Ta décision pour P5** : P5 fait tourner chaque nuit un job qui transforme les runs gagnants en « compétences » réutilisables. Reste à décider ce qu'est concrètement une compétence :
- **A. Note de savoir-faire** : un modèle extrait la nuit 1 à 3 lignes par run gagnant (par exemple « tests : `cargo test --offline` »), tu les promeus, et elles sont injectées comme l'agenda. Coût faible, ça s'appuie sur l'existant ✅. ⚡ Ça reste du texte, donc pas de gain de vitesse.
- **B. Recette rejouable** : la suite de commandes qui a réussi devient un script exécuté par le harness, sans modèle. C'est le vrai gain (compilation JIT des trajectoires). ⚡ Ça généralise mal : il faut la paramétrer et la vérifier à chaque usage.
- **C. Trajectoire exemple** : une trajectoire gagnante compressée, retrouvée en comparant les objectifs et injectée comme exemple. ⚡ Ça coûte du contexte, et retrouver la bonne trajectoire demande un index.

Je recommande A maintenant et B en P7 (recherche). A ou autre chose ? Dis-moi aussi si je commite tout ça.

## Utilisateur — 2026-10-02 13:34

ok pour ta reco, continue

## Claude — 2026-10-02 13:36

P5 v0 (sleep → candidats → keep/dismiss) compile. Test :

18/18, clippy propre. Smoke live : `sleep` sur les 2 runs landés du projet `live`.

P5 v0 validé live (3 candidats en 8 s ; 2 utiles gardés, 1 redondant avec les règles écarté). Docs, puis P6.

P0 à P6 sont faits. P7 demande un choix de direction, d'où l'arrêt.

**P6 v0 (scheduler) :**
- `solve` garde un verrou partagé sur `<store>/interactive.lock` pendant tout le run.
- `sleep` prend le verrou exclusif avant chaque distillation, donc il attend tant qu'un `solve` tourne. Ce sont des `flock` : ils disparaissent avec le processus, pas de verrou orphelin.
- Code dans `src/schedule.rs`, branché dans `solve.rs:67` et `sleep.rs:74`.
- Ce que ça ne couvre pas : les autres clients du serveur modèle (`/metrics` répond 404, leur charge est invisible), ton activité au clavier, et une distillation déjà envoyée ne s'interrompt pas.

Vérifié : 19 tests verts, dont un nouveau test qui fait attendre `sleep` derrière deux `solve` concurrents ; clippy propre. `docs/roadmap.md` et `AGENTS.md` sont à jour. Ce comportement n'a pas encore tourné contre le vrai modèle.

**P7 :** c'est cinq paris de recherche, avec un coût très différent selon le pari :

| Piste | Faisable ici sans toi | Coût / risque |
|---|---|---|
| Recettes rejouables (option B, reportée) | oui | ✅ moyen, s'appuie sur les trajectoires déjà stockées |
| Exécution spéculative des outils | oui | ⚡ gain incertain : le modèle domine la latence, pas les outils |
| LoRA nocturne | non | ⚡ occupe le GPU partagé, il faut une stack d'entraînement |
| Compilateur d'intention | partiel | ⚡ spec floue, risque de construire à vide |
| Cluster maison | non | décision matérielle |

Ma reco : les recettes rejouables seulement, puis on mesure le gain avant d'ouvrir d'autres pistes.

Rien n'est commité, le repo n'a toujours aucun commit.

Je pars sur les recettes rejouables, ou tu préfères une autre piste ?

## Utilisateur — 2026-10-02 15:45

commit, push et go avec ta reco

## Claude — 2026-10-02 15:46

Commit + push faits (repo privé `ikarys/fennec-os`). Je démarre P7 : recettes rejouables, en lisant `attempt_inner` et `agent::run` avant de coder.

Recettes branchées, mon nouveau test passe, mais l'ancien test `sleep` casse : j'investigue.

Mesure live : j'annule `from_roman` dans le projet de test, puis je relance la même tâche pour comparer le replay au modèle (71 s la dernière fois).

Tout est commité et poussé sur `ikarys/fennec-os`, un repo GitHub **privé** que j'ai créé parce qu'il n'y avait pas de remote. Il y a deux commits : `cba702a` (P0 à P6) et `5db4659` (recettes).

**Recettes rejouables (P7) :**
- Quand le dernier run sur le même objectif a été landé dans ce projet, ses appels d'outils sont rejoués dans un nouveau world, avant tout appel au modèle. Ils sont jugés par les mêmes règles.
- Si le replay passe, aucun modèle n'est appelé. S'il échoue, les modèles tournent normalement.
- Si le dernier run sur cet objectif a été rejeté ou a échoué, la recette n'est plus utilisée. Une trajectoire dont un `write_file` a été élidé par le bornage du contexte n'est pas rejouée.
- Code dans `src/recipe.rs` : le replay se fait passer pour un modèle, donc la boucle de l'agent n'a pas changé.

**Mesure live :**
- J'ai annulé `from_roman` dans le projet de test, puis relancé la même tâche. La recette a rejoué 7 étapes, `cargo test` est passé (4 tests), en 0,3 s sans appel modèle, contre 71 s avec le modèle.
- C'est le meilleur cas : refaire un travail annulé.

**Limites :**
- ⚡ Il faut le même objectif au mot près, après normalisation.
- ⚡ `write_file` réécrit l'ancien contenu. Le gain vient surtout des tâches faites de commandes (régénérer, bumper, migrer). Sur du code qui a bougé entre-temps, les checks rattrapent l'échec et on retombe sur le modèle, avec un world gâché.
- ✅ Il n'y a aucun risque sur le verdict : c'est le code qui juge, jamais le replay.

**Effet de bord :** un objectif déjà accompli est rejoué en diff vide et passe, sans appeler le modèle. C'est apparu dans un test existant, que j'ai adapté en lui donnant un autre objectif.

Vérifié : 20 tests verts, clippy propre, roadmap et `AGENTS.md` à jour.

**Reste de P7 :**
- Le LoRA nocturne et le cluster maison dépendent de décisions matérielles et du GPU partagé.
- L'exécution spéculative des outils a un gain douteux, car c'est le modèle qui domine la latence.
- Le compilateur d'intention est la suite logique : il ferait correspondre une recette à un objectif reformulé.

Je m'arrête ici comme prévu : on mesure le gain des recettes sur ton usage réel avant d'ouvrir une autre piste.

## Utilisateur — 2026-10-03 05:25

ok go compilateur d'intention. fait important, je n'aime pas les TUI pour les agents, je trouve cela trop restrictif et pas assez interactif.

## Claude — 2026-10-03 05:26

Le module `spec` (le compilateur d'intention) compile. Je branche maintenant la CLI : `spec`, `spec show`, `spec accept`, `spec reject`.

22 tests verts, clippy propre. Passage au test live : je compile une vraie demande sur le projet `roman` avec qwen.

Le serveur modèle (192.168.1.174) ne répond plus, même au ping, donc pas de vérif live possible. Je finalise la doc puis je commite.

J'ai codé le compilateur d'intention v0 et poussé le commit `f705da5`, mais il n'a pas tourné contre le vrai modèle : le serveur 192.168.1.174 ne répond plus du tout, ni HTTP ni ping.

Sur ta remarque : fennec-os n'a pas de TUI, c'est une CLI de commandes. C'est noté en mémoire, et je ne proposerai pas de TUI pour l'interface humaine.

**Ce que fait le compilateur d'intention :**
- `spec <projet> "<demande>"` : un agent écrit uniquement les tests d'acceptation, puis rend un `goal:` reformulé et des lignes `verify:`.
- **Ce que le harness vérifie lui-même :** il y a un goal et au moins une commande, des fichiers ont été écrits, rien sous `.fennec-os` ni dans un chemin interdit, et surtout **les tests échouent maintenant**. Un test qui passe déjà ne teste pas la demande.
- **Ce qui te revient :** tu valides la spec, pas le code. `spec show <id>`, puis `spec accept <id> [--attempts N]` : les tentatives partent du world de la spec, ses tests sont protégés et ses commandes servent de vérification. `spec reject <id> "correction"` : la correction est injectée au prochain compile de la même demande.
- Le test de bout en bout passe : un tricheur qui modifie le test est rejeté, l'honnête gagne et le land amène les tests avec l'implémentation. 22 tests verts, clippy propre.

**Limites :**
- ⚡ « Échoue » veut dire n'importe quel code de sortie non nul : un test qui ne compile pas passe ce contrôle. C'est ta lecture du diff qui protège.
- ⚡ Si l'agent de spec écrit autre chose que des tests, ces fichiers deviennent protégés et bloquent l'implémentation. Ça aussi, ça se voit au diff.
- ✅ Le modèle ne juge toujours rien : le verdict reste au code, et le sens des tests reste à toi.

**Prochaines étapes :**
- Relancer le test live quand le serveur revient.
- Choisir où tu valides une spec : c'est là que l'interactivité compte (lire les tests, demander une modif, poser une question au compilateur). Une page web locale servie par fennec-os qui affiche specs, diffs et file de revue, avec chat par spec, me semble le bon candidat.

Tu veux cette page web, ou une autre surface ?

## Utilisateur — 2026-10-03 06:41

commit tout, code et vision etc ....si pas de repo , créé un repo privé sur github et push

## Claude — 2026-10-03 06:41

Tout est commité et poussé, sur deux repos GitHub privés :

- **fennec-os** (`ikarys/fennec-os`) : rien à faire, le code, `AGENTS.md` et `docs/roadmap.md` étaient déjà poussés, dernier commit `f705da5`. J'avais créé ce repo privé lors du premier push.
- **Vision** : la note `notes/agent-architecture-reflexion.md` était encore non suivie dans `iatraining`. Je l'ai commitée (`c9abb47`) et poussée sur le repo qui existait déjà (`ikarys/iatraining`), donc pas de nouveau repo.

Les deux working trees sont propres.

## Utilisateur — 2026-10-03 06:49

ya pas l'historique de notre conversaion sur la R&D dans le repos, mets ca dans un ou plusieurs MD sur le disque, commit et push

