// The single view. Polls the server; a page left open stays current.

const $ = (id) => document.getElementById(id);
const COLUMNS = {
  mine: "Rien ne t'attend.",
  ready: "Rien de prêt.",
  running: "Rien en cours.",
  failed: "Aucun échec.",
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

function card(item) {
  const el = document.createElement("div");
  el.className = "card";
  for (const key of ["project", "title", "detail"]) {
    const line = document.createElement("div");
    line.className = key;
    line.textContent = item[key] ?? "";
    el.append(line);
  }
  if (item.checks?.length) {
    const checks = document.createElement("details");
    checks.append(Object.assign(document.createElement("summary"), { textContent: "Tests" }));
    for (const command of item.checks) checks.append(Object.assign(document.createElement("code"), { textContent: command }));
    el.append(checks);
  }
  if (item.proof?.length) {
    const proof = document.createElement("details");
    proof.append(Object.assign(document.createElement("summary"), { textContent: "Preuve" }));
    for (const t of item.proof) {
      proof.append(Object.assign(document.createElement("code"), { textContent: `$ ${t.command}  → ${t.exit}` }));
      proof.append(Object.assign(document.createElement("pre"), { textContent: t.output }));
    }
    el.append(proof);
  }
  for (const s of item.shots ?? []) {
    if (!s.file) continue;
    // Visible without a click: the screenshot is the proof a human reads first.
    const link = Object.assign(document.createElement("a"), { href: `/shots/${s.file}`, target: "_blank", className: "shot" });
    link.append(Object.assign(document.createElement("img"), { src: `/shots/${s.file}`, alt: `Capture de ${s.url}`, loading: "lazy" }));
    link.append(Object.assign(document.createElement("small"), { textContent: s.url }));
    el.append(link);
  }
  if (item.ask) el.append(answerForm(item.id));
  if (item.reply) el.append(replyForm(item.id));
  if (item.rule) el.append(ruleForm(item.id));
  if (item.retry) el.append(retryForm(item.id));
  if (item.land !== undefined) el.append(landForm(item.id, item.land));
  return el;
}

// The one answer a request needs: validate "done when", or say what is wrong.
function answerForm(id) {
  const form = document.createElement("form");
  form.className = "answer";
  const correction = Object.assign(document.createElement("textarea"), { placeholder: "Ce qui ne va pas (pour corriger)" });
  const error = Object.assign(document.createElement("div"), { className: "error" });
  const send = (accept) => async () => {
    error.textContent = "";
    try {
      await api("/api/answer", { id, accept, correction: correction.value });
      refresh(true);
    } catch (e) {
      error.textContent = e.message;
    }
  };
  const validate = Object.assign(document.createElement("button"), { type: "button", className: "primary", textContent: "Valider" });
  const fix = Object.assign(document.createElement("button"), { type: "button", textContent: "Corriger" });
  validate.onclick = send(true);
  fix.onclick = send(false);
  const row = Object.assign(document.createElement("div"), { className: "row" });
  row.append(fix, validate);
  form.append(correction, row, error);
  return form;
}

// A stopped request's question: the answer restarts the work with it.
function replyForm(id) {
  const form = document.createElement("form");
  form.className = "answer";
  const text = Object.assign(document.createElement("textarea"), { placeholder: "Ta réponse" });
  const error = Object.assign(document.createElement("div"), { className: "error" });
  const send = Object.assign(document.createElement("button"), { type: "submit", className: "primary", textContent: "Répondre" });
  form.onsubmit = async (e) => {
    e.preventDefault();
    error.textContent = "";
    try {
      await api("/api/reply", { id, text: text.value });
      refresh(true);
    } catch (err) {
      error.textContent = err.message;
    }
  };
  const row = Object.assign(document.createElement("div"), { className: "row" });
  row.append(send);
  form.append(text, row, error);
  return form;
}

// An offered rule: one click says where it holds, or that it does not.
function ruleForm(id) {
  const form = document.createElement("form");
  form.className = "answer";
  const error = Object.assign(document.createElement("div"), { className: "error" });
  const row = Object.assign(document.createElement("div"), { className: "row" });
  for (const [scope, label, primary] of [["no", "Non"], ["project", "Pour ce projet"], ["user", "Pour moi (tous projets)", true]]) {
    const button = Object.assign(document.createElement("button"), { type: "button", textContent: label, className: primary ? "primary" : "" });
    button.onclick = async () => {
      error.textContent = "";
      try {
        await api("/api/rule", { id, scope });
        refresh(true);
      } catch (e) {
        error.textContent = e.message;
      }
    };
    row.append(button);
  }
  form.append(row, error);
  return form;
}

// The work becomes one commit on the project's current branch, named here
// so the human sees where it goes before clicking.
// The branch and remote are on the button: the human sees where it goes
// before it goes there.
function landForm(id, { branch, remote, message, committed }) {
  const form = document.createElement("form");
  form.className = "answer";
  const error = Object.assign(document.createElement("div"), { className: "error" });
  const button = Object.assign(document.createElement("button"), {
    type: "submit", className: "primary",
    textContent: !branch ? "Préparer l'intégration"
      : committed ? `Pousser ${branch} vers ${remote}`
      : remote ? `Intégrer dans ${branch} et pousser vers ${remote}`
      : `Intégrer dans ${branch}`,
  });
  if (message && !committed) {
    form.append(Object.assign(document.createElement("div"), { className: "detail", textContent: `Commit : ${message}` }));
  }
  form.onsubmit = async (e) => {
    e.preventDefault();
    error.textContent = "";
    try {
      await api("/api/land", { id });
      refresh(true);
    } catch (err) {
      error.textContent = err.message;
    }
  };
  const row = Object.assign(document.createElement("div"), { className: "row" });
  row.append(button);
  form.append(row, error);
  return form;
}

// A failed request starts again; fennec is told why the last try failed.
function retryForm(id) {
  const form = document.createElement("form");
  form.className = "answer";
  const error = Object.assign(document.createElement("div"), { className: "error" });
  const button = Object.assign(document.createElement("button"), { type: "submit", className: "primary", textContent: "Réessayer" });
  form.onsubmit = async (e) => {
    e.preventDefault();
    error.textContent = "";
    try {
      await api("/api/retry", { id });
      refresh(true);
    } catch (err) {
      error.textContent = err.message;
    }
  };
  const row = Object.assign(document.createElement("div"), { className: "row" });
  row.append(button);
  form.append(row, error);
  return form;
}

function renderProjects(projects) {
  const select = $("project");
  const current = select.value;
  select.replaceChildren(...projects.map((p) => new Option(p.name, p.path)));
  if (!projects.length) select.append(new Option("Ajoute un projet d'abord", ""));
  if (projects.some((p) => p.path === current)) select.value = current;
}

let shown = "";
async function refresh(force) {
  // Never wipe what the human is typing in a card.
  if (!force && document.activeElement?.closest(".board")) return;
  try {
    const view = await api("/api/view");
    const json = JSON.stringify(view);
    if (!force && json === shown) return;
    shown = json;
    $("error").textContent = "";
    for (const [key, empty] of Object.entries(COLUMNS)) {
      const items = view[key];
      $(key).replaceChildren(...(items.length ? items.map(card) : [Object.assign(
        document.createElement("div"), { className: "empty", textContent: empty })]));
      $("n-" + key).textContent = items.length || "";
    }
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
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = entry.name;
    if (entry.repo) button.append(Object.assign(document.createElement("span"), { className: "repo", textContent: "git" }));
    button.onclick = () => browse(entry.path).catch((e) => ($("picker-path").textContent = e.message));
    li.append(button);
    return li;
  }));
}

$("add-project").onclick = async () => {
  await browse(pickerPath);
  $("picker").showModal();
};
$("picker-cancel").onclick = () => $("picker").close();
$("picker-choose").onclick = async () => {
  try {
    await api("/api/projects", { path: pickerPath });
    $("picker").close();
    refresh(true);
  } catch (e) {
    $("picker-path").textContent = e.message;
  }
};

refresh();
setInterval(refresh, 3000);
