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
  return el("div", { className: "focus" }, el("span", { className: "label", textContent: label }), text);
}

function firstLine(text) {
  return (text ?? "").split("\n").find((l) => l.trim()) ?? "";
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
  for (const rule of item.rules ?? []) box.append(el("p", { textContent: `Règle suivie : ${rule}` }));
  if (item.checks?.length) {
    box.append(el("p", { textContent: "Tests :" }));
    for (const c of item.checks) box.append(el("code", { textContent: c }));
  }
  for (const t of item.tries ?? []) {
    box.append(el("code", { textContent: `$ ${t.command}  → ${t.exit}` }), el("pre", { textContent: t.output }));
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
    const warns = (item.untried ? ["le produit n'a pas encore été essayé"] : []).concat(item.unverified ?? []);
    if (warns.length) {
      add(el("div", { className: "muted", style: "margin-top:10px", textContent: "Non vérifié — à regarder toi-même :" }),
        el("ul", { className: "warn" }, ...warns.map((w) => el("li", { textContent: w }))));
    }
    if (item.land_error) add(el("div", { className: "bad", textContent: "Intégration interrompue : " + firstLine(item.land_error) }));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }));
    if (item.land) actions.append(action(landLabel(item.land), "/api/land", { id: item.id }, { primary: true }));
  } else if (item.kind === "failed") {
    add(el("div", { className: "bad", textContent: firstLine(item.reason) || "raison inconnue" }), shots(item));
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
  const prompt = projects.length ? "Quel projet ?" : "Ajoute un projet d'abord";
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
  $("picker-path").textContent = data.path + (data.repo ? " · dépôt git" : "");
  $("picker-choose").disabled = !data.repo;
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
    refresh(true);
  } catch (e) {
    $("picker-path").textContent = e.message;
  }
};

refresh();
setInterval(refresh, 3000);
