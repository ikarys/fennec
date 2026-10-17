// The single view. Polls the server; a page left open stays current.
// A card shows only what the decision needs; the rest waits behind "Détails".

const $ = (id) => document.getElementById(id);
const el = (tag, props = {}, ...children) => {
  const node = Object.assign(document.createElement(tag), props);
  node.append(...children.filter((c) => c !== null && c !== undefined && c !== false));
  return node;
};

async function api(path, body) {
  const res = await fetch(path, body === undefined ? {} : {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (!res.ok) throw new Error(data.error || res.statusText);
  return data;
}

// A button that calls the server and shows its error next to the card's actions.
function action(label, path, body, { primary = false, quiet = false } = {}) {
  const button = el("button", { type: "button", textContent: label, className: primary ? "primary" : quiet ? "quiet" : "" });
  button.onclick = async () => {
    const error = button.closest(".card, .past-actions").querySelector(".error");
    error.textContent = "";
    try {
      await api(path, typeof body === "function" ? body() : body);
      refresh(true);
    } catch (e) {
      error.textContent = e.message;
    }
  };
  return button;
}

// Giving up is one more click than doing: the first click only asks.
function dropButton(id) {
  const button = el("button", { type: "button", className: "quiet", textContent: "Abandonner" });
  button.onclick = () => {
    const sure = action("Confirmer l'abandon", "/api/drop", { id }, { quiet: true });
    sure.style.color = "var(--bad)";
    button.replaceWith(sure);
  };
  return button;
}

const TAGS = {
  talk: ["Discussion", "var(--accent)"],
  validate: ["À valider", "var(--accent)"],
  blocked: ["Question", "var(--accent)"],
  rule: ["Règle ?", "var(--accent)"],
  ready: ["Prêt", "var(--ok)"],
  failed: ["Échoué", "var(--bad)"],
};

function focus(label, text) {
  return el("div", { className: "focus" }, el("span", { className: "label", textContent: label }), ...rich(text));
}

// `command` spans as code, the rest as plain text: models write Markdown.
function rich(text) {
  return (text ?? "").split("`").map((part, i) => (i % 2 ? el("code", { textContent: part }) : part));
}

function firstLine(text) {
  return (text ?? "").split("\n").find((l) => l.trim()) ?? "";
}

// fennec-os reports rule breaks in its own words; the card says what happened.
// The raw reason stays behind "Détails".
// The model server's errors (ureq's wording), in the human's words.
function unreachable(line) {
  // Anchored: a test's output that mentions a timeout is not the server's.
  const m = line.match(/(?:^|\()(timeout|http status|io|host not found|connection failed|bad uri)(?::? ?([^\s:]*))/);
  if (!m) return null;
  const model = line.match(/model '([^']+)' not found/);
  if (model) return `le serveur ne sert pas le modèle « ${model[1]} » (choisi dans Équipe ou dans les Réglages)`;
  if (m[1] === "timeout") return "le serveur ne répond pas à temps";
  if (m[1] === "http status") return m[2] === "401" || m[2] === "403" ? "le serveur refuse la clé"
    : m[2] === "404" ? "le serveur ne connaît pas cette adresse (elle finit souvent par /v1)"
    : `le serveur répond l'erreur ${m[2]}`;
  if (m[1] === "host not found") return "cette adresse n'existe pas";
  if (m[1] === "bad uri") return "cette adresse n'est pas valide";
  return "rien ne répond à cette adresse";
}

function explain(reason) {
  const line = firstLine(reason);
  let m = line.match(/^rule task\.protect violated: changed (.+)$/);
  if (m) return `fennec a modifié un test validé (${m[1]}), ce qui lui est interdit`;
  m = line.match(/^rule \S+ violated: changed (\S+) but nothing under (\S+)$/);
  if (m) return `${m[1]} a changé sans changement dans ${m[2]}, comme les règles du projet l'exigent`;
  m = line.match(/^no model: .* and (\S+)\/models did not answer/);
  const down = unreachable(line);
  if (m) return `fennec ne joint aucun modèle à ${m[1]}${down ? ` (${down})` : ""} : vérifie les Réglages, puis réessaie`;
  if (down) return `fennec ne joint pas son modèle : ${down}. Vérifie les Réglages, puis réessaie`;
  m = line.match(/^rule \S+ violated: changed (.+)$/);
  if (m) return `fennec a modifié ${m[1]}, que les règles du projet protègent`;
  return line;
}

function shots(item) {
  const row = el("div", { className: "shots" });
  for (const s of item.shots ?? []) {
    if (s.missing) continue; // said in "non vérifié"
    if (!s.file) {
      row.append(el("div", { className: "bad", textContent: `Page ${s.url} : ${firstLine(s.reason)}` }));
      continue;
    }
    const capture = (file, label) => {
      const link = el("a", { href: `/shots/${file}`, target: "_blank", className: "shot" });
      link.append(el("img", { src: `/shots/${file}`, alt: `${label} — ${s.url}` }), `${label} — ${s.url}`);
      return link;
    };
    if (s.before) {
      row.append(capture(s.before, "Avant"),
        el("div", { className: "steps", textContent: "↓ Fait par fennec : " + (s.steps ?? []).join(" → ") }),
        capture(s.file, "Après"));
    } else {
      row.append(capture(s.file, "Capture"));
    }
  }
  return row.children.length ? row : null;
}

// Everything that explains but does not decide.
function details(item) {
  const box = el("details", {}, el("summary", { textContent: "Détails" }));
  box.dataset.key = item.id;
  if (item.title?.length > 200) box.append(el("p", { textContent: item.title }));
  if (item.reason) box.append(el("pre", { textContent: item.reason }));
  if (item.land_error) box.append(el("pre", { textContent: item.land_error }));
  if (item.land?.message) box.append(el("p", { textContent: `Message du commit : ${item.land.message}` }));
  if (item.team?.length) box.append(el("p", { textContent: `Équipe : ${item.team.join(", ")}` }));
  for (const rule of item.rules ?? []) box.append(el("p", { textContent: `Règle suivie : ${rule}` }));
  if (item.checks?.length) {
    box.append(el("p", { textContent: "Tests :" }));
    for (const c of item.checks) box.append(el("code", { textContent: c }));
  }
  for (const t of item.tries ?? []) {
    box.append(el("code", { textContent: `$ ${t.command}  → ${t.exit}` }), el("pre", { textContent: t.output }));
  }
  for (const t of item.role_checks ?? []) {
    box.append(el("code", { textContent: `${t.role} : $ ${t.command}  → ${t.missing ? "pas installé" : t.exit}` }),
      el("pre", { textContent: t.output }));
  }
  return box.children.length > 1 ? box : null;
}

// What fennec did, in its words, and where: the work itself, not just its verdict.
function work(item) {
  const nodes = [];
  if (item.summary) nodes.push(focus("Ce que fennec a fait", item.summary));
  if (item.files?.length) {
    nodes.push(el("div", { className: "files" }, "Fichiers : ",
      ...item.files.flatMap((f, i) => [i ? ", " : "", el("code", { textContent: f })])));
  }
  return nodes;
}

// Something read on demand: one button that opens it under the card, and closes it.
function opener(label, hide, where, load) {
  const button = el("button", { type: "button", textContent: label });
  let box = null;
  button.onclick = async () => {
    if (box) {
      box.remove();
      box = null;
      button.textContent = label;
      return;
    }
    box = el("div", { className: "changes" }, el("div", { className: "muted", textContent: "Je charge…" }));
    where().append(box);
    button.textContent = hide;
    try {
      box.replaceChildren(...await load());
    } catch (e) {
      box.replaceChildren(el("div", { className: "bad", textContent: e.message }));
    }
  };
  return button;
}

// The exact change.
function changesButton(item, where) {
  return opener("Voir les changements", "Masquer les changements", where, async () => {
    const c = await api("/api/changes", { id: item.id });
    const nodes = item.summary || !c.summary ? [] : [focus("Ce que fennec a fait", c.summary)];
    nodes.push(diffView(c.diff));
    if (c.cut) nodes.push(el("div", { className: "muted", textContent: "Coupé : le reste est trop long pour la page, regarde-le dans le projet." }));
    return nodes;
  });
}

// Every step, with what was sent to the model, what came back, and what it cost.
function journalButton(item, where) {
  return opener("Journal", "Masquer le journal", where, async () => {
    const steps = await api("/api/journal", { id: item.id });
    if (!steps.length) return [el("div", { className: "muted", textContent: "Aucune étape gardée pour cette demande." })];
    return steps.map(stepView);
  });
}

// Work under way: it opens on its journal, which follows the model as it answers.
function underway(item) {
  const slot = el("div");
  const box = el("details", { className: "past" }, el("summary", {}, line(item, item.step, true)), slot);
  box.dataset.key = "run-" + item.id;
  box.ontoggle = () => follow(item.id, box, slot);
  return box;
}

async function follow(id, box, slot) {
  let last = "";
  while (box.open && box.isConnected) {
    try {
      const steps = await api("/api/journal", { id });
      const json = JSON.stringify(steps);
      if (json !== last) {
        last = json;
        // What the human opened stays open as new messages come in.
        const opened = [...slot.querySelectorAll("details")].map((d) => d.open);
        slot.replaceChildren(...steps.map(stepView));
        slot.querySelectorAll("details").forEach((d, i) => { d.open = opened[i] ?? false; });
      }
    } catch (e) {
      slot.replaceChildren(el("div", { className: "bad", textContent: e.message }));
    }
    await new Promise((done) => setTimeout(done, 3000));
  }
}

const number = (n) => (n ?? 0).toLocaleString("fr-FR");

function duration(ms) {
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`;
}

function stepView(step) {
  const c = step.cost;
  const cost = [step.model, `${c.turns} appel${c.turns > 1 ? "s" : ""} au modèle`,
    `${number(c.sent)} tokens envoyés`,
    `${number(c.received)} reçus` + (c.thinking ? ` (dont ${number(c.thinking)} de réflexion)` : ""),
    duration(c.ms)].filter(Boolean).join(" · ");
  const verdict = step.passed === true ? " ✓" : step.passed === false ? " ✗" : "";
  const messages = step.messages ?? [];
  return el("div", { className: "step" },
    el("div", { className: "step-name", textContent: step.name + verdict }),
    el("div", { className: "muted", textContent: cost }),
    ...(messages.length ? messages.map(messageView)
      : [el("div", { className: "muted", textContent: step.live
        ? "fennec attend la première réponse du modèle…"
        : "Les messages de cette étape n'ont pas été gardés (demande ancienne)." })]));
}

const who = { system: "Consigne", user: "Envoyé au modèle", assistant: "Modèle", tool: "Résultat d'outil" };

function messageView(m) {
  const text = typeof m.content === "string" ? m.content : m.content == null ? "" : JSON.stringify(m.content, null, 1);
  const calls = (m.tool_calls ?? []).map((t) => `→ ${t.function?.name} ${t.function?.arguments ?? ""}`);
  const thought = m.reasoning_content ?? m.reasoning;
  const head = (text || calls.join(" ") || thought || "").replace(/\s+/g, " ").slice(0, 110);
  return el("details", { className: "message" },
    el("summary", {}, el("b", { textContent: who[m.role] ?? m.role }), " ", head),
    thought ? el("pre", { className: "thought", textContent: "Réflexion :\n" + thought }) : null,
    text ? el("pre", { textContent: text }) : null,
    calls.length ? el("pre", { textContent: calls.join("\n") }) : null);
}

function diffView(diff) {
  if (!diff) return el("div", { className: "muted", textContent: "Aucun changement." });
  const pre = el("pre", { className: "diff" });
  for (const l of diff.split("\n")) {
    const kind = l.startsWith("diff --git") ? "file" : l.startsWith("+") && !l.startsWith("+++") ? "add"
      : l.startsWith("-") && !l.startsWith("---") ? "del" : l.startsWith("@@") ? "hunk" : "";
    if (/^(index |--- |\+\+\+ )/.test(l)) continue;
    pre.append(el("span", { className: kind, textContent: kind === "file" ? l.replace(/^diff --git a\/(\S+) .*/, "$1") : l }), "\n");
  }
  return pre;
}

// The conversation that shaped the request, the human's words on the right.
function conversation(item) {
  // The opening message is the card's title already.
  const said = (item.thread ?? []).filter((t, i) => i > 0 || t.who !== "human");
  if (!said.length) return null;
  return el("div", { className: "thread" }, ...said.map((t) =>
    el("div", { className: t.who === "human" ? "said human" : "said" },
      el("span", { className: "label", textContent: t.who === "human" ? "Toi" : "fennec" }), ...rich(t.text))));
}

function landLabel({ branch, remote, committed }) {
  if (!branch) return "Préparer le commit";
  if (committed) return `Pousser ${branch} vers ${remote}`;
  return remote ? `Commit dans ${branch} et pousser vers ${remote}` : `Commit dans ${branch}`;
}

function card(item) {
  const [tag, color] = TAGS[item.kind];
  const node = el("div", { className: "card" },
    el("div", { className: "meta", style: `--c: ${color}` },
      el("span", { className: "tag", textContent: tag }), el("span", { textContent: item.project })),
    el("div", { className: "title", textContent: item.title.length > 200 ? item.title.slice(0, 200) + "…" : item.title }));
  const actions = el("div", { className: "actions" });
  const add = (...nodes) => node.append(...nodes.filter(Boolean));

  if (item.kind === "talk") {
    add(conversation(item));
    const text = el("textarea", { placeholder: "Ta réponse", ariaLabel: "Ta réponse" });
    add(el("div", { className: "reply" }, text));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      action("C'est clair, cadre-la", "/api/frame", { id: item.id }),
      action("Répondre", "/api/say", () => ({ id: item.id, text: text.value }), { primary: true }));
  } else if (item.kind === "validate") {
    add(conversation(item), focus("Terminé quand", item.done_when));
    const correction = el("textarea", { placeholder: "Ce qui ne va pas, ou une précision", hidden: true });
    const fix = el("button", { type: "button", textContent: "Corriger" });
    fix.onclick = () => {
      // Opening the field is the first click; sending is the second.
      correction.hidden = false;
      correction.focus();
      fix.replaceWith(action("Envoyer", "/api/say", () => ({ id: item.id, text: correction.value })));
    };
    add(el("div", { className: "reply" }, correction));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }), fix,
      action("Valider", "/api/answer", { id: item.id, accept: true, correction: "" }, { primary: true }));
  } else if (item.kind === "blocked") {
    // Requests from before the conversation have the question alone.
    const last = item.thread?.at(-1);
    add(...(last?.who === "fennec" && last.text === item.question
      ? [conversation(item)] : [conversation(item), focus("fennec demande", item.question)]));
    const text = el("textarea", { placeholder: "Ta réponse", ariaLabel: "Ta réponse" });
    add(el("div", { className: "reply" }, text));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      action("Répondre", "/api/reply", () => ({ id: item.id, text: text.value }), { primary: true }));
  } else if (item.kind === "rule") {
    add(focus("Règle proposée", item.rule), el("div", { className: "muted", textContent: `D'après ce que tu as dit : « ${item.heard ?? ""} »` }));
    actions.append(action("Non", "/api/rule", { id: item.id, scope: "no" }),
      action("Pour ce projet", "/api/rule", { id: item.id, scope: "project" }),
      action("Pour tous mes projets", "/api/rule", { id: item.id, scope: "user" }, { primary: true }));
  } else if (item.kind === "ready") {
    if (item.done_when) add(focus("Terminé quand", item.done_when));
    add(...work(item), shots(item));
    // Commands leave no picture: their proof would otherwise hide behind "Détails".
    const tried = item.tries?.length ?? 0;
    if (tried) {
      add(el("div", { className: "proved",
        textContent: `✓ fennec l'a essayé : ${tried} essai${tried > 1 ? "s" : ""} réussi${tried > 1 ? "s" : ""} (voir Détails)` }));
    }
    const warns = (item.untried ? ["le produit n'a pas encore été essayé"] : []).concat(item.unverified ?? []);
    if (warns.length) {
      add(el("div", { className: "muted", style: "margin-top:10px", textContent: "Non vérifié — à regarder toi-même :" }),
        el("ul", { className: "warn" }, ...warns.map((w) => el("li", {}, ...rich(w)))));
    }
    if (item.land_error) add(el("div", { className: "bad", textContent: "Commit interrompu : " + firstLine(item.land_error) }));
    // Three ways out, none forced: look, set aside, or commit.
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      changesButton(item, () => node.querySelector(".changes-slot")),
      action("Passer", "/api/pass", { id: item.id }));
    if (item.land) actions.append(action(landLabel(item.land), "/api/land", { id: item.id }, { primary: true }));
  } else if (item.kind === "failed") {
    add(el("div", { className: "bad", textContent: explain(item.reason) || "raison inconnue" }), shots(item));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      action("Réessayer", "/api/retry", { id: item.id }, { primary: true }));
  }
  // What was said to the model so far, on every card with a choice to make.
  actions.querySelector(".spacer")?.after(journalButton(item, () => node.querySelector(".changes-slot")));
  add(el("div", { className: "changes-slot" }), details(item), actions, el("div", { className: "error", role: "alert" }));
  return node;
}

function line(item, state, dot) {
  return el("div", { className: "line" }, dot ? el("span", { className: "dot" }) : null,
    el("span", { className: "muted", textContent: item.project }),
    el("span", { className: "what", textContent: item.title.length > 120 ? item.title.slice(0, 120) + "…" : item.title }),
    el("span", { className: "state", textContent: state }));
}

// A finished request opens on what was asked, done and proven.
function past(item) {
  const box = el("details", { className: "past" },
    el("summary", {}, line(item, pastState(item), false)));
  box.dataset.key = "past-" + item.id;
  const talk = conversation(item);
  if (talk) box.append(talk);
  if (item.done_when) box.append(focus("Terminé quand", item.done_when));
  box.append(...work(item));
  const tried = item.tries?.length ?? 0;
  if (tried) box.append(el("div", { className: "proved", textContent: `✓ fennec l'a essayé : ${tried} essai${tried > 1 ? "s" : ""} réussi${tried > 1 ? "s" : ""}` }));
  if (item.unverified?.length) {
    box.append(el("div", { className: "muted", textContent: "Non vérifié :" }),
      el("ul", { className: "warn" }, ...item.unverified.map((w) => el("li", {}, ...rich(w)))));
  }
  const s = shots(item);
  if (s) box.append(s);
  const more = details(item);
  if (more) box.append(more);
  const slot = el("div");
  const actions = el("div", { className: "actions" });
  actions.append(journalButton(item, () => slot));
  if (item.changed) actions.append(changesButton(item, () => slot));
  if (item.kind === "passed") actions.append(action("Reprendre", "/api/resume", { id: item.id }, { primary: true }));
  box.append(slot, el("div", { className: "past-actions" }, actions, el("div", { className: "error", role: "alert" })));
  return box;
}

function pastState(item) {
  if (item.kind === "passed") return "mis de côté";
  if (item.kind === "dropped") return "abandonné";
  const parts = [item.branch, item.remote && `poussé vers ${item.remote}`, item.commit?.slice(0, 7)];
  return "✓ commit " + parts.filter(Boolean).join(" · ");
}

function renderProjects(projects) {
  const select = $("project");
  const current = select.value;
  // With several projects, none is preselected: a silent default sends a
  // request to the wrong project.
  const prompt = projects.length ? "Quel projet ?" : "Aucun projet";
  const none = projects.length === 1 ? [] : [new Option(prompt, "")];
  select.replaceChildren(...none, ...projects.map((p) => new Option(p.name, p.path)));
  if (projects.some((p) => p.path === current)) select.value = current;
}

const fill = (box, nodes, empty) =>
  box.replaceChildren(...(nodes.length ? nodes : [el("div", { className: "empty", textContent: empty })]));

let shown = "";
async function refresh(force) {
  // Never wipe what the human is typing or reading in a card.
  const busy = document.activeElement?.closest("#todo") || [...document.querySelectorAll("#todo textarea")].some((t) => t.value);
  if (!force && busy) return;
  try {
    const view = await api("/api/view");
    const json = JSON.stringify(view);
    if (!force && json === shown) return;
    shown = json;
    $("error").textContent = "";
    const open = new Set([...document.querySelectorAll("details[open]")].map((d) => d.dataset.key));
    fill($("todo"), view.todo.map(card), "Rien ne t'attend.");
    $("n-todo").textContent = view.todo.length ? `· ${view.todo.length}` : "";
    // Seen from another tab: what waits for the human, in the tab's title.
    document.title = view.todo.length ? `(${view.todo.length}) fennec` : "fennec";
    fill($("running"), view.running.map(underway), "Rien en cours.");
    fill($("history"), view.history.map(past), "Rien de terminé pour l'instant.");
    for (const d of document.querySelectorAll("details")) d.open = open.has(d.dataset.key);
    // How it works, while nothing is under way: the page alone must explain itself.
    $("how").hidden = view.todo.length + view.running.length > 0;
    renderProjects(view.projects);
  } catch (e) {
    $("error").textContent = "Serveur injoignable : " + e.message;
  }
}

$("new-request").addEventListener("submit", async (event) => {
  event.preventDefault();
  $("error").textContent = "";
  try {
    await api("/api/requests", { project: $("project").value, text: $("text").value });
    $("text").value = "";
    refresh(true);
  } catch (e) {
    $("error").textContent = e.message;
  }
});

// Folder picker: the server lists folders, since a page cannot read paths.
let pickerPath = "";
async function browse(path) {
  const data = await api("/api/folders", { path });
  pickerPath = data.path;
  // An empty folder starts a new project; files without git are not ours to commit.
  $("picker-path").textContent = data.path + (data.repo ? " · dépôt git"
    : data.empty ? " · dossier vide" : " · pas un dépôt git : choisis un dépôt ou un dossier vide");
  $("picker-choose").disabled = !data.repo && !data.empty;
  $("picker-choose").textContent = data.repo || !data.empty ? "Ajouter ce dossier" : "Créer le projet ici";
  const entries = (data.parent ? [{ name: "..", path: data.parent }] : []).concat(data.children);
  $("picker-list").replaceChildren(...entries.map((entry) => {
    const button = el("button", { type: "button", textContent: entry.name });
    if (entry.repo) button.append(el("span", { className: "repo", textContent: "git" }));
    button.onclick = () => browse(entry.path).catch((e) => ($("picker-path").textContent = e.message));
    return el("li", {}, button);
  }));
}

$("add-project").onclick = async () => {
  await browse(pickerPath);
  $("picker").showModal();
};
$("picker-go").onsubmit = (e) => {
  e.preventDefault();
  browse($("picker-input").value.trim()).catch(() => ($("picker-path").textContent = "Dossier introuvable : " + $("picker-input").value));
};
$("picker-cancel").onclick = () => $("picker").close();
$("picker-choose").onclick = async () => {
  try {
    await api("/api/projects", { path: pickerPath });
    $("picker").close();
    // A project just added is the one the next request is for.
    $("project").append(new Option("", pickerPath));
    $("project").value = pickerPath;
    // Closing hands focus back to "+ projet": typing would then reopen it.
    $("text").focus();
    refresh(true);
  } catch (e) {
    $("picker-path").textContent = e.message;
  }
};

// The team of the project chosen above: which team, which of its roles
// work, and roles to add from the catalogue. Each change is saved at once.
const STAGES = { frame: "cadrage", solve: "réalisation", prove: "preuve" };
const order = Object.keys(STAGES);
const byStage = (a, b) => order.indexOf(a.stage) - order.indexOf(b.stage);
let team = null;

function renderTeam(view) {
  view.roles.sort(byStage);
  view.catalog.sort(byStage);
  team = view;
  // A team named by the project but not found here: say so, and let the
  // human pick one, since fennec cannot work on the project until then.
  $("team-error").textContent = view.missing
    ? `L'équipe « ${view.team} » est introuvable ici : choisis-en une pour que fennec puisse travailler.`
    : "";
  const missing = view.missing ? [new Option(`${view.team} (introuvable)`, view.team)] : [];
  $("team-name").replaceChildren(...missing, ...view.teams.map((t) => new Option(t, t)));
  $("team-name").value = view.team;
  $("team-roles").replaceChildren(...view.roles.map((r) => {
    const box = el("input", { type: "checkbox", checked: !r.out, ariaLabel: r.name });
    box.onchange = () => {
      const remove = team.roles.filter((x) => x.key === r.key ? !box.checked : x.out).map((x) => x.key);
      teamCall("/api/team/choose", { team: team.team, remove });
    };
    const name = el("button", { type: "button", textContent: r.name, title: "Modifier ce rôle" });
    name.onclick = () => editRole(r);
    return el("li", { className: r.out ? "out" : "" }, box, name,
      el("span", { className: "muted", textContent: [STAGES[r.stage] ?? r.stage, r.model].filter(Boolean).join(" · ") }));
  }));
  $("team-catalog").replaceChildren(new Option("Ajouter un rôle…", ""),
    ...view.catalog.map((r) => new Option(`${r.name} — ${STAGES[r.stage] ?? r.stage}`, r.key)));
}

async function teamCall(path, body) {
  try {
    renderTeam(await api(path, { project: $("project").value, ...body }));
    return true;
  } catch (e) {
    $("team-error").textContent = e.message;
    return false;
  }
}

// One form for a role edited or written from scratch; saved in the project.
let editing = null;
$("role-stage").replaceChildren(...Object.entries(STAGES).map(([k, label]) => new Option(label, k)));
async function editRole(r) {
  editing = r?.key ?? null;
  // The models the server serves; the role's own stays listed if it no longer does.
  const fallback = new Option("Modèle : celui des Réglages", "");
  $("role-model").replaceChildren(fallback);
  let served = [];
  try {
    const s = await api("/api/settings", {});
    served = (await api("/api/settings/served", { base_url: s.base_url, api_key: "" })).models;
  } catch {
    // The server does not answer: the role keeps what it has.
  }
  for (const m of new Set([...served, r?.model].filter(Boolean))) $("role-model").append(new Option(`Modèle : ${m}`, m));
  $("role-model").value = r?.model ?? "";
  $("role-name").value = r?.name ?? "";
  $("role-stage").value = r?.stage ?? "solve";
  // Role files wrap their lines; the box shows each paragraph as one.
  $("role-brief").value = (r?.brief ?? "").replace(/([^\n])\n(?!\n)/g, "$1 ");
  $("role-checks").value = (r?.checks ?? []).join("\n");
  showChecks();
  $("team-role").hidden = false;
  $("role-name").focus();
}
// Only a proof runs a role's commands, so only a proof role has them.
const showChecks = () => ($("role-checks").hidden = $("role-stage").value !== "prove");
$("role-stage").onchange = showChecks;
$("role-new").onclick = () => editRole(null);
$("role-cancel").onclick = () => ($("team-role").hidden = true);
$("team-role").onsubmit = async (e) => {
  e.preventDefault();
  const checks = $("role-stage").value === "prove" ? $("role-checks").value.split("\n").filter((c) => c.trim()) : [];
  const body = { key: editing, name: $("role-name").value, stage: $("role-stage").value, brief: $("role-brief").value, checks, model: $("role-model").value };
  if (await teamCall("/api/team/role", body)) $("team-role").hidden = true;
};
$("team-save-open").onclick = () => {
  $("team-save").hidden = false;
  $("team-new-name").focus();
};
$("team-save").onsubmit = async (e) => {
  e.preventDefault();
  if (await teamCall("/api/team/save", { name: $("team-new-name").value })) {
    $("team-save").hidden = true;
    $("team-new-name").value = "";
  }
};

$("open-team").onclick = async () => {
  const project = $("project").value;
  if (!project) {
    $("error").textContent = "Choisis d'abord le projet dont tu veux voir l'équipe.";
    return;
  }
  $("team-title").textContent = `Équipe — ${$("project").selectedOptions[0].textContent}`;
  try {
    renderTeam(await api("/api/team", { project }));
    $("team-role").hidden = $("team-save").hidden = true;
    $("team").showModal();
  } catch (e) {
    $("error").textContent = e.message;
  }
};
// Another team starts whole: what was left out belonged to the old one.
$("team-name").onchange = () => teamCall("/api/team/choose", { team: $("team-name").value, remove: [] });
$("team-add").onsubmit = (e) => {
  e.preventDefault();
  if ($("team-catalog").value) teamCall("/api/team/add", { role: $("team-catalog").value });
};
$("team-close").onclick = () => $("team").close();

refresh();
setInterval(refresh, 3000);

// Where the model is. Ticking none means every model the server serves.
let chosen = [];
let asked = 0;
async function listModels() {
  // A slow answer for an address since replaced must not overwrite the new one.
  const ask = ++asked;
  $("settings-status").textContent = "Je demande au serveur…";
  $("settings-models").replaceChildren();
  try {
    const { models } = await api("/api/settings/served", { base_url: $("settings-url").value, api_key: $("settings-key").value });
    if (ask !== asked) return;
    $("settings-status").textContent = models.length
      ? "Coche les modèles à utiliser ; aucun coché : tous ceux du serveur."
      : "Le serveur répond mais ne sert aucun modèle.";
    $("settings-models").replaceChildren(...models.map((m) =>
      el("li", {}, el("input", { type: "checkbox", value: m, checked: chosen.includes(m), ariaLabel: m }), m)));
  } catch (e) {
    if (ask === asked) $("settings-status").textContent = unreachable(e.message) ?? e.message;
  }
}
$("open-settings").onclick = async () => {
  try {
    const s = await api("/api/settings", {});
    chosen = s.models;
    $("settings-url").value = s.base_url;
    $("settings-key").value = "";
    $("settings-key").placeholder = s.has_key ? "enregistrée (laisse vide pour la garder)" : "";
    $("settings-env").hidden = !s.env.length;
    $("settings-env").textContent = `Au lancement, ${s.env.join(", ")} passe avant ces réglages.`;
    $("settings-error").textContent = "";
    $("settings").showModal();
    listModels();
  } catch (e) {
    $("error").textContent = e.message;
  }
};
$("settings-test").onclick = () => {
  // An empty list (server down, or still asking) says nothing about the choice.
  const boxes = [...document.querySelectorAll("#settings-models input")];
  if (boxes.length) chosen = boxes.filter((b) => b.checked).map((b) => b.value);
  listModels();
};
$("settings-cancel").onclick = () => $("settings").close();
$("settings-form").onsubmit = async (e) => {
  e.preventDefault();
  const boxes = [...document.querySelectorAll("#settings-models input")];
  // Models the server did not list (it was down) are kept as they were.
  const models = boxes.length ? boxes.filter((b) => b.checked).map((b) => b.value) : chosen;
  try {
    await api("/api/settings/save", { base_url: $("settings-url").value, api_key: $("settings-key").value, models });
    $("settings").close();
  } catch (err) {
    $("settings-error").textContent = err.message;
  }
};
