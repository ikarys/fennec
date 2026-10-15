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
  return el;
}

function renderProjects(projects) {
  const select = $("project");
  const current = select.value;
  select.replaceChildren(...projects.map((p) => new Option(p.name, p.path)));
  if (!projects.length) select.append(new Option("Ajoute un projet d'abord", ""));
  if (projects.some((p) => p.path === current)) select.value = current;
}

async function refresh() {
  try {
    const view = await api("/api/view");
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
    refresh();
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
    refresh();
  } catch (e) {
    $("picker-path").textContent = e.message;
  }
};

refresh();
setInterval(refresh, 3000);
