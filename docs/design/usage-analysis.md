# Analyse d'usage — interventions humaines (OpenFox + Claude Code)

Date : 2026-10-15. Source : sessions locales, lecture seule.

- OpenFox : 52 sessions, 295 messages humains, 13/09 → 02/10, 9 projets (`~/.local/share/openfox/sessions.db`).
- Claude Code : 73 sessions, 426 messages humains (+16 interruptions, 5 refus d'outil), 14 dossiers (`~/.claude/projects/*/*.jsonl`, session courante exclue).

Classification manuelle, une passe, parts approximatives (±3 pts).

## Répartition

| Intervention | OpenFox | Claude Code | Lecture pour fennec |
|---|---|---|---|
| Cadrer, discuter, arbitrer (dont R&D) | 27 % | 30 % | Humain, synchrone : la vraie valeur |
| Gestes répétitifs | 21 % | 18 % | OF : bouton « Add the acceptance criteria… » (~38×), continue/retry. CC : `commit`, `commit et push`, `go`, `continue`. À supprimer |
| Rejet d'un résultat annoncé fini | 10 % | 7 % | « tu mens », « belle régression », « RIEN DANS CETTE UI », « rarement une UI aussi moche ». Vérification réelle absente |
| Outil cassé / bloqué | 10 % | 3 % | OF + modèle local : timeouts, commandes bloquées. Runtime à fiabiliser |
| Où en est-on / quoi tester | 8 % | 7 % | « tu as fini ou tu attends ? », « comment je le lance ? », « 10min tu fou quoi ?! ». Compte rendu à pousser |
| Goût, finition, contenu | 8 % | 10 % | UI, slides, README. Humain |
| Pourquoi tu fais ça | 5 % | 5 % | Surveillance de dérive : minoritaire |
| Préférences durables | 5 % | 6 % | Peu nombreuses, mais les plus coûteuses (voir plus bas) |
| Intervention sur le PC (mode 3) | 0 % | 11 % | VPN, Omarchy, terminal. Usage distinct, hors fennec |

## Constats fusionnés

1. **Délégation demandée explicitement**, dans les deux outils : « code tout sans me demander sauf décision », « t'arrête pas tant que t'as pas fini », « donne-moi juste un rapport à la fin ». Au moins 5 occurrences. Le mode lead est déjà ton usage voulu ; les outils ne le tiennent pas.
2. **Les préférences durables sont peu nombreuses mais répétées entre outils et projets** : identité git (Ikarys / bikarys@proton.me, « 50 fois que je dis »), pas de co-signature Claude, ne jamais toucher `develop` (branche + PR), commit sans push par défaut, uv plutôt que `.venv`, réponses en français, sorties courtes, pas de sous-agents, VISION jamais bloquant, pas de TUI pour les agents, tout passe par l'UI. Elles doivent vivre au niveau utilisateur et être lues par **tous** les outils, pas par projet.
3. **La vérification est le trou commun** : les rejets portent surtout sur l'UI et le comportement réel (crash au lancement, menu disparu, drag impossible), pas sur des tests unitaires.
4. **Le git est un rituel manuel** dans Claude Code : la politique commit/push/branche est une préférence stable par projet, donc automatisable.
5. **Le mode 3 existe vraiment** (11 % de l'usage Claude Code, 0 % dans OpenFox) : tu le fais déjà ailleurs, il reste hors du périmètre de fennec.
6. **La casse runtime est 3× plus forte avec OpenFox et le modèle local** : un fennec local-first héritera du problème si le runtime n'est pas robuste.

## Ce qui change par rapport à la seule analyse OpenFox

- Confirmé : « corrections → règles » ne couvre qu'environ 5 % des interventions ; pas besoin d'un plugin de capture.
- Nuancé : ces 5 % sont la plus forte source d'irritation, et elles traversent les outils → un fichier de règles utilisateur unique et partagé suffit, à écrire à la main.
- Nouveau : la délégation sans interruption + un rapport final sont formulés par toi-même, ce qui valide le scénario « je lance et je pars ».
