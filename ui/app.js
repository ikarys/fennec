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
  iconize(button);
  button.onclick = async () => {
    const error = errorOf(button);
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

// Where a button's failure is said: next to the buttons, wherever they sit.
const errorOf = (button) => button.closest(".decide, .card, .past-actions").querySelector(".error");

// Giving up is one more click than doing: the first click only asks.
// Once there is work, giving up undoes it: the label says so.
function dropButton(id, undo = false) {
  const button = el("button", { type: "button", className: "quiet", textContent: undo ? "Annuler le travail" : "Abandonner" });
  button.onclick = () => {
    const sure = action(undo ? "Confirmer : tout annuler" : "Confirmer l'abandon", "/api/drop", { id }, { quiet: true });
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
  later: ["Plus tard", "var(--muted)"],
  failed: ["Échoué", "var(--bad)"],
};

function focus(label, text) {
  return el("div", { className: "focus" }, el("span", { className: "label", textContent: label }), ...md(text));
}

// `command` spans as code, **words** in bold, the rest as plain text.
function rich(text) {
  return (text ?? "").split("`").flatMap((part, i) => (i % 2 ? [el("code", { textContent: part })]
    : part.split("**").map((p, j) => (j % 2 ? el("b", { textContent: p }) : p))));
}

// `**words**` in bold, `code` as code, [label](url) as a link, the rest plain.
function inline(text) {
  const out = [];
  const rest = text ?? "";
  const re = /(`[^`\n]+`)|(\*\*[^*\n]+?\*\*)|(\[[^\]\n]*\]\([^)\s]+\))|(\*[^*\n]+\*)/g;
  let last = 0;
  let m;
  while ((m = re.exec(rest))) {
    if (m.index > last) out.push(rest.slice(last, m.index));
    if (m[1]) out.push(el("code", { textContent: m[1].slice(1, -1) }));
    else if (m[2]) out.push(el("b", {}, ...inline(m[2].slice(2, -2))));
    else if (m[3]) {
      const link = m[3].match(/^\[([^\]\n]*)\]\(([^)\s]+)\)$/);
      out.push(el("a", { href: link[2], target: "_blank", rel: "noreferrer" }, ...inline(link[1])));
    }
    else out.push(el("i", {}, ...inline(m[4].slice(1, -1))));
    last = m.index + m[0].length;
  }
  if (last < rest.length) out.push(rest.slice(last));
  return out;
}

// Models write Markdown: its lines become headings, lists, tables and code,
// not one block of raw symbols.
function md(text) {
  const lines = (text ?? "").split("\n");
  const blocks = [];
  const lists = [];
  const cells = (line) => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => c.trim());
  const separator = (l) => l.includes("-") && /^\s*\|?[\s:|-]+\|?\s*$/.test(l);
  let i = 0;
  while (i < lines.length) {
    const l = lines[i];
    const fence = l.match(/^\s*(`{3,}|~{3,})/);
    if (fence) {
      const close = fence[1][0].repeat(3);
      const buf = [];
      i++;
      while (i < lines.length && !lines[i].trimStart().startsWith(close)) buf.push(lines[i++]);
      i++;
      blocks.push(el("pre", { textContent: buf.join("\n") }));
      lists.length = 0;
      continue;
    }
    const head = l.match(/^\s{0,3}(#{1,6})\s+(.*)$/);
    if (head) {
      blocks.push(el(`h${head[1].length}`, {}, ...inline(head[2].trim())));
      lists.length = 0;
      i++;
      continue;
    }
    if (l.includes("|") && i + 1 < lines.length && separator(lines[i + 1])) {
      const rows = [];
      while (i < lines.length && lines[i].includes("|") && lines[i].trim()) rows.push(lines[i++]);
      const [h, , ...body] = rows;
      const table = el("table", { className: "grid" },
        el("thead", {}, el("tr", {}, ...cells(h).map((c) => el("th", {}, ...inline(c))))));
      if (body.length) table.append(el("tbody", {},
        ...body.map((r) => el("tr", {}, ...cells(r).map((c) => el("td", {}, ...inline(c)))))));
      blocks.push(table);
      lists.length = 0;
      continue;
    }
    const item = l.match(/^(\s*)(?:([-*+])|(\d+)[.)])\s+(.*)$/);
    if (item) {
      const indent = item[1].replace(/\t/g, "    ").length;
      while (lists.length && lists.at(-1).indent > indent) lists.pop();
      const top = lists.at(-1);
      const li = el("li", {}, ...inline(item[4]));
      if (top && top.indent === indent) {
        top.node.append(li);
        lists.push({ indent, node: top.node, li });
      } else {
        const node = el(item[3] ? "ol" : "ul");
        node.append(li);
        if (top) top.li.append(node); else blocks.push(node);
        lists.push({ indent, node, li });
      }
      i++;
      continue;
    }
    lists.length = 0;
    if (/^\s*>/.test(l)) {
      const buf = [];
      while (i < lines.length && /^\s*>/.test(lines[i])) buf.push(lines[i++].replace(/^\s*> ?/, ""));
      blocks.push(el("blockquote", {}, ...md(buf.join("\n"))));
      continue;
    }
    if (/^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(l)) {
      blocks.push(el("hr"));
      i++;
      continue;
    }
    if (l.trim()) blocks.push(el("p", {}, ...inline(l)));
    i++;
  }
  return blocks;
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
  if (/not a git repository/.test(line)) {
    return "le dossier du projet n'est plus un dépôt git (son .git a disparu) : ajoute-le de nouveau avec « + projet », puis réessaie";
  }
  if (/the project folder has uncommitted changes/.test(line)) {
    return "ton dossier a des changements non commités : fennec y travaille et doit pouvoir annuler un essai raté sans toucher aux tiens. Commit-les ou mets-les de côté (git stash), puis réessaie";
  }
  if (/the project folder already holds work in progress/.test(line)) {
    return "ton dossier contient déjà le travail d'une autre demande : commit-la, ou abandonne-la, puis réessaie";
  }
  m = line.match(/^port (\d+) is already taken/);
  if (m) return `le port ${m[1]} est déjà pris sur ta machine : arrête ce qui l'occupe, puis rouvre la page`;
  m = line.match(/^nothing listens on port (\d+) after 30 s/);
  if (m) return `la page n'a pas démarré : rien n'écoute sur le port ${m[1]} après 30 s`;
  if (/the project folder is on no branch/.test(line)) return "ton dossier n'est sur aucune branche git : fais un checkout d'une branche, puis réessaie";
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
  // Without stats (an old request), the team that worked on it at least.
  if (item.team?.length && !item.stats?.stages) box.append(el("p", { textContent: `Équipe : ${item.team.join(", ")}` }));
  for (const rule of item.rules ?? []) box.append(el("p", { textContent: `Règle suivie : ${rule}` }));
  // What each step and each model cost, as a table.
  for (const [key, label] of [["stages", "Étape"], ["models", "Modèle"]]) {
    const rows = Object.entries(item.stats?.[key] ?? {}).sort(([a], [b]) => stageRank(a) - stageRank(b));
    if (!rows.length) continue;
    box.append(el("div", { className: "scroll" }, el("table", { className: "grid" },
      el("thead", {}, el("tr", {}, ...[label, key === "stages" ? "Rôles" : "", "Temps", "Appels", "Envoyés", "Cache", "Générés", "Vitesse"]
        .map((h) => el("th", { textContent: h })))),
      el("tbody", {}, ...rows.map(([name, c]) => el("tr", {},
        el("th", {}, key === "stages" ? el("span", { className: `dotc ${stageOf(name)}` }) : null, key === "stages" ? stageName(name) : name),
        el("td", {}, ...(c.roles ?? []).map((r) => roleTag(r, stageOf(name)))),
        el("td", { textContent: duration(c.ms ?? 0) }), el("td", { textContent: number(c.turns) }),
        el("td", { textContent: compact(c.sent) }),
        el("td", {}, el("span", { className: "meter", style: `--p:${percent(c.cached, c.sent)}%` }), `${percent(c.cached, c.sent)} %`),
        el("td", { textContent: compact(c.received) }), el("td", { textContent: c.ms ? `${tps(c)}/s` : "—" })))))));
  }
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
  // Which of the project's skills the work read: the Journal shows where.
  if (item.skills?.length) nodes.push(el("div", { className: "files" }, "Skills utilisés : ", item.skills.join(", ")));
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
  return opener("Voir les changements", "Masquer les changements", where, () => changes(item));
}
async function changes(item) {
  const c = await api("/api/changes", { id: item.id });
  const nodes = item.summary || !c.summary ? [] : [focus("Ce que fennec a fait", c.summary)];
  nodes.push(diffView(c.diff));
  if (c.cut) nodes.push(el("div", { className: "muted", textContent: "Coupé : le reste est trop long pour la page, regarde-le dans le projet." }));
  return nodes;
}

// The ready work is a real folder: opened in the editor, its path shown to copy.
function codeButton(item, where) {
  return opener("Ouvrir le code", "Masquer le dossier", where, async () => {
    const c = await api("/api/open", { id: item.id });
    return [el("div", { className: "muted", textContent: (c.opened ? "Ouvert dans VS Code. " : "VS Code introuvable : ouvre ce dossier dans ton éditeur. ")
        + "Tu peux y modifier le code : au commit, fennec refait les tests avant de mettre le tout dans ton dossier." }),
      el("pre", { textContent: c.folder })];
  });
}

// A web product is tried in the human's own browser, started as the proof started it.
// The tab opens on the click itself: one opened after the answer would be blocked.
function pageButton(item) {
  const error = () => errorOf(button);
  const button = el("button", { type: "button", textContent: "Ouvrir la page" });
  button.onclick = async () => {
    const tab = window.open("", "_blank");
    button.disabled = true;
    button.textContent = "Je lance la page…";
    try {
      const c = await api("/api/page", { id: item.id });
      if (tab) tab.location = c.url;
      else window.location.assign(c.url);
      error().textContent = "";
    } catch (e) {
      tab?.close();
      error().textContent = explain(e.message);
    } finally {
      button.disabled = false;
      button.textContent = "Ouvrir la page";
    }
  };
  return button;
}

// Every step, with what was sent to the model, what came back, and what it cost.
function journalButton(item, where) {
  return opener("Journal", "Masquer le journal", where, () => journal(item));
}
async function journal(item) {
  const steps = await api("/api/journal", { id: item.id });
  if (!steps.length) return [el("div", { className: "muted", textContent: "Aucune étape gardée pour cette demande." })];
  return steps.map(stepView);
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

// Steps in the order they run; the server keeps them sorted by name.
const stageRank = (name) => ["Discussion", "Cadrage", "Démarrage", "Travail", "Preuve"].findIndex((s) => name.startsWith(s)) >>> 0;
const number = (n) => (n ?? 0).toLocaleString("fr-FR");
const compact = (n) => (n ?? 0).toLocaleString("fr-FR", { notation: "compact", maximumFractionDigits: 1 });
const percent = (part, whole) => (whole ? Math.round((100 * part) / whole) : 0);
// Tokens out per second of model time; that time includes reading the prompt.
const tps = (c) => (c.ms ? Math.round((c.received * 1000) / c.ms) : 0);
const speed = (c) => (c.ms && c.received ? `≈ ${tps(c)} tokens/s` : "");

// How the work was kept safe, and what it cost: under the work, small.
function facts(item) {
  const s = item.stats;
  if (!s?.turns) return [];
  const n = s.failed_attempts;
  const safe = el("div", { className: "pills", title: "Chaque essai tourne isolé ; un essai raté est annulé sans trace" },
    pill("🔒", "sans réseau"), pill("🏠", "ton dossier personnel caché"), pill("📖", "git en lecture seule"),
    n ? pill("↺", `${n} essai${n > 1 ? "s" : ""} raté${n > 1 ? "s" : ""} annulé${n > 1 ? "s" : ""}`, "warn") : null);
  return [el("div", { className: "facts" }, metrics(s), timeline(s), safe)];
}

function pill(icon, text, tone = "") {
  return el("span", { className: `pill ${tone}` }, el("span", { className: "icon", textContent: icon, ariaHidden: "true" }), text);
}

// Model time, tokens in and out, cache and speed: a figure each, not a sentence.
function metrics(c) {
  const cache = percent(c.cached, c.sent);
  return el("div", { className: "metrics" },
    metric("Temps de modèle", duration(c.ms ?? 0)),
    metric("Appels", number(c.turns)),
    metric("Envoyés", compact(c.sent), el("span", { className: "meter", style: `--p:${cache}%`, title: `${cache} % en cache` }),
      el("small", { textContent: `${cache} % cache` })),
    metric("Générés", compact(c.received), c.thinking ? el("small", { textContent: `${percent(c.thinking, c.received)} % réflexion` }) : null),
    metric("Vitesse", c.ms ? `${tps(c)}` : "—", el("small", { textContent: "tokens/s" })));
}

function metric(label, value, ...extra) {
  return el("span", { className: "metric" }, el("small", { textContent: label }), el("b", { textContent: value }), ...extra);
}

// Where the time went, one coloured segment per step, with the roles of each.
function timeline(s) {
  const stages = Object.entries(s.stages ?? {}).sort(([a], [b]) => stageRank(a) - stageRank(b));
  if (!stages.length) return null;
  const total = stages.reduce((t, [, c]) => t + (c.ms ?? 0), 0) || 1;
  const bar = el("div", { className: "timeline" }, ...stages.map(([name, c]) =>
    el("span", { className: `seg ${stageOf(name)}`, style: `flex:${Math.max(c.ms ?? 0, total / 40)}`,
      title: `${stageName(name)} : ${duration(c.ms ?? 0)}`, textContent: `${stageName(name)} ${duration(c.ms ?? 0)}` })));
  const who = el("div", { className: "who" }, ...stages.flatMap(([name, c]) =>
    (c.roles ?? []).map((r) => roleTag(r, stageOf(name)))));
  return el("div", {}, bar, who.children.length ? who : null);
}

const stageOf = (name) => (name.startsWith("Travail") ? "solve" : name.startsWith("Preuve") ? "prove" : "frame");
const stageName = (name) => (name.startsWith("Cadrage") ? "Tests d'abord" : name);
const roleTag = (name, stage) => el("span", { className: `rtag ${stage}`, textContent: name });

function duration(ms) {
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`;
}

function stepView(step) {
  const c = step.cost;
  const verdict = step.passed === true ? " ✓" : step.passed === false ? " ✗" : "";
  const messages = step.messages ?? [];
  return el("div", { className: "step" },
    el("div", { className: "step-name" }, el("span", { className: `dotc ${stageOf(step.name)}` }), step.name + verdict,
      step.model ? el("span", { className: "model", textContent: step.model }) : null),
    metrics(c),
    ...rolesView(step),
    ...(messages.length ? messages.map(messageView)
      : [el("div", { className: "muted", textContent: step.live
        ? "fennec attend la première réponse du modèle…"
        : "Les messages de cette étape n'ont pas été gardés (demande ancienne)." })]));
}

// Who of the team took part: what each role told the model, and what it ran itself.
function rolesView(step) {
  if (!step.roles) return [];
  if (!step.roles.length) return [el("div", { className: "muted", textContent: "Aucun rôle de l'équipe à cette étape." })];
  const nodes = [el("div", { className: "who" }, ...step.roles.map((r) => roleTag(r.name, stageOf(step.name))))];
  for (const r of step.roles) {
    if (r.brief) {
      nodes.push(el("details", { className: "message" }, el("summary", {}, el("b", { textContent: r.name }), " : sa consigne au modèle"),
        el("pre", { textContent: r.brief })));
    }
  }
  for (const c of step.checks ?? []) {
    const result = c.missing ? "pas installé, non vérifié" : c.exit === 0 ? "✓ réussi" : `✗ échoué (code ${c.exit}) : bloque`;
    nodes.push(el("details", { className: "message" },
      el("summary", {}, el("b", { textContent: c.role }), ` a lancé « ${c.command} » → ${result}`),
      el("pre", { textContent: c.output || "(aucune sortie)" })));
  }
  return nodes;
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
      el("span", { className: "tag", textContent: tag }), el("span", { className: "proj", textContent: item.project })),
    el("div", { className: "title", textContent: item.title.length > 200 ? item.title.slice(0, 200) + "…" : item.title }));
  const actions = el("div", { className: "actions" });
  const add = (...nodes) => node.append(...nodes.filter(Boolean));

  if (item.kind === "talk") {
    add(conversation(item));
    const text = el("textarea", { placeholder: "Ta réponse", ariaLabel: "Ta réponse" });
    add(el("div", { className: "reply" }, text));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      action("C'est clair, fais-le", "/api/frame", { id: item.id }),
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
    // The decision first, without scrolling: what was proven, what was not,
    // and the buttons. What fennec did and what it cost come under them.
    const tried = item.tries?.length ?? 0;
    // Commands leave no picture: their proof would otherwise hide behind "Détails".
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
    if (item.amend_error) add(el("div", { className: "bad", textContent: "Correction non aboutie, le travail est resté tel quel : " + firstLine(item.amend_error) }));
    // "Ready" reads as "done": say where the work is until the commit.
    const commit = item.land ? `« ${landLabel(item.land)} »` : "le commit";
    add(el("div", { className: "muted", style: "margin-top:10px", textContent: item.here
      ? `Le travail est dans ton dossier, sur une branche de travail. ${commit} le met sur ta branche ; « Annuler le travail » rend ton dossier comme avant.`
      : `Ton dossier n'a pas encore changé : le travail attend à part. ${commit} l'y met.` }));
    // Ways out, none forced: commit, look, ask for a change, set aside or undo.
    const change = el("textarea", { placeholder: "Ce qu'il faut changer", ariaLabel: "Ce qu'il faut changer", hidden: true });
    const fix = el("button", { type: "button", textContent: "Corriger" });
    fix.onclick = () => {
      change.hidden = false;
      change.focus();
      fix.replaceWith(action("Envoyer la correction", "/api/amend", () => ({ id: item.id, text: change.value })));
    };
    if (item.land) actions.append(action(landLabel(item.land), "/api/land", { id: item.id }, { primary: true }));
    if (item.page) actions.append(pageButton(item));
    actions.append(changesButton(item, () => node.querySelector(".changes-slot")),
      codeButton(item, () => node.querySelector(".changes-slot")), fix,
      action("Passer", "/api/pass", { id: item.id }), el("span", { className: "spacer" }), dropButton(item.id, true));
    add(el("div", { className: "reply" }, change), actions, el("div", { className: "error", role: "alert" }),
      el("div", { className: "changes-slot" }), doneWhen(item), ...work(item), shots(item), ...facts(item));
  } else if (item.kind === "later") {
    // Kept, not started: said again if need be, then launched.
    const text = el("textarea", { className: "later-text", defaultValue: item.title, ariaLabel: "La demande" });
    node.querySelector(".title").replaceWith(text);
    actions.append(action("Lancer", "/api/launch", { id: item.id }, { primary: true }),
      action("Enregistrer le texte", "/api/rewrite", () => ({ id: item.id, text: text.value })),
      action("Supprimer", "/api/forget", { id: item.id }, { quiet: true }));
  } else if (item.kind === "failed") {
    add(el("div", { className: "bad", textContent: explain(item.reason) || "raison inconnue" }), shots(item), ...facts(item));
    actions.append(dropButton(item.id, true), el("span", { className: "spacer" }),
      action("Réessayer", "/api/retry", { id: item.id }, { primary: true }));
  }
  // What was said to the model so far, on every card with a choice to make.
  actions.querySelector(".spacer")?.after(journalButton(item, () => node.querySelector(".changes-slot")));
  // A ready card has its buttons at the top already.
  if (node.contains(actions)) add(details(item));
  else add(el("div", { className: "changes-slot" }), details(item), actions, el("div", { className: "error", role: "alert" }));
  return node;
}

// When the work started straight from the request, "done when" is the
// request itself: the title says it already.
function doneWhen(item) {
  const same = (item.done_when ?? "").trim() === (item.title ?? "").trim();
  return item.done_when && !same ? focus("Terminé quand", item.done_when) : null;
}

function line(item, state, dot) {
  // Under way, the figures are the last finished step's: how long it has run says more.
  if (dot && item.since) {
    const elapsed = el("span", { className: "elapsed" });
    elapsed.dataset.since = item.since;
    return el("div", { className: "line" }, el("span", { className: "dot" }),
      el("span", { className: "muted proj", textContent: item.project }),
      el("span", { className: "what", textContent: item.title.length > 120 ? item.title.slice(0, 120) + "…" : item.title }),
      el("span", { className: "state" }, state, " · ", elapsed));
  }
  const s = item.stats;
  return el("div", { className: "line" }, dot ? el("span", { className: "dot" }) : null,
    el("span", { className: "muted proj", textContent: item.project }),
    el("span", { className: "what", textContent: item.title.length > 120 ? item.title.slice(0, 120) + "…" : item.title }),
    s?.turns ? el("span", { className: "state cost", title: "temps de modèle · tokens envoyés · générés" },
      el("span", { className: "mini", textContent: `⏱ ${duration(s.ms)}` }), el("span", { className: "mini", textContent: `↑ ${compact(s.sent)}` }),
      el("span", { className: "mini", textContent: `↓ ${compact(s.received)}` })) : null,
    el("span", { className: "state", textContent: state }));
}

// A finished request opens on what was asked, done and proven.
function past(item) {
  const box = el("details", { className: "past" },
    el("summary", {}, line(item, pastState(item), false)));
  box.dataset.key = "past-" + item.id;
  const talk = conversation(item);
  if (talk) box.append(talk);
  const done = doneWhen(item);
  if (done) box.append(done);
  box.append(...work(item));
  const tried = item.tries?.length ?? 0;
  if (tried) box.append(el("div", { className: "proved", textContent: `✓ fennec l'a essayé : ${tried} essai${tried > 1 ? "s" : ""} réussi${tried > 1 ? "s" : ""}` }));
  if (item.unverified?.length) {
    box.append(el("div", { className: "muted", textContent: "Non vérifié :" }),
      el("ul", { className: "warn" }, ...item.unverified.map((w) => el("li", {}, ...rich(w)))));
  }
  const s = shots(item);
  if (s) box.append(s);
  box.append(...facts(item));
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

// The address names the project and the request read in full:
// "#<project>/<request>", either part may be empty. The project is
// percent-encoded, so only a raw slash ever names the request.
const where = () => {
  const raw = location.hash.slice(1);
  const i = raw.lastIndexOf("/");
  return {
    path: i < 0 ? decodeURIComponent(raw) : decodeURIComponent(raw.slice(0, i)),
    id: i < 0 ? "" : raw.slice(i + 1),
  };
};
const hashOf = (path, id) => "#" + encodeURIComponent(path) + (id ? "/" + id : "");
let picked = where().id;

function renderProjects(projects) {
  const select = $("project");
  const current = select.value || where().path;
  // With several projects, none is preselected: a silent default sends a
  // request to the wrong project.
  // The empty choice is also the view of every project at once.
  const prompt = projects.length ? "Tous les projets" : "Aucun projet";
  const none = projects.length === 1 ? [] : [new Option(prompt, "")];
  select.replaceChildren(...none, ...projects.map((p) => new Option(p.name, p.path)));
  if (projects.some((p) => p.path === current)) select.value = current;
  // On load too: the address already names the project.
  if (select.value !== current || !$("chips").childElementCount) showChips();
  focusOn(select.value);
}

// Every project in view on the left: what waits for the human in each, and
// whether fennec works on it. A click is the same as choosing it above.
function renderSide(view) {
  const pick = (path) => {
    $("project").value = path;
    $("project").dispatchEvent(new Event("change"));
  };
  const entry = (button, path, items) => {
    const n = items.filter((i) => !path || i.path === path).length;
    button.setAttribute("aria-current", String($("project").value === path));
    button.onclick = () => pick(path);
    button.querySelectorAll(".count, .dot").forEach((e) => e.remove());
    if (path && view.running.some((i) => i.path === path)) button.append(el("span", { className: "dot", title: "fennec y travaille" }));
    if (n) button.append(el("span", { className: "count", title: "t'attend", textContent: n }));
    return button;
  };
  entry($("side-all"), "", view.todo);
  $("side-all").parentElement.hidden = view.projects.length < 2;
  $("side-projects").replaceChildren(...view.projects.map((p) => el("li", {},
    entry(el("button", { type: "button", className: "item", title: p.path }, el("span", { className: "name", textContent: p.name })), p.path, view.todo))));
  const page = PAGES.includes(where().path) ? where().path : "";
  $("open-team").setAttribute("aria-current", String(page === "team"));
  $("open-stats").setAttribute("aria-current", String(page === "stats"));
  $("open-settings").setAttribute("aria-current", String(page === "settings"));
}

// The chosen project is the one on screen: its requests only, kept in the
// address so a reload or a bookmark comes back to it.
function focusOn(path) {
  // A discussion or a page owns the address: the project's page must not take it back.
  if (["chat", "team", "stats", "settings"].includes(where().path)) return;
  document.body.classList.toggle("focused", !!path);
  if (path !== paged) showPage(path).catch((e) => ($("page-error").textContent = e.message));
  history.replaceState(null, "", path || picked ? hashOf(path, picked) : location.pathname);
}
const mine = (items) => {
  const path = $("project").value;
  return path ? items.filter((i) => i.path === path) : items;
};

const fill = (box, nodes, empty) =>
  box.replaceChildren(...(nodes.length ? nodes : [el("div", { className: "empty", textContent: empty })]));

// Each running line's time, ticking between refreshes.
const tick = () => document.querySelectorAll(".elapsed").forEach((e) => {
  e.textContent = duration(Date.now() - e.dataset.since * 1000);
});
setInterval(tick, 1000);

// The discussion list, and the chosen one read in full: the chat file is the
// exchange, so every refresh reads it again and redraws what moved.
let chatBox = null;
async function showChats() {
  document.body.classList.add("chats", "picked");
  $("chats").hidden = false;
  $("page").hidden = true;
  $("hint").hidden = true;
  for (const name of PAGES) $(name).hidden = true;
  $("side-chats").setAttribute("aria-current", "true");
  let chats;
  try {
    chats = await api("/api/chats");
  } catch (e) {
    $("chat-list").replaceChildren(el("div", { className: "empty", textContent: e.message }));
    return;
  }
  $("chat-list").replaceChildren(...(chats.length ? chats.map(chatRow)
    : [el("div", { className: "empty", textContent: "Aucune discussion : « Nouvelle discussion » pour en démarrer." })]));
  const id = where().id;
  if (!id) {
    chatBox = null;
    detailed = "";
    return $("pick").replaceChildren();
  }
  let c;
  try {
    c = await api("/api/chat", { id });
  } catch (e) {
    chatBox = null;
    detailed = "";
    return $("pick").replaceChildren(el("div", { className: "card" }, el("div", { className: "bad", textContent: explain(e.message) })));
  }
  if (where().id !== id) return; // the page moved on while reading
  showChat(c);
}

function chatRow(c) {
  const a = el("a", { className: "row", href: "#chat/" + c.id },
    line({ id: c.id, project: c.folder.split("/").pop(), title: c.title, since: c.since },
      c.status === "thinking" ? "répond…" : c.status === "waiting" ? "une question" : "prête",
      c.status === "thinking"));
  a.setAttribute("aria-current", String(c.id === where().id));
  return a;
}

function showChat(c) {
  const thinking = c.status === "thinking";
  const fresh = !chatBox || chatBox.id !== c.id;
  if (fresh) {
    const composer = chatComposer(c.id);
    const head = el("div", { className: "chat-head" });
    const thread = el("div", { className: "thread" });
    const aside = el("aside", { className: "chat-side" });
    chatBox = { id: c.id, composer, head, thread, aside, asideKey: "" };
    detailed = "";
    $("pick").replaceChildren(el("div", { className: "split" },
      el("div", { className: "card chat" }, head, thread, composer.node,
        el("div", { className: "error", id: "chat-error", role: "alert" })),
      aside));
  }
  const box = chatBox;
  box.head.replaceChildren(
    el("div", { className: "meta", style: `--c: ${c.project ? "var(--ok)" : "var(--muted)"}` },
      el("span", { className: "tag", textContent: c.project ? "Projet" : "Dossier" }),
      el("span", { className: "proj", textContent: c.folder })),
    el("div", { className: "title", textContent: chatTitle(c) }));
  // What was opened stays open as new messages come in; the page follows the
  // turn to the bottom, unless the person scrolled up to read.
  const detail = $("detail");
  const follow = fresh || detail.scrollHeight - detail.scrollTop - detail.clientHeight < 80;
  const opened = [...box.thread.querySelectorAll("details")].map((d) => d.open);
  box.thread.replaceChildren(...chatThread(c));
  if (thinking) box.thread.append(el("div", { className: "thinking" }, el("span", { className: "dot" }), "fennec répond…"));
  // A turn that waits on the person is the last thing on screen: on top of
  // the exchange, it would read as a turn that stopped answering.
  if (c.status === "waiting" && c.pending) {
    box.thread.append(chatQuestion(c));
    box.thread.append(el("div", { className: "thinking" }, el("span", { className: "dot" }), "fennec attend ta réponse"));
  }
  if (c.error) box.thread.append(el("div", { className: "bad", textContent: explain(c.error) }));
  box.thread.querySelectorAll("details").forEach((d, i) => { d.open = opened[i] ?? false; });
  const busy = thinking || c.status === "waiting";
  box.composer.send.disabled = busy;
  box.composer.stop.hidden = !busy;
  const asideKey = JSON.stringify([c.skills, c.stats, c.project, c.grants]);
  if (box.asideKey !== asideKey) {
    box.asideKey = asideKey;
    box.aside.replaceChildren(...chatAside(c));
  }
  if (follow) detail.scrollTop = detail.scrollHeight;
}

// A call named a path outside the folder the person chose: fennec asks
// before running it. "Ouvrir" opens that path's folder for the chat; the
// refusal covers this call only.
function chatQuestion(c) {
  const p = c.pending ?? {};
  const open = el("button", { type: "button", className: "primary", textContent: "Ouvrir le dossier" });
  const refuse = el("button", { type: "button", className: "quiet", textContent: "Refuser" });
  const decide = async (allow) => {
    $("chat-error").textContent = "";
    try {
      await api("/api/chat/decide", { id: c.id, allow });
    } catch (e) {
      $("chat-error").textContent = e.message;
    }
  };
  open.onclick = () => decide(true);
  refuse.onclick = () => decide(false);
  return el("div", { className: "ask" },
    el("span", { className: "muted", textContent: "fennec demande" }),
    el("code", { title: p.path, textContent: p.path }),
    el("span", { className: "muted", title: p.directory, textContent: p.directory }),
    open, refuse);
}

function chatTitle(c) {
  const first = (c.messages ?? []).find((m) => m.role === "user")?.content;
  if (!first) return "Nouvelle discussion";
  const line = (first.split("\n").find((l) => l.trim()) ?? "").trim();
  return line.length > 60 ? line.slice(0, 60) + "…" : line;
}

// The exchange: the human's words in bubbles, fennec's in Markdown, the tool
// calls folded, each with what it brought back.
function chatThread(c) {
  const calls = new Map();
  for (const m of c.messages ?? []) for (const t of m.tool_calls ?? []) calls.set(t.id, t);
  const results = new Map();
  for (const m of c.messages ?? []) if (m.role === "tool") results.set(m.tool_call_id, m.content);
  const nodes = [];
  for (const m of c.messages ?? []) {
    if (m.role === "tool") continue;
    if (m.role === "user") {
      nodes.push(el("div", { className: "said human" },
        el("span", { className: "label", textContent: "Toi" }), ...rich(m.content)));
      continue;
    }
    const box = el("div", { className: "said" }, el("span", { className: "label", textContent: "fennec" }));
    if (m.content) box.append(...md(m.content));
    for (const t of m.tool_calls ?? []) box.append(toolCall(t, results.get(t.id)));
    nodes.push(box);
  }
  return nodes.length ? nodes : [el("div", { className: "muted", textContent: "Écris ce qui t'intéresse : fennec lit le dossier et répond." })];
}

function toolCall(t, result) {
  const args = t.function?.arguments ?? "";
  const head = `→ ${t.function?.name ?? "?"} ${String(args).replace(/\s+/g, " ").slice(0, 110)}`;
  return el("details", { className: "message" },
    el("summary", {}, head),
    args ? el("pre", { textContent: args }) : null,
    el("pre", { textContent: result ?? "…" }));
}

function chatComposer(id) {
  const text = el("textarea", { placeholder: "Ton message", ariaLabel: "Ton message" });
  const stop = el("button", { type: "button", className: "quiet", textContent: "Arrêter", hidden: true });
  const send = el("button", { type: "button", className: "primary", textContent: "Envoyer" });
  const say = async () => {
    if (!text.value.trim()) return;
    try {
      await api("/api/chat/say", { id, text: text.value });
      text.value = "";
      text.focus();
    } catch (e) {
      $("chat-error").textContent = e.message;
    }
  };
  send.onclick = say;
  text.onkeydown = (e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); say(); } };
  stop.onclick = async () => {
    $("chat-error").textContent = "";
    try { await api("/api/chat/stop", { id }); }
    catch (e) { $("chat-error").textContent = e.message; }
  };
  return { node: el("div", { className: "chat-composer" }, text,
    el("div", { className: "chat-actions" }, stop, el("span", { className: "spacer" }), send)),
    send, stop, text };
}

function chatAside(c) {
  const row = (s, on) => {
    const b = el("button", { type: "button", textContent: on ? "Ajouter" : "Retirer" });
    b.onclick = async () => {
      $("chat-error").textContent = "";
      try {
        await api("/api/chat/skill", { id: c.id, name: s.name, on });
        refresh(true);
      } catch (e) {
        $("chat-error").textContent = e.message;
      }
    };
    return el("li", { title: `${s.description}\n${s.from}` },
      el("span", { className: "what", title: s.description }, s.name), b);
  };
  const chosen = (c.skills ?? []).filter((s) => s.chosen);
  const rest = (c.skills ?? []).filter((s) => !s.chosen);
  const all = chosen.map((s) => row(s, false)).concat(rest.slice(0, 8).map((s) => row(s, true)));
  const nodes = [el("h2", { textContent: "Skills" }),
    all.length ? el("ul", {}, ...all) : el("p", { className: "muted", textContent: "Aucun skill sur cette machine." })];
  // Folders the person opened for this chat: a refusal was not, an opening
  // stays until taken back.
  const grants = c.grants ?? [];
  if (grants.length) {
    const ul = el("ul");
    for (const g of grants) {
      const b = el("button", { type: "button", textContent: "Retirer" });
      b.onclick = async () => {
        $("chat-error").textContent = "";
        try {
          await api("/api/chat/grant", { id: c.id, directory: g });
          refresh(true);
        } catch (e) {
          $("chat-error").textContent = e.message;
        }
      };
      ul.append(el("li", {}, el("span", { className: "what", title: g }, g), b));
    }
    nodes.push(el("h2", { textContent: "Dossiers ouverts" }), ul);
  }
  if (c.stats?.turns) nodes.push(el("h2", { textContent: "Coût" }), metrics(c.stats));
  const button = el("button", { type: "button", className: "convert",
    textContent: c.project ? "En faire une demande" : "En faire un projet" });
  button.onclick = async () => {
    $("chat-error").textContent = "";
    try {
      if (c.project) {
        // The exchange becomes the request's opening, kept in the backlog.
        const made = await api("/api/chat/request", { id: c.id });
        location.hash = hashOf(c.folder, made.id);
      } else {
        openPicker("project", "En faire un projet", c.folder);
      }
    } catch (e) {
      $("chat-error").textContent = explain(e.message);
    }
  };
  nodes.push(el("h2", { textContent: c.project ? "Demande" : "Projet" }), button);
  return nodes;
}

let shown = "";
async function refresh(force) {
  // Never wipe what the human is typing in the request read in full; the
  // rest keeps up. (Holding the whole page whenever focus sat in that pane
  // froze it after a mere click on a tab.)
  const typing = !force && ([...document.querySelectorAll("#detail textarea, #detail input")].some((t) => t.value !== t.defaultValue)
    || document.activeElement?.matches("#detail textarea, #detail input"));
  try {
    const view = await api("/api/view");
    const json = JSON.stringify(view);
    const changed = force || json !== shown;
    // A discussion takes over the page: its file moves under it, so it is
    // read on every refresh, whatever the requests do.
    if (where().path === "chat") {
      if (changed) {
        shown = typing ? "" : json;
        renderProjects(view.projects);
        renderSide(view);
      }
      showChats();
      return;
    }
    if (!changed) {
      document.body.classList.remove("chats");
      $("chats").hidden = true;
      $("side-chats").removeAttribute("aria-current");
      return;
    }
    // Held while typing: drawn once the field is left, even if nothing moves.
    shown = typing ? "" : json;
    $("error").textContent = "";
    document.body.classList.remove("chats");
    $("chats").hidden = true;
    $("side-chats").removeAttribute("aria-current");
    renderProjects(view.projects);
    renderSide(view);
    const todo = mine(view.todo), running = mine(view.running);
    const row = (item, node) => {
      const a = el("a", { className: "row", href: hashOf($("project").value, item.id) }, node);
      a.setAttribute("aria-current", String(item.id === picked));
      return a;
    };
    fill($("todo"), todo.map((i) => row(i, line(i, TAGS[i.kind][0]))), "Rien ne t'attend.");
    $("n-todo").textContent = todo.length || "";
    // Seen from another tab: what waits for the human, in the tab's title.
    const others = view.todo.length - todo.length;
    $("n-others").textContent = others ? `${others} autre${others > 1 ? "s" : ""} t'attend${others > 1 ? "ent" : ""} ailleurs` : "";
    const name = $("project").selectedOptions[0]?.value ? $("project").selectedOptions[0].text : "fennec";
    document.title = todo.length ? `(${todo.length}) ${name}` : name;
    fill($("running"), running.map((i) => row(i, line(i, i.step, true))), "Rien en cours.");
    // The backlog: only there when it holds something.
    const later = mine(view.later ?? []);
    $("later-box").hidden = !later.length;
    $("n-later").textContent = later.length || "";
    fill($("later"), later.map((i) => row(i, line(i, "à lancer"))), "");
    const history = mine(view.history);
    fill($("history"), history.map((i) => row(i, line(i, pastState(i), false))), "Rien de terminé pour l'instant.");
    if (!typing) showDetail(todo.map((i) => [i, "todo"]).concat(later.map((i) => [i, "todo"]), running.map((i) => [i, "run"]), history.map((i) => [i, "past"]))
      .find(([i]) => i.id === picked));
    tick();
    // How it works, until the first request: the page alone must explain itself.
    $("how").hidden = view.todo.length + view.running.length + view.history.length > 0;
    // Its figures move as its requests do.
    if (paged && !force) showPage(paged).catch(() => {});
  } catch (e) {
    $("error").textContent = "Serveur injoignable : " + e.message;
  }
}

// The right pane: the chosen request in full, a page the address names,
// or else the project's page. Drawn again only when the request changed,
// so what the human opened in it stays open.
const PAGES = ["team", "stats", "settings"];
let detailed = "";
let shownPageKey = "";
function showDetail(found) {
  const page = PAGES.includes(where().path) ? where().path : "";
  document.body.classList.toggle("picked", !!found || !!page);
  $("page").hidden = !!found || !!page || !paged;
  $("hint").hidden = !!found || !!page || !!paged;
  for (const name of PAGES) $(name).hidden = name !== page;
  if (!found) {
    detailed = "";
    chatBox = null;
    if (page) {
      // A page stays drawn while its fields are filled: redraw only on entry.
      const key = page + "|" + $("project").value;
      if (key !== shownPageKey) {
        shownPageKey = key;
        if (page === "team") showTeamPage();
        if (page === "stats") showStats().catch((e) => ($("stats-error").textContent = e.message));
        if (page === "settings") showSettingsPage();
      }
      return $("pick").replaceChildren();
    }
    shownPageKey = "";
    return $("pick").replaceChildren();
  }
  const [item, kind] = found;
  // Under way, its figures move at each call: the journal would restart with them.
  const key = kind + (kind === "run" ? item.id + item.step : JSON.stringify(item));
  if (key === detailed) return;
  detailed = key;
  const node = kind === "todo" ? card(item) : kind === "run" ? underway(item) : past(item);
  if (node.tagName === "DETAILS") node.open = true;
  const decide = decision(node, item, kind);
  // Nothing done yet on a request for later: no tabs to read.
  const main = decide && item.kind !== "later" ? tabbed(node, item) : node;
  // A committed request has nothing left to decide: tabs, but no panel.
  const panel = decide?.querySelector("button") ? decide : null;
  const facts = [...node.querySelectorAll(".facts")];
  chatBox = null;
  $("pick").replaceChildren(panel || facts.length
    ? el("div", { className: "split" }, main, el("aside", {}, panel,
      facts.length ? el("h2", { textContent: "Coût et isolement" }) : null, ...facts))
    : main);
}

// Lucide-style strokes (24 × 24), drawn by CSS as a mask in the button's colour.
const ICONS = {
  commit: '<circle cx="12" cy="12" r="3"/><path d="M3 12h6M15 12h6"/>',
  page: '<path d="M15 3h6v6M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  diff: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M9 10h6M12 7v6M9 17h6"/>',
  code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/>',
  fix: '<path d="M21.2 6.8a2.8 2.8 0 0 0-4-4L3.8 16.2a2 2 0 0 0-.5.8L2 21.4a.5.5 0 0 0 .6.6l4.4-1.3a2 2 0 0 0 .8-.5z"/>',
  pass: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8M10 12h4"/>',
  journal: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
  undo: '<path d="M3 7v6h6"/><path d="M21 17a9 9 0 0 0-15-6.7L3 13"/>',
  resume: '<path d="M6 3l14 9-14 9z"/>',
  trash: '<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/>',
  save: '<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/>',
  retry: '<path d="M3 12a9 9 0 0 1 15.7-6L21 8M21 3v5h-5M21 12a9 9 0 0 1-15.7 6L3 16M8 16H3v5"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  close: '<path d="M18 6 6 18M6 6l12 12"/>',
  send: '<path d="m22 2-7 20-4-9-9-4z"/><path d="M22 2 11 13"/>',
  clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
  chat: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
  team: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.9M16 3.1a4 4 0 0 1 0 7.8"/>',
  stats: '<path d="M3 3v18h18"/><path d="M18 17V9M13 17V5M8 17v-3"/>',
  gear: '<path d="M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.3a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.5a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.3a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.3a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.5a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.3a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
  folder: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>',
  inbox: '<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.5 5.1 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.5-6.9A2 2 0 0 0 16.8 4H7.2a2 2 0 0 0-1.7 1.1z"/>',
  work: '<path d="M22 12h-4l-3 9L9 3l-3 9H2"/>',
  history: '<path d="M3 12a9 9 0 1 0 9-9 9.8 9.8 0 0 0-6.7 2.7L3 8"/><path d="M3 3v5h5M12 7v5l4 2"/>',
  shield: '<path d="M20 13c0 5-3.5 7.5-7.7 9a1 1 0 0 1-.7 0C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.2-2.7a1.2 1.2 0 0 1 1.6 0C14.5 3.8 17 5 19 5a1 1 0 0 1 1 1z"/>',
  rule: '<path d="M9 11l3 3L22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/>',
};
const ICON_OF = [[/^(Commit|Pousser|Préparer)/, "commit"], [/^Ouvrir la page/, "page"], [/changements/, "diff"],
  [/^Ouvrir le code/, "code"], [/^Corriger|^Envoyer/, "fix"], [/^Passer/, "pass"], [/^Journal/, "journal"],
  [/^Annuler le travail|^Abandonner/, "undo"], [/^Reprendre|^Lancer/, "resume"], [/^Supprimer/, "trash"], [/^Enregistrer le texte/, "save"], [/^Réessayer/, "retry"]];
function iconize(button) {
  const name = ICON_OF.find(([re]) => re.test(button.textContent))?.[1];
  if (name) setIcon(button, name);
}
function setIcon(button, name) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`;
  button.style.setProperty("--icon", `url("data:image/svg+xml,${encodeURIComponent(svg)}")`);
  button.classList.add("ic");
}
document.querySelectorAll("[data-icon]").forEach((node) => setIcon(node, node.dataset.icon));

// What was asked, done and proven first; the exact change, the exchange with
// the model and the rest behind tabs above it, loaded when opened.
let tabNow = "Résumé";
function tabbed(node, item) {
  const more = [...node.querySelectorAll("details")].find((d) => d.firstChild?.textContent === "Détails");
  more?.remove();
  const panes = [["Résumé", async () => [node]], ["Changements", () => changes(item)], ["Journal", () => journal(item)]];
  if (more) panes.push(["Détails", async () => [...more.childNodes].slice(1)]);
  const panel = el("div", { className: "tab-panel" });
  const shown = new Map();
  const bar = el("div", { className: "tabs", role: "tablist" });
  const open = async (name) => {
    tabNow = name;
    bar.querySelectorAll("button").forEach((b) => b.setAttribute("aria-selected", String(b.textContent === name)));
    if (!shown.has(name)) {
      shown.set(name, el("div", {}, el("div", { className: "muted", textContent: "Je charge…" })));
      const box = shown.get(name);
      panes.find(([n]) => n === name)[1]().then((nodes) => box.replaceChildren(...nodes))
        .catch((e) => box.replaceChildren(el("div", { className: "bad", textContent: e.message })));
    }
    panel.replaceChildren(shown.get(name));
  };
  bar.append(...panes.map(([name]) => {
    const b = el("button", { type: "button", role: "tab", textContent: name });
    b.onclick = () => open(name);
    return b;
  }));
  open(panes.some(([n]) => n === tabNow) ? tabNow : "Résumé");
  return el("div", {}, bar, panel);
}

// What the human decides on a request, taken out of its card: on the right,
// above what it cost, the main button first. A conversation keeps its
// buttons under what was said.
function decision(node, item, kind) {
  if (kind === "run" || (kind === "todo" && !["ready", "failed", "later"].includes(item.kind))) return null;
  const parts = kind === "past" ? [node.querySelector(".past-actions")]
    : [":scope > .reply", ":scope > .actions", ":scope > .error"].map((q) => node.querySelector(q));
  const box = el("div", { className: "decide" }, ...parts.filter(Boolean));
  // These two are tabs over the request now.
  box.querySelectorAll("button").forEach((b) => /changements|^Journal/.test(b.textContent) && b.remove());
  // The correction is written where "Corriger" was clicked.
  const fix = [...box.querySelectorAll("button")].find((b) => b.textContent === "Corriger");
  const reply = box.querySelector(":scope > .reply");
  if (fix && reply) fix.before(reply);
  box.querySelectorAll("button").forEach(iconize);
  return box;
}

window.addEventListener("hashchange", () => {
  const w = where();
  picked = w.id;
  if (w.path !== $("project").value && [...$("project").options].some((o) => o.value === w.path)) {
    $("project").value = w.path;
    showChips();
    focusOn(w.path);
  }
  $("detail").scrollTop = 0;
  refresh(true);
});
$("back").onclick = () => (location.hash = hashOf($("project").value, ""));

function compose() {
  $("compose-error").textContent = "";
  $("compose").showModal();
  $("text").focus();
}
$("compose-open").onclick = compose;
$("compose-close").onclick = () => $("compose").close();

// Written now, worked on later: kept in the project's backlog, nothing starts.
$("later-add").onclick = async () => {
  $("compose-error").textContent = "";
  try {
    await api("/api/requests", { project: $("project").value, text: $("text").value, later: true });
    $("text").value = "";
    $("compose").close();
    refresh(true);
  } catch (e) {
    $("compose-error").textContent = e.message;
  }
};

$("new-request").addEventListener("submit", async (event) => {
  event.preventDefault();
  $("compose-error").textContent = "";
  try {
    const off = [...document.querySelectorAll("#chips button[aria-pressed=false]")].map((b) => b.dataset.key);
    const made = await api("/api/requests", { project: $("project").value, text: $("text").value, off });
    $("text").value = "";
    $("compose").close();
    // What was just asked is what the human watches next.
    location.hash = hashOf($("project").value, made.id);
  } catch (e) {
    $("compose-error").textContent = e.message;
  }
});

// Folder picker: the server lists folders, since a page cannot read paths.
// It adds a project, or starts a discussion in the chosen folder.
let pickerPath = "";
let pickerMode = "project";
async function browse(path) {
  const data = await api("/api/folders", { path });
  pickerPath = data.path;
  // An empty folder starts a new project; files without git are not ours to commit.
  $("picker-path").textContent = data.path + (data.home ? " · ton dossier personnel : choisis un dossier dedans"
    : data.repo ? " · dépôt git"
    : data.empty ? " · dossier vide" : " · pas un dépôt git : choisis un dépôt ou un dossier vide");
  if (pickerMode === "chat") {
    // A discussion works in any folder inside the home, git or not.
    $("picker-choose").disabled = data.home;
    $("picker-choose").textContent = "Discuter ici";
  } else {
    $("picker-choose").disabled = data.home || (!data.repo && !data.empty);
    $("picker-choose").textContent = data.repo || !data.empty ? "Ajouter ce dossier" : "Créer le projet ici";
  }
  const entries = (data.parent ? [{ name: "..", path: data.parent }] : []).concat(data.children);
  $("picker-list").replaceChildren(...entries.map((entry) => {
    const button = el("button", { type: "button", textContent: entry.name });
    if (entry.repo) button.append(el("span", { className: "repo", textContent: "git" }));
    button.onclick = () => browse(entry.path).catch((e) => ($("picker-path").textContent = e.message));
    return el("li", {}, button);
  }));
}

// A new project often has no folder yet: made here, then opened, empty.
$("picker-new").onsubmit = async (e) => {
  e.preventDefault();
  try {
    const data = await api("/api/folders", { path: pickerPath, create: $("picker-new-name").value });
    $("picker-new-name").value = "";
    await browse(data.path);
  } catch (err) {
    $("picker-path").textContent = err.message;
  }
};

async function openPicker(mode, title, path) {
  pickerMode = mode;
  $("picker-title").textContent = title;
  $("picker-place-wrap").hidden = mode !== "project";
  try {
    await browse(path ?? "");
  } catch (e) {
    $("picker-path").textContent = e.message;
  }
  $("picker").showModal();
}
$("add-project").onclick = () => {
  // Next to the projects already there: new ones usually sit beside them.
  const last = $("project").value || [...$("project").options].at(-1)?.value || "";
  openPicker("project", "Ajouter un projet", pickerPath || last.replace(/\/[^/]+$/, ""));
};
$("new-chat").onclick = () => openPicker("chat", "Nouvelle discussion", "");
$("side-chats").onclick = () => {
  location.hash === "#chat" ? refresh(true) : (location.hash = "#chat");
};
$("picker-go").onsubmit = (e) => {
  e.preventDefault();
  browse($("picker-input").value.trim()).catch(() => ($("picker-path").textContent = "Dossier introuvable : " + $("picker-input").value));
};
$("picker-cancel").onclick = () => $("picker").close();
$("picker-choose").onclick = async () => {
  try {
    if (pickerMode === "chat") {
      // A folder becomes a discussion: the address names it, the page follows.
      const made = await api("/api/chats", { folder: pickerPath });
      $("picker").close();
      location.hash = "#chat/" + made.id;
      return;
    }
    await api("/api/projects", { path: pickerPath, place: $("picker-place").value });
    $("picker").close();
    // A project just added is the one the next request is for.
    $("project").append(new Option("", pickerPath));
    $("project").value = pickerPath;
    // A new project is there to be asked something.
    refresh(true);
    compose();
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

// The team of the chosen project, as a page the address names: #team.
async function showTeamPage() {
  $("team-role").hidden = $("team-save").hidden = true;
  $("team-error").textContent = "";
  const project = $("project").value;
  if (!project) {
    $("team-title").textContent = "Équipe";
    $("team-body").hidden = true;
    $("team-hint").hidden = false;
    $("team-name").replaceChildren();
    $("team-roles").replaceChildren();
    $("team-catalog").replaceChildren();
    return;
  }
  $("team-title").textContent = `Équipe — ${$("project").selectedOptions[0].textContent}`;
  $("team-body").hidden = false;
  $("team-hint").hidden = true;
  try {
    renderTeam(await api("/api/team", { project }));
  } catch (e) {
    $("team-error").textContent = e.message;
  }
}
$("open-team").onclick = () => {
  location.hash === "#team" ? refresh(true) : (location.hash = "#team");
};
// Another team starts whole: what was left out belonged to the old one.
$("team-name").onchange = () => teamCall("/api/team/choose", { team: $("team-name").value, remove: [] });
$("team-add").onsubmit = (e) => {
  e.preventDefault();
  if ($("team-catalog").value) teamCall("/api/team/add", { role: $("team-catalog").value });
};

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
async function showSettingsPage() {
  try {
    const s = await api("/api/settings", {});
    chosen = s.models;
    $("settings-url").value = s.base_url;
    $("settings-tokens").value = s.max_tokens;
    $("settings-key").value = "";
    $("settings-key").placeholder = s.has_key ? "enregistrée (laisse vide pour la garder)" : "";
    $("settings-env").hidden = !s.env.length;
    $("settings-env").textContent = `Au lancement, ${s.env.join(", ")} passe avant ces réglages.`;
    $("settings-error").textContent = "";
    listModels();
    showSkillList();
  } catch (e) {
    $("settings-error").textContent = e.message;
  }
}
$("open-settings").onclick = () => {
  location.hash === "#settings" ? refresh(true) : (location.hash = "#settings");
};
$("settings-test").onclick = () => {
  // An empty list (server down, or still asking) says nothing about the choice.
  const boxes = [...document.querySelectorAll("#settings-models input")];
  if (boxes.length) chosen = boxes.filter((b) => b.checked).map((b) => b.value);
  listModels();
};
$("settings-cancel").onclick = () => showSettingsPage();
$("settings-form").onsubmit = async (e) => {
  e.preventDefault();
  const boxes = [...document.querySelectorAll("#settings-models input")];
  // Models the server did not list (it was down) are kept as they were.
  const models = boxes.length ? boxes.filter((b) => b.checked).map((b) => b.value) : chosen;
  try {
    await api("/api/settings/save", { base_url: $("settings-url").value, api_key: $("settings-key").value, models,
      max_tokens: Number($("settings-tokens").value) });
    $("settings-key").value = "";
  } catch (err) {
    $("settings-error").textContent = err.message;
  }
};

// Stats: what every request cost, per model and per project, and where
// fennec works in each project, which can be changed here.
const PLACES = { here: "Dans le dossier", apart: "À part, à côté" };
const cell = (text, className = "") => el("td", { textContent: text, className });

async function showStats() {
  const v = await api("/api/stats");
  const a = v.all;
  const tile = (value, label) => el("div", { className: "tile" }, el("b", { textContent: value }), el("span", { textContent: label }));
  $("stats-tiles").replaceChildren(
    tile(number(a.requests), "demandes"),
    tile(duration(a.ms ?? 0), "de modèle"),
    tile(compact(a.sent), `tokens envoyés · ${percent(a.cached, a.sent)} % en cache`),
    tile(compact(a.received), `tokens générés · ${speed(a) || "—"}`));
  const fastest = Math.max(1, ...v.models.map(tps));
  $("stats-models").replaceChildren(...v.models.map((m) => el("tr", {},
    el("th", { textContent: m.name }), cell(number(m.turns)), cell(compact(m.sent)),
    el("td", {}, el("span", { className: "bar", style: `--p:${percent(m.cached, m.sent)}%` }), `${percent(m.cached, m.sent)} %`),
    cell(compact(m.received)),
    el("td", {}, el("span", { className: "bar", style: `--p:${percent(tps(m), fastest)}%` }), m.ms ? `${tps(m)}/s` : "—"),
    cell(duration(m.ms ?? 0)))));
  $("stats-stages").replaceChildren(...v.stages.sort((x, y) => stageRank(x.name) - stageRank(y.name)).map((t) => el("tr", {},
    el("th", { textContent: t.name }), cell(number(t.turns)), cell(duration(t.ms ?? 0)),
    cell(`${percent(t.ms, a.ms)} %`), cell(compact(t.sent)), cell(compact(t.received)), cell(t.ms ? `${tps(t)}/s` : "—"))));
  $("stats-projects").replaceChildren(...v.projects.map((p) => {
    const place = el("select", { ariaLabel: `Où fennec travaille dans ${p.name}` },
      ...Object.entries(PLACES).map(([k, label]) => new Option(label, k)));
    if (!PLACES[p.place]) place.append(new Option(p.place, p.place));
    place.value = p.place;
    place.onchange = async () => {
      $("stats-error").textContent = "";
      try {
        await api("/api/projects", { path: p.path, place: place.value });
      } catch (e) {
        $("stats-error").textContent = e.message;
        place.value = p.place;
      }
    };
    return el("tr", {}, el("th", { textContent: p.name, title: p.path }), el("td", {}, place),
      cell(number(p.requests)), cell(number(p.landed)), cell(number(p.failed), p.failed ? "bad" : ""),
      cell(number(p.failed_attempts)), cell(duration(p.ms ?? 0)), cell(compact(p.sent)));
  }));
}
// One project's page, above its requests: read when it is chosen, not at each refresh.
let paged = "";
async function showPage(path) {
  paged = path;
  if (!path) return;
  $("page-error").textContent = "";
  $("page-name").textContent = path.split("/").pop();
  $("page-path").textContent = path;
  const [stats, rules] = await Promise.all([api("/api/stats"), api("/api/rules", { project: path })]);
  if (paged !== path) return;
  const p = stats.projects.find((x) => x.path === path) ?? {};
  const tile = (value, label, tone = "") => el("div", { className: `tile ${tone}` }, el("b", { textContent: value }), el("span", { textContent: label }));
  $("page-tiles").replaceChildren(
    tile(number(p.requests), "demandes"), tile(number(p.landed), "commitées"),
    tile(number(p.failed), "échouées", p.failed ? "bad" : ""),
    tile(number(p.failed_attempts), "essais ratés annulés"),
    tile(duration(p.ms ?? 0), "de modèle"), tile(compact(p.sent), "tokens envoyés"));
  const place = $("page-place");
  place.replaceChildren(...Object.entries(PLACES).map(([k, label]) => new Option(label, k)));
  if (p.place && !PLACES[p.place]) place.append(new Option(p.place, p.place));
  place.value = p.place ?? "here";
  place.onchange = () => api("/api/projects", { path, place: place.value })
    .catch((e) => { $("page-error").textContent = e.message; place.value = p.place; });
  showRules(path, rules);
}

// The skills of the machine, valid for every project: chosen from the
// settings, the team sees the name and goal of each enabled skill and reads
// it when the request needs it.
let skillList = [];
async function showSkillList() {
  try {
    skillList = await api("/api/skills", {});
    drawSkillList();
  } catch (e) {
    $("skills-error").textContent = e.message;
  }
}
function drawSkillList() {
  $("skills-error").textContent = "";
  const q = $("skill-search").value.trim().toLowerCase();
  const list = skillList.filter((s) => !q || (s.name + " " + s.description).toLowerCase().includes(q));
  $("skills-list").replaceChildren(
    ...list.slice(0, 12).map((s) => {
      const b = el("button", { type: "button", textContent: s.chosen ? "Retirer" : "Ajouter" });
      b.onclick = async () => {
        try {
          skillList = await api("/api/skills", { name: s.name, on: !s.chosen });
          drawSkillList();
        } catch (e) {
          $("skills-error").textContent = e.message;
        }
      };
      return el("li", { title: `${s.description}\n${s.from}` }, el("strong", { textContent: s.name }),
        el("span", { className: "what", textContent: s.description }), b);
    }),
    ...(list.length ? [] : [el("li", { className: "muted", textContent: skillList.length
      ? "Aucun skill ne correspond à la recherche." : "Aucun skill sur cette machine." })]));
}
$("skill-search").oninput = drawSkillList;

function showRules(path, rules) {
  const box = $("page-rules");
  box.querySelector(".summary").textContent = rules.length ? `Règles suivies (${rules.length})` : "Aucune règle pour l'instant";
  box.querySelector("ul").replaceChildren(...rules.map((r) => {
    const remove = el("button", { type: "button", className: "quiet", ariaLabel: "Retirer la règle", title: "Retirer la règle" });
    setIcon(remove, "close");
    remove.onclick = () => changeRules(path, { change: "remove", scope: r.scope, text: r.text });
    return el("li", {}, ...rich(r.text),
      el("span", { className: "muted", textContent: r.scope === "user" ? " — tous tes projets" : " — ce projet" }), remove);
  }));
}

async function changeRules(path, change) {
  try {
    showRules(path, await api("/api/rules", { project: path, ...change }));
    $("page-error").textContent = "";
    return true;
  } catch (e) {
    $("page-error").textContent = explain(e.message);
    return false;
  }
}
$("rule-form").onsubmit = async (e) => {
  e.preventDefault();
  if (await changeRules(paged, { change: "add", scope: $("rule-scope").value, text: $("rule-text").value })) $("rule-text").value = "";
};
$("page-team").onclick = () => $("open-team").click();
$("page-open").onclick = async () => {
  try {
    const c = await api("/api/open-project", { project: paged });
    $("page-error").textContent = c.opened ? "" : `VS Code introuvable : ouvre ${c.folder} dans ton éditeur.`;
  } catch (e) {
    $("page-error").textContent = e.message;
  }
};

$("open-stats").onclick = () => {
  location.hash === "#stats" ? refresh(true) : (location.hash = "#stats");
};

// The project's roles, as chips under the request: one click leaves a role
// out of this request only (no QA for a one-line page). The team stays as is.
async function showChips() {
  const project = $("project").value;
  if (!project) return $("chips").replaceChildren();
  try {
    const v = await api("/api/team", { project });
    const roles = v.roles.filter((r) => !r.out);
    $("chips").replaceChildren(el("span", { className: "muted", textContent: "Équipe pour cette demande :" }),
      ...roles.map((r) => {
        const chip = el("button", { type: "button", className: `rtag ${r.stage} chip`, textContent: r.name,
          title: "Clique pour faire sans ce rôle sur cette demande" });
        chip.dataset.key = r.key;
        chip.setAttribute("aria-pressed", "true");
        chip.onclick = () => chip.setAttribute("aria-pressed", chip.getAttribute("aria-pressed") === "true" ? "false" : "true");
        return chip;
      }));
  } catch {
    $("chips").replaceChildren();
  }
}
$("n-others").onclick = () => {
  $("project").value = "";
  $("project").dispatchEvent(new Event("change"));
};
$("project").addEventListener("change", () => {
  // Choosing a project while a discussion is open takes its address back;
  // the address doing it keeps the page honest.
  if (where().path === "chat") {
    location.hash = hashOf($("project").value, "");
    return;
  }
  picked = "";
  showChips();
  focusOn($("project").value);
  refresh(true);
});
