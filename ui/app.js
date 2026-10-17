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
    const error = button.closest(".card").querySelector(".error");
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
  validate: ["À valider", "var(--accent)"],
  blocked: ["Question", "var(--accent)"],
  rule: ["Règle ?", "var(--accent)"],
  ready: ["Prêt à intégrer", "var(--ok)"],
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
function explain(reason) {
  const line = firstLine(reason);
  let m = line.match(/^rule task\.protect violated: changed (.+)$/);
  if (m) return `fennec a modifié un test validé (${m[1]}), ce qui lui est interdit`;
  m = line.match(/^rule \S+ violated: changed (\S+) but nothing under (\S+)$/);
  if (m) return `${m[1]} a changé sans changement dans ${m[2]}, comme les règles du projet l'exigent`;
  m = line.match(/^no model: .* and (\S+)\/models did not answer/);
  if (m) return `fennec ne joint aucun modèle à ${m[1]} : démarre-le avec FENNEC_OS_BASE_URL=<adresse du serveur du modèle>, puis réessaie`;
  m = line.match(/^rule \S+ violated: changed (.+)$/);
  if (m) return `fennec a modifié ${m[1]}, que les règles du projet protègent`;
  return line;
}

function shots(item) {
  const row = el("div", { className: "shots" });
  for (const s of item.shots ?? []) {
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

function landLabel({ branch, remote, committed }) {
  if (!branch) return "Préparer l'intégration";
  if (committed) return `Pousser ${branch} vers ${remote}`;
  return remote ? `Intégrer dans ${branch} et pousser vers ${remote}` : `Intégrer dans ${branch}`;
}

function card(item) {
  const [tag, color] = TAGS[item.kind];
  const node = el("div", { className: "card" },
    el("div", { className: "meta", style: `--c: ${color}` },
      el("span", { className: "tag", textContent: tag }), el("span", { textContent: item.project })),
    el("div", { className: "title", textContent: item.title.length > 200 ? item.title.slice(0, 200) + "…" : item.title }));
  const actions = el("div", { className: "actions" });
  const add = (...nodes) => node.append(...nodes.filter(Boolean));

  if (item.kind === "validate") {
    add(focus("Terminé quand", item.done_when));
    const correction = el("textarea", { placeholder: "Ce qui ne va pas", hidden: true });
    const fix = el("button", { type: "button", textContent: "Corriger" });
    fix.onclick = () => {
      // Opening the field is the first click; sending is the second.
      correction.hidden = false;
      correction.focus();
      fix.replaceWith(action("Envoyer la correction", "/api/answer", () => ({ id: item.id, accept: false, correction: correction.value })));
    };
    add(el("div", { className: "reply" }, correction));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }), fix,
      action("Valider", "/api/answer", { id: item.id, accept: true, correction: "" }, { primary: true }));
  } else if (item.kind === "blocked") {
    add(focus("fennec demande", item.question));
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
    add(shots(item));
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
    if (item.land_error) add(el("div", { className: "bad", textContent: "Intégration interrompue : " + firstLine(item.land_error) }));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }));
    if (item.land) actions.append(action(landLabel(item.land), "/api/land", { id: item.id }, { primary: true }));
  } else if (item.kind === "failed") {
    add(el("div", { className: "bad", textContent: explain(item.reason) || "raison inconnue" }), shots(item));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      action("Réessayer", "/api/retry", { id: item.id }, { primary: true }));
  }
  add(details(item), actions, el("div", { className: "error", role: "alert" }));
  return node;
}

function line(item, state, dot) {
  return el("div", { className: "line" }, dot ? el("span", { className: "dot" }) : null,
    el("span", { className: "muted", textContent: item.project }),
    el("span", { className: "what", textContent: item.title.length > 120 ? item.title.slice(0, 120) + "…" : item.title }),
    el("span", { className: "state", textContent: state }));
}

function landedState(item) {
  const parts = [item.branch, item.remote && `poussé vers ${item.remote}`, item.commit?.slice(0, 7)];
  return "✓ " + parts.filter(Boolean).join(" · ");
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
    for (const d of document.querySelectorAll("details")) d.open = open.has(d.dataset.key);
    $("n-todo").textContent = view.todo.length ? `· ${view.todo.length}` : "";
    fill($("running"), view.running.map((r) => line(r, r.step, true)), "Rien en cours.");
    fill($("landed"), view.landed.map((r) => line(r, landedState(r), false)), "Rien d'intégré pour l'instant.");
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
      el("span", { className: "muted", textContent: STAGES[r.stage] ?? r.stage }));
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
function editRole(r) {
  editing = r?.key ?? null;
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
  const body = { key: editing, name: $("role-name").value, stage: $("role-stage").value, brief: $("role-brief").value, checks };
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
    if (ask === asked) $("settings-status").textContent = e.message;
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
