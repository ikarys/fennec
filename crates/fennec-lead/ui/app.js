// The single view. Polls the server; a page left open stays current.
// A card shows only what the decision needs; the rest waits behind "Details".

const $ = (id) => document.getElementById(id);
const el = (tag, props = {}, ...children) => {
  const node = Object.assign(document.createElement(tag), props);
  node.append(...children.filter((c) => c !== null && c !== undefined && c !== false));
  return node;
};

// The interface's language, chosen in the settings: English unless the
// person picked another. Text is written in English in this file; `t` gives
// it in the chosen language (`FR` in fr.js), `{0}`, `{1}`... filled from
// `args`. A display-only preference kept in localStorage, like the theme.
const LANG_KEY = "fennec.lang";
const LANGS = [["en", "English"], ["fr", "Français"]];
function lang() {
  let l;
  try { l = localStorage.getItem(LANG_KEY); } catch { l = null; }
  return LANGS.some(([id]) => id === l) ? l : "en";
}
function t(text, ...args) {
  let said = (lang() === "fr" && typeof FR !== "undefined" && FR[text]) || text;
  // A translation that depends on the count: [one, many], the count first.
  if (Array.isArray(said)) said = said[Number(args[0]) > 1 ? 1 : 0];
  return said.replace(/\{(\d+)\}/g, (_, i) => String(args[i] ?? ""));
}
// The page's own text, written in English in index.html: elements marked
// `data-t` and every placeholder, title and aria-label.
function translatePage(root = document) {
  if (lang() === "en") return;
  for (const node of root.querySelectorAll("[data-t]")) node.textContent = t(node.textContent.trim().replace(/\s+/g, " "));
  for (const attr of ["placeholder", "title", "aria-label"]) {
    for (const node of root.querySelectorAll(`[${attr}]`)) node.setAttribute(attr, t(node.getAttribute(attr)));
  }
}

document.documentElement.lang = lang();
translatePage();

async function api(path, body) {
  const res = await fetch(path, body === undefined ? {} : {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (!res.ok) throw new Error(t(data.error || res.statusText));
  return data;
}

// The themes, chosen in the settings: Sable and Sable Clair, the CSS's own
// dark and light palettes, Slate, the first fennec's palette, and Orange, the
// v4 design's. Each theme is a single palette of this UI's eleven tokens,
// carried on its own. A display-only preference kept in localStorage, applied
// on the document root so every derived shade follows the chosen palette.
const THEME_KEY = "fennec.theme";
const SABLE_LIGHT = {
  "--bg": "#f6f5f2", "--card": "#fff", "--text": "#1d1d1b", "--muted": "#6b6a66",
  "--line": "#e2e0da", "--soft": "#efede8", "--accent": "#c4622d", "--ok": "#2f7d4f",
  "--warn": "#9a6a12", "--bad": "#a33a3a", "--info": "#4a6fa5"
};
const SABLE_DARK = {
  "--bg": "#171716", "--card": "#222220", "--text": "#ecebe7", "--muted": "#9a988f",
  "--line": "#33322f", "--soft": "#2b2a28", "--accent": "#e08050", "--ok": "#5fb383",
  "--warn": "#d9a441", "--bad": "#d46a6a", "--info": "#7d9ccc"
};
const THEMES = [
  { id: "sable", name: "Fennec Sand", palette: SABLE_DARK },
  { id: "sable-clair", name: "Fennec Sand Light", palette: SABLE_LIGHT },
  { id: "slate", name: "Fennec Slate", palette: {
    "--bg": "#191d24", "--card": "#1f242d", "--text": "#dde2e9", "--muted": "#98a0ad",
    "--line": "#353c49", "--soft": "#272d38", "--accent": "#e5a55c", "--ok": "#62b183",
    "--warn": "#d5983f", "--bad": "#dd6b64", "--info": "#6fa0df" } },
  { id: "orange", name: "Fennec Orange", palette: {
    "--bg": "#0b0e14", "--card": "#10141c", "--text": "#e7eaf1", "--muted": "#79839a",
    "--line": "#232b3a", "--soft": "#1b2230", "--accent": "#f0883e", "--ok": "#4cc38a",
    "--warn": "#e0a83e", "--bad": "#e5534b", "--info": "#5aa9e6" } },
];

function isLight(hex) {
  const value = hex.replace("#", "");
  if (value.length !== 6) return false;
  const channel = (at) => {
    const srgb = parseInt(value.slice(at, at + 2), 16) / 255;
    return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4) > 0.18;
}

function applyTheme() {
  const theme = THEMES.find((t) => t.id === loadTheme()) ?? THEMES[0];
  const style = document.documentElement.style;
  for (const token of Object.keys(theme.palette)) style.removeProperty(token);
  for (const [token, value] of Object.entries(theme.palette)) style.setProperty(token, value);
  style.colorScheme = isLight(theme.palette["--bg"]) ? "light" : "dark";
}

function loadTheme() {
  let id;
  try { id = localStorage.getItem(THEME_KEY); } catch { id = null; }
  return THEMES.some((t) => t.id === id) ? id : THEMES[0].id;
}

function saveTheme(id) {
  try { localStorage.setItem(THEME_KEY, id); } catch { /* the choice stays for this session */ }
}

applyTheme();

// The side bars, once closed, stay closed: like the theme, a display-only
// preference kept in localStorage.
const SIDE_KEY = "fennec.side";
const FACTS_KEY = "fennec.facts";
const pref = (key) => { try { return localStorage.getItem(key); } catch { return null; } };
const setPref = (key, value) => {
  try { value ? localStorage.setItem(key, value) : localStorage.removeItem(key); } catch { /* for this session */ }
};
const setSide = (off) => { document.body.classList.toggle("side-off", off); setPref(SIDE_KEY, off ? "off" : null); };
const setFacts = (off) => { document.body.classList.toggle("facts-off", off); setPref(FACTS_KEY, off ? "off" : null); };
if (pref(SIDE_KEY)) document.body.classList.add("side-off");
if (pref(FACTS_KEY)) document.body.classList.add("facts-off");

// The interface's scale, chosen in the settings: a zoom on the document root,
// so fonts and spacing grow together and no box overflows. A display-only
// preference kept in localStorage, like the theme.
const FONT_KEY = "fennec.fontscale";
const FONT_BASE_PX = 15, FONT_MIN_PX = 12, FONT_MAX_PX = 21, FONT_STEP_PX = 1;

function loadFontScale() {
  let v;
  try { v = parseFloat(localStorage.getItem(FONT_KEY)); } catch { v = NaN; }
  if (v > 0 && v < 5) v *= FONT_BASE_PX;
  if (!Number.isFinite(v) || v < FONT_MIN_PX || v > FONT_MAX_PX) v = FONT_BASE_PX;
  return v;
}

function saveFontScale(v) {
  try { localStorage.setItem(FONT_KEY, String(v)); } catch { /* stays for this session */ }
}

function applyFontScale() {
  const v = loadFontScale();
  document.documentElement.style.zoom = v / FONT_BASE_PX;
  return v;
}

applyFontScale();

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
  const button = el("button", { type: "button", className: "quiet", textContent: undo ? t("Undo") : t("Drop"), title: undo ? t("Undo the work: your folder goes back as it was") : t("Drop the task") });
  button.onclick = () => {
    const sure = action(t("Confirm"), "/api/drop", { id }, { quiet: true });
    sure.style.color = "var(--bad)";
    button.replaceWith(sure);
  };
  return button;
}

// Erasing a finished request is one more click than reading it: the first
// click only asks.
function forgetButton(id) {
  const button = el("button", { type: "button", className: "quiet", textContent: t("Delete"), title: t("Delete this task") });
  button.onclick = () => {
    const sure = action(t("Confirm"), "/api/forget", { id }, { quiet: true });
    sure.style.color = "var(--bad)";
    button.replaceWith(sure);
  };
  return button;
}

// A list row's own actions, on its right: archiving is one click, erasing
// asks once before doing. An error, if any, is said with the buttons.
function rowActions(id, { archive, forget, restore = false, stop } = {}) {
  const error = el("div", { className: "error", role: "alert" });
  const go = (path, body) => async () => {
    error.textContent = "";
    try {
      await api(path, body);
      refresh(true);
    } catch (e) {
      error.textContent = e.message;
    }
  };
  const nodes = [];
  // Stopping work under way is one more click than reading the line.
  if (stop) {
    const halt = el("button", { type: "button", className: "icon", ariaLabel: t("Stop"), title: t("Stop") });
    setIcon(halt, "stop");
    halt.onclick = () => {
      const sure = el("button", { type: "button", className: "quiet", textContent: t("Sure?") });
      sure.style.color = "var(--bad)";
      sure.onclick = go(stop, { id });
      halt.replaceWith(sure);
    };
    nodes.push(halt);
  }
  if (archive) {
    const first = el("button", { type: "button", className: "icon",
      ariaLabel: restore ? t("Restore") : t("Archive"), title: restore ? t("Restore") : t("Archive") });
    setIcon(first, restore ? "undo" : "inbox");
    first.onclick = go(archive, { id, on: !restore });
    nodes.push(first);
  }
  if (forget) {
    const del = el("button", { type: "button", className: "icon", ariaLabel: t("Delete"), title: t("Delete") });
    setIcon(del, "trash");
    del.onclick = () => {
      const sure = el("button", { type: "button", className: "quiet", textContent: t("Confirm") });
      sure.style.color = "var(--bad)";
      sure.onclick = go(forget, { id });
      del.replaceWith(sure);
    };
    nodes.push(del);
  }
  return el("div", { className: "row-actions" }, ...nodes, error);
}
const rowWrap = (link, actions) => el("div", { className: "row-wrap" }, link, actions);

// A status's label, in English: shown through `t`.
const TAGS = {
  talk: ["Discussion", "var(--accent)"],
  validate: ["To validate", "var(--accent)"],
  blocked: ["Question", "var(--accent)"],
  rule: ["Rule?", "var(--accent)"],
  ready: ["Ready", "var(--ok)"],
  paused: ["Stopped after the code", "var(--accent)"],
  report: ["Report", "var(--ok)"],
  later: ["Backlog", "var(--muted)"],
  failed: ["Failed", "var(--bad)"],
};

function focus(label, text) {
  return el("div", { className: "focus" }, el("span", { className: "label", textContent: label }), ...md(text));
}

// Where a thing shown comes from: checked by code ("verified") or said by a
// model (indicative). Shown next to what the human decides on.
function prov(kind) {
  return el("span", { className: `prov ${kind}`, textContent: kind === "verified" ? t("verified") : t("model"),
    title: kind === "verified" ? t("Checked by code: you can rely on it.") : t("Written by a model: indicative, read it.") });
}
function focusBy(label, text, kind) {
  const node = focus(label, text);
  node.querySelector(".label").append(" ", prov(kind));
  return node;
}

// The task in the cycle: its risk (verified), its design and plan (the
// model's), the tests written first, and what the human skipped.
// An analysis' report: the model's reading of the project (findings,
// suggestions) and its proposals for the project's knowledge. Only the human
// accepts a proposal, as written or edited; fennec checks it and commits it.
// `live`: the report waits for the human, its buttons act.
function reportView(item, live) {
  const r = item.report ?? {};
  const out = [focusBy(t("Report"), r.summary ?? "", "model")];
  const label = (text) => el("div", { className: "label report-label", textContent: text });
  if (r.findings?.length) {
    out.push(label(t("Findings")), el("ul", { className: "findings" }, ...r.findings.map((f) =>
      el("li", { className: `sev-${f.severity}` },
        el("strong", { textContent: f.title }), el("span", { className: "muted small", textContent: " · " + t(f.severity) }),
        f.detail ? el("div", {}, ...md(f.detail)) : null,
        f.files?.length ? el("div", { className: "muted small mono", textContent: f.files.join(", ") }) : null))));
  }
  if (r.suggestions?.length) {
    out.push(label(t("Suggestions")), el("ol", { className: "suggestions" }, ...r.suggestions.map((s, i) => {
      const act = s.kind === "knowledge"
        ? el("span", { className: "muted small", textContent: t("A change to the project's knowledge: see the proposals") })
        : s.made ? el("a", { className: "link", href: hashOf(item.path, s.made), textContent: t("✓ in the Backlog") })
        : live ? action(t("Make it a task"), "/api/suggest", { id: item.id, index: i }) : null;
      return el("li", {}, el("strong", { textContent: s.title }), s.why ? el("div", {}, ...md(s.why)) : null, act);
    })));
  }
  const pending = [];
  if (r.proposals?.length) {
    out.push(label(t("Proposals for the project's knowledge")), ...r.proposals.map((p, i) => {
      const head = el("div", { className: "row" }, el("code", { textContent: p.file }), prov("model"),
        el("span", { className: "muted small", textContent: p.before == null ? t("new file") : t("replaces the current file") }));
      const box = el("div", { className: "proposal" }, head, p.why ? el("div", { className: "muted" }, ...md(p.why)) : null);
      const now = p.before == null ? null
        : el("details", {}, el("summary", { textContent: t("As it is now") }), el("pre", { textContent: p.before }));
      if (p.accepted) box.append(el("div", { className: "proved", textContent: t("✓ committed {0}", (p.commit ?? "").slice(0, 7)) }));
      else if (p.rejected) box.append(el("div", { className: "muted", textContent: t("rejected") }));
      else if (p.allowed === false) box.append(el("div", { className: "bad", textContent: t("fennec will not write it: not a knowledge file") }));
      if (p.accepted || p.rejected || p.allowed === false || !live) {
        box.append(el("details", {}, el("summary", { textContent: t("Proposed text") }), el("pre", { textContent: p.content })));
        return box;
      }
      const take = el("input", { type: "checkbox", checked: true, ariaLabel: t("Accept {0}", p.file) });
      const text = el("textarea", { className: "mono", value: p.content, ariaLabel: t("Proposed text"),
        rows: Math.min(18, Math.max(4, p.content.split("\n").length)) });
      pending.push({ i, take, text });
      box.append(el("label", { className: "row" }, take, el("span", { textContent: t("Accept, as edited below") })), text, ...(now ? [now] : []),
        el("div", { className: "row" }, action(t("Reject"), "/api/knowledge/reject", { id: item.id, index: i }, { quiet: true })));
      return box;
    }));
  }
  if (pending.length) {
    out.push(el("div", { className: "row" }, Object.assign(
      action(t("Commit the accepted proposals"), "/api/knowledge/accept", () => ({ id: item.id,
        accepted: pending.filter((x) => x.take.checked).map((x) => ({ index: x.i, content: x.text.value })) }), { primary: true }),
      { title: t("fennec checks them, then commits them alone on your branch: \"{0}\"", item.message ?? "") })));
  }
  if (item.knowledge_checks?.length) {
    out.push(label(t("Checks on what was committed")), el("ul", { className: "checks" }, ...item.knowledge_checks.map((c) =>
      el("li", {}, prov("verified"), " ", el("span", { className: c.ok ? "" : "bad", textContent: (c.ok ? "✓ " : "⚠ ") + c.text })))));
  }
  return out;
}

// The end of the cycle: what this work may change in the project's
// knowledge, proposed by a model. Accepted as edited, it goes into this
// task's commit; fennec numbers a new decision and writes nothing else.
function capitaliseView(item) {
  const list = item.capitalise ?? [];
  if (!list.length) return [];
  return [el("div", { className: "label report-label", textContent: t("Knowledge to update with this commit") }), ...list.map((p, i) => {
    const box = el("div", { className: "proposal" }, el("div", { className: "row" }, el("code", { textContent: p.file }), prov("model"),
      el("span", { className: "muted small", textContent: p.before == null ? t("new file") : t("replaces the current file") })),
    p.why ? el("div", { className: "muted" }, ...md(p.why)) : null);
    if (p.allowed === false) {
      box.append(el("div", { className: "bad", textContent: t("fennec will not write it: not a knowledge file") }));
      return box;
    }
    const text = el("textarea", { className: "mono", value: p.content, ariaLabel: t("Proposed text"),
      rows: Math.min(18, Math.max(4, p.content.split("\n").length)) });
    const now = p.before == null ? null
      : el("details", {}, el("summary", { textContent: t("As it is now") }), el("pre", { textContent: p.before }));
    const state = p.accepted ? el("span", { className: "proved", textContent: t("✓ goes into the commit") })
      : p.rejected ? el("span", { className: "muted", textContent: t("left out") }) : null;
    box.append(text, ...(now ? [now] : []), el("div", { className: "row" }, ...[state, el("span", { className: "spacer" }),
      action(t("Leave out"), "/api/capitalise", { id: item.id, index: i, accept: false }, { quiet: true }),
      action(p.accepted ? t("Save the text") : t("Put in the commit"), "/api/capitalise",
        () => ({ id: item.id, index: i, accept: true, content: text.value }), { primary: !p.accepted && !p.rejected })].filter(Boolean)));
    return box;
  })];
}

function cycleView(item, { tests = false } = {}) {
  const c = item.cycle ?? {};
  const nodes = [];
  const zones = c.risk?.zones ?? [];
  if (c.risk) {
    nodes.push(el("div", { className: c.sensitive ? "risk sensitive" : "risk" }, prov("verified"), " ",
      c.sensitive
        ? t("Sensitive: it touches {0}. Design and tests must be validated, the review marked done before the commit.",
          zones.map((z) => `${z.name} (${t(z.kind)})`).join(", "))
        : t("Outside the sensitive zones: validation and review are up to you; the work may not enter a zone.")));
  }
  if (c.architecture) nodes.push(focusBy(t("Architecture of the change"), c.architecture, "model"));
  if (c.plan?.length) {
    nodes.push(el("div", { className: "focus" }, el("span", { className: "label" }, t("Plan"), " ", prov("model")),
      el("ol", { className: "plan-steps" }, ...c.plan.map((s) => el("li", {}, ...rich(s))))));
  }
  if (c.planned?.length) {
    nodes.push(el("div", { className: "files" }, t("Files it expects to touch: "),
      ...c.planned.flatMap((f, i) => [i ? ", " : "", el("code", { textContent: f })])));
  }
  if (c.unplanned?.length) {
    nodes.push(el("div", { className: "risk sensitive" }, prov("verified"), " ", t("Outside the plan: "),
      ...c.unplanned.flatMap((f, i) => [i ? ", " : "", el("code", { textContent: f })])));
  }
  if (tests && c.tests_first && c.tests) {
    nodes.push(el("div", { className: "focus" }, el("span", { className: "label" }, t("Tests written first, to validate"), " ", prov("model")),
      diffView(c.tests)));
  }
  const marks = [c.unvalidated ? pill("!", t("not validated"), "warn", t("Started without the design being validated.")) : null,
    c.unreviewed ? pill("!", t("not reviewed"), "warn", t("Committed without the review being marked done.")) : null].filter(Boolean);
  if (marks.length) nodes.push(el("div", { className: "pills" }, ...marks));
  return nodes;
}

// `command` spans as code, **words** in bold, the rest as plain text.
function rich(text) {
  const lines = (text ?? "").split("\n");
  const images = lines.filter(isImage);
  if (images.length) return [...rich(lines.filter((l) => !isImage(l)).join("\n").trim()), thumbs(images)];
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
      // The model writes these, steered by any file it read: a `javascript:`
      // link would run as this page, which can change the settings.
      if (/^(https?:|\/|#)/i.test(link[2])) out.push(el("a", { href: link[2], target: "_blank", rel: "noreferrer" }, ...inline(link[1])));
      else out.push(m[3]);
    }
    else out.push(el("i", {}, ...inline(m[4].slice(1, -1))));
    last = m.index + m[0].length;
  }
  if (last < rest.length) out.push(rest.slice(last));
  return out;
}

// An image the human attached is a line of its own in what they wrote; the
// server sends it to the model as an image and serves it back here.
function isImage(line) {
  return /^\s*!\[image\]\(file:\/\/.+\)\s*$/.test(line);
}
const imageUrl = (line) => "/images/" + line.trim().slice(0, -1).split("/").pop();
function thumbs(lines, remove) {
  if (!lines?.length) return null;
  return el("div", { className: "thumbs" }, ...lines.map((l) => {
    const link = el("a", { href: imageUrl(l), target: "_blank", rel: "noreferrer", className: "thumb" },
      el("img", { src: imageUrl(l), alt: t("attached image"), loading: "lazy" }));
    if (!remove) return link;
    const x = el("button", { type: "button", className: "quiet drop", ariaLabel: t("Remove the image"), title: t("Remove the image") });
    setIcon(x, "close");
    x.onclick = () => remove(l);
    return el("span", { className: "thumb-wrap" }, link, x);
  }));
}
// What the human wrote, with the images they joined: picked, pasted or
// dropped. The field keeps their words only; value() adds the image lines.
function attach(text, start = []) {
  let lines = [...start];
  const strip = el("span", { className: "thumbs-slot" });
  const error = el("span", { className: "bad" });
  const show = () => strip.replaceChildren(thumbs(lines, (l) => { lines = lines.filter((x) => x !== l); show(); }) ?? "");
  const add = async (files) => {
    error.textContent = "";
    for (const f of files.filter((f) => f.type.startsWith("image/"))) {
      try {
        const data = await new Promise((done, fail) => {
          const r = new FileReader();
          r.onload = () => done(r.result);
          r.onerror = () => fail(r.error);
          r.readAsDataURL(f);
        });
        const { line } = await api("/api/image", { data });
        if (!lines.includes(line)) lines.push(line);
      } catch (e) {
        error.textContent = e.message;
      }
    }
    show();
  };
  text.addEventListener("paste", (e) => {
    const files = [...e.clipboardData.files];
    if (files.some((f) => f.type.startsWith("image/"))) { e.preventDefault(); add(files); }
  });
  text.addEventListener("dragover", (e) => e.preventDefault());
  text.addEventListener("drop", (e) => {
    if (e.dataTransfer.files.length) { e.preventDefault(); add([...e.dataTransfer.files]); }
  });
  const pick = el("input", { type: "file", accept: "image/png,image/jpeg,image/gif,image/webp", multiple: true, hidden: true });
  pick.onchange = () => { add([...pick.files]); pick.value = ""; };
  const button = el("button", { type: "button", className: "quiet", ariaLabel: t("Attach an image"),
    title: t("Attach an image — or paste it, or drop it in the text") });
  setIcon(button, "image");
  button.onclick = () => pick.click();
  show();
  return {
    node: el("div", { className: "attach" }, button, pick, strip, error),
    value: () => [text.value.trim(), ...lines].filter(Boolean).join("\n"),
    clear: () => { text.value = ""; lines = []; show(); },
  };
}

// Models write Markdown: its lines become headings, lists, tables and code,
// not one block of raw symbols.
function md(text) {
  const all = (text ?? "").split("\n");
  if (all.some(isImage)) return [...md(all.filter((l) => !isImage(l)).join("\n")), thumbs(all.filter(isImage))];
  const lines = all;
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
// The raw reason stays behind "Details".
// The model server's errors (ureq's wording), in the human's words.
function unreachable(line) {
  // Anchored: a test's output that mentions a timeout is not the server's.
  const m = line.match(/(?:^|\()(timeout|http status|io|host not found|connection failed|bad uri)(?::? ?([^\s:]*))/);
  if (!m) return null;
  const model = line.match(/model '([^']+)' not found/);
  if (model) return t("the server does not serve the model \"{0}\" (chosen in Team or in Settings)", model[1]);
  if (m[1] === "timeout") return t("the server does not answer in time");
  if (m[1] === "http status") return m[2] === "401" || m[2] === "403" ? t("the server refuses the key")
    : m[2] === "404" ? t("the server does not know this address (it often ends with /v1)")
    : t("the server answers with error {0}", m[2]);
  if (m[1] === "host not found") return t("this address does not exist");
  if (m[1] === "bad uri") return t("this address is not valid");
  return t("nothing answers at this address");
}

function explain(reason) {
  const line = firstLine(reason);
  let m = line.match(/^rule task\.protect violated: changed (.+)$/);
  if (m) return t("fennec changed a validated test ({0}), which it may not do", m[1]);
  m = line.match(/^rule \S+ violated: changed (\S+) but nothing under (\S+)$/);
  if (m) return t("{0} changed with no change under {1}, as the project's rules require", m[1], m[2]);
  if (/not a git repository/.test(line)) {
    return t("the project folder is no longer a git repository (its .git is gone): add it again with \"+ project\", then retry");
  }
  if (/the project folder has uncommitted changes/.test(line)) {
    return t("your folder has uncommitted changes: fennec works there and must be able to undo a failed try without touching yours. Commit them or set them aside (git stash), then retry");
  }
  if (/the project folder already holds work in progress/.test(line)) {
    return t("your folder already holds another task's work: commit it, or drop it, then retry");
  }
  m = line.match(/^port (\d+) is already taken/);
  if (m) return t("port {0} is already taken on your machine: stop what holds it, then open the page again", m[1]);
  m = line.match(/^nothing listens on port (\d+) after 30 s/);
  if (m) return t("the page did not start: nothing listens on port {0} after 30 s", m[1]);
  if (/the project folder is on no branch/.test(line)) return t("your folder is on no git branch: check out a branch, then retry");
  m = line.match(/^no model: .* and (\S+)\/models did not answer/);
  const down = unreachable(line);
  if (m) return down ? t("fennec reaches no model at {0} ({1}): check Settings, then retry", m[1], down)
    : t("fennec reaches no model at {0}: check Settings, then retry", m[1]);
  if (down) return t("fennec cannot reach its model: {0}. Check Settings, then retry", down);
  m = line.match(/^rule \S+ violated: changed (.+)$/);
  if (m) return t("fennec changed {0}, which the project's rules protect", m[1]);
  return t(line);
}

function shots(item) {
  const row = el("div", { className: "shots" });
  for (const s of item.shots ?? []) {
    if (s.missing) continue; // said in "not verified"
    if (!s.file) {
      row.append(el("div", { className: "bad", textContent: t("Page {0}: {1}", s.url, t(firstLine(s.reason))) }));
      continue;
    }
    const capture = (file, label) => {
      const link = el("a", { href: `/shots/${file}`, target: "_blank", className: "shot" });
      link.append(el("img", { src: `/shots/${file}`, alt: `${label} — ${s.url}` }), `${label} — ${s.url}`);
      return link;
    };
    if (s.before) {
      row.append(capture(s.before, t("Before")),
        el("div", { className: "steps", textContent: t("↓ Done by fennec: {0}", (s.steps ?? []).join(" → ")) }),
        capture(s.file, t("After")));
    } else {
      row.append(capture(s.file, t("Screenshot")));
    }
  }
  return row.children.length ? row : null;
}

// Everything that explains but does not decide.
function details(item) {
  const box = el("details", { className: "more", open: true }, el("summary", { textContent: t("Details") }));
  box.dataset.key = item.id;
  const part = (title, ...nodes) => box.append(el("h3", { textContent: title }), ...nodes);
  // A plan step carries the discussion's whole plan after "---": the same in
  // every step, so it is named as shared, apart from the step itself.
  const plan = (item.description ?? "").split("\n\n---\n")[1];
  if (plan) part(t("The discussion's plan, shared by its tasks"), el("div", {}, ...md(plan)));
  const errors = [item.reason, item.land_error].filter(Boolean);
  if (errors.length) part(t("Error"), ...errors.map((e) => el("pre", { className: "failed", textContent: e })));
  if (item.land?.message && item.files?.length !== 0) part(t("Commit message"), el("pre", { textContent: item.land.message }));
  // Without stats (an old request), the team that worked on it at least.
  if (item.team?.length && !item.stats?.stages) part(t("Team"), el("p", { textContent: item.team.join(", ") }));
  if (item.rules?.length) part(t("Rules applied here"), el("ul", {}, ...item.rules.map((r) => el("li", { textContent: r }))));
  // What each step and each model cost, in one table so the columns line up.
  const rows = (key) => Object.entries(item.stats?.[key] ?? {}).sort(([a], [b]) => stageRank(a) - stageRank(b));
  const head = (label) => el("tr", {}, ...[label, label === "Model" ? "" : "Roles", "Time", "Calls", "Sent", "Cache", "Generated", "Speed"]
    .map((h) => el("th", { textContent: t(h) })));
  const line = (stage) => ([name, c]) => el("tr", {},
    el("th", {}, stage ? el("span", { className: `dotc ${stageOf(name)}` }) : null, stage ? stageName(name) : name),
    el("td", {}, ...(c.roles ?? []).map((r) => roleTag(r, stageOf(name)))),
    el("td", { textContent: duration(c.ms ?? 0) }), el("td", { textContent: number(c.turns) }),
    el("td", { textContent: compact(c.sent) }),
    el("td", { textContent: `${percent(c.cached, c.sent)} %` }),
    el("td", { textContent: compact(c.received) }), el("td", { textContent: c.ms ? `${tps(c)}/s` : "—" }));
  const [stages, models] = [rows("stages"), rows("models")];
  if (stages.length || models.length) {
    part(t("Cost per step"), el("div", { className: "scroll" }, el("table", { className: "grid" },
      stages.length ? el("thead", {}, head("Step")) : null, el("tbody", {}, ...stages.map(line(true))),
      models.length ? el("tbody", { className: "sub" }, head("Model"), ...models.map(line(false))) : null)));
  }
  if (item.checks?.length) part(t("Tests"), el("ul", {}, ...item.checks.map((c) => el("li", {}, el("code", { textContent: c })))));
  // Each command run to prove the work, folded; a failed one opens by itself.
  const ran = (who) => (c) => {
    const ok = !c.missing && c.exit === 0;
    return el("details", { className: `message try${ok ? "" : " failed"}`, open: !ok },
      el("summary", {}, el("b", { textContent: c.missing ? t("not installed") : ok ? t("✓ passed") : t("✗ code {0}", c.exit) }),
        el("span", { className: "head", textContent: `${who}$ ${c.command}` })),
      c.output ? el("pre", { textContent: c.output }) : null);
  };
  if (item.tries?.length) part(t("fennec's tries"), ...item.tries.map(ran("")));
  if (item.role_checks?.length) part(t("The team's checks"), ...item.role_checks.map((c) => ran(`${c.role} · `)(c)));
  return box.children.length > 1 ? box : null;
}

// What fennec did, in its words, and where: the work itself, not just its verdict.
const FILES_SHOWN = 8;
function work(item) {
  const nodes = [];
  if (item.summary) nodes.push(focus(t("What fennec did"), item.summary));
  if (item.files?.length) {
    // A long list (installed packages...) shows its first lines; the rest on demand.
    const list = (files) => files.map((f) => el("code", { textContent: f }));
    const rest = item.files.length > FILES_SHOWN + 2 ? item.files.slice(FILES_SHOWN) : [];
    nodes.push(el("div", { className: "files" }, t("Files ({0}): ", item.files.length),
      ...list(item.files.slice(0, item.files.length - rest.length)),
      rest.length ? el("details", {}, el("summary", { textContent: t("{0} more", rest.length) }), ...list(rest)) : null));
  }
  // Which of the project's skills the work read: the Journal shows where.
  if (item.skills?.length) nodes.push(el("div", { className: "files" }, t("Skills used: "), item.skills.join(", ")));
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
    box = el("div", { className: "changes" }, el("div", { className: "muted", textContent: t("Loading…") }));
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
  return opener(t("See the changes"), t("Hide the changes"), where, () => changes(item));
}
async function changes(item) {
  const c = await api("/api/changes", { id: item.id });
  const nodes = item.summary || !c.summary ? [] : [focus(t("What fennec did"), c.summary)];
  nodes.push(diffView(c.diff));
  if (c.cut) nodes.push(el("div", { className: "muted", textContent: t("Cut: the rest is too long for the page, look at it in the project.") }));
  return nodes;
}

// The ready work is a real folder: opened in the editor, its path shown to copy.
function codeButton(item, where) {
  return opener(t("Open the code"), t("Hide the folder"), where, async () => {
    const c = await api("/api/open", { id: item.id });
    return [el("div", { className: "muted", textContent: (c.opened ? t("Opened in VS Code.") : t("VS Code not found: open this folder in your editor."))
        + " " + t("You may change the code there: at commit, fennec runs the tests again before putting it all in your folder.") }),
      el("pre", { textContent: c.folder })];
  });
}

// A web product is tried in the human's own browser, started as the proof started it.
// The tab opens on the click itself: one opened after the answer would be blocked.
function pageButton(item) {
  const error = () => errorOf(button);
  const button = el("button", { type: "button", textContent: t("Open the page") });
  button.onclick = async () => {
    const tab = window.open("", "_blank");
    button.disabled = true;
    button.textContent = t("Starting the page…");
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
      button.textContent = t("Open the page");
    }
  };
  return button;
}

// Every step, with what was sent to the model, what came back, and what it cost.
function journalButton(item, where) {
  return opener(t("Journal"), t("Hide the journal"), where, () => journal(item));
}
async function journal(item) {
  const steps = await api("/api/journal", { id: item.id });
  if (!steps.length) return [el("div", { className: "muted", textContent: t("No step kept for this task.") })];
  return steps.map(stepView);
}

// Work under way reads as a story: what was asked and said, what "done"
// means, and where the work stands; the model's raw exchange is the Journal tab.
const STEPS = [["Discussion", /^thinks/], ["Framing", /^gets to work/], ["Work", /^works/],
  ["Review", /^reviews/], ["Try", /^tries/], ["Commit", /^makes the commit/]];
function stages(item) {
  if (item.step == null) return ended(item);
  const now = STEPS.findIndex(([, re]) => re.test(item.step));
  return el("ol", { className: "stages" }, ...STEPS.map(([name], i) =>
    el("li", { className: i < now ? "done" : i === now ? "now" : "", textContent: t(name) })));
}
// A finished request: the steps its figures show it reached, and where it
// stopped when it did not end in a commit. A step skipped (direct work has
// no Framing) stays plain.
function ended(item) {
  const reached = Object.keys(item.stats?.stages ?? {})
    .map((name) => ["Discussion", "Framing", "Work", "Review", "Proof"].findIndex((s) => stageKey(name).startsWith(s)));
  if (item.kind === "landed") reached.push(STEPS.length - 1);
  if (!reached.length) return null;
  const last = Math.max(...reached);
  const broke = item.kind === "failed" || item.kind === "dropped";
  return el("ol", { className: "stages" }, ...STEPS.map(([name], i) => el("li", {
    className: !reached.includes(i) ? "" : i === last && broke ? "stop" : "done", textContent: t(name),
    title: i === last && broke ? (item.kind === "failed" ? t("Failed here") : t("Stopped here")) : "" })));
}

function underway(item) {
  const stop = el("button", { type: "button", className: "stop", textContent: t("Stop") });
  setIcon(stop, "stop");
  stop.onclick = () => {
    const sure = action(t("Confirm"), "/api/stop", { id: item.id });
    sure.classList.add("stop", "sure");
    setIcon(sure, "stop");
    stop.replaceWith(sure);
  };
  const bar = el("div", { className: "past-actions run-actions" },
    // Queued or mid-commit with its worker dead: "Retry" starts it
    // over; "Stop" stays the way out.
    ...(!item.alive && (item.waiting || item.step === "makes the commit")
      ? [action(t("Retry"), "/api/retry", { id: item.id }, { primary: true })]
      : []),
    stop,
    el("div", { className: "error", role: "alert" }));
  let elapsed = null;
  if (item.since) {
    // Queued, not working: no ticking figure, one age at each redraw.
    elapsed = item.waiting
      ? el("span", { textContent: t("for {0}", duration(Date.now() - item.since * 1000)) })
      : el("span", { className: "elapsed" });
    if (!item.waiting) elapsed.dataset.since = item.since;
  }
  const box = el("div", { className: "past run" },
    stages(item),
    el("div", { className: "state" }, el("span", { className: "dot" }), t("fennec {0}", stepText(item.step)), elapsed ? " · " : "", elapsed),
    conversation(item), doneWhen(item), bar, ...facts(item));
  box.dataset.key = "run-" + item.id;
  return box;
}

// The Journal tab of a request under way follows the model as it answers;
// a newer view of the request starts its own and this one stops.
let followed = 0;
async function follow(id, slot) {
  const mine = ++followed;
  let last = "";
  while (mine === followed && picked === id) {
    try {
      const steps = await api("/api/journal", { id });
      const json = JSON.stringify(steps);
      if (json !== last) {
        last = json;
        // What the human opened stays open as new messages come in.
        const opened = [...slot.querySelectorAll("details")].map((d) => d.open);
        // Follow the model as it writes, unless the human scrolled up to read.
        const end = slot.lastElementChild?.getBoundingClientRect().bottom;
        const following = end == null || end <= innerHeight + 80;
        slot.replaceChildren(...steps.map(stepView));
        slot.querySelectorAll("details").forEach((d, i) => { d.open = opened[i] ?? false; });
        if (following && slot.isConnected) slot.lastElementChild?.scrollIntoView({ block: "end", behavior: "smooth" });
      }
    } catch (e) {
      slot.replaceChildren(el("div", { className: "bad", textContent: e.message }));
    }
    await new Promise((done) => setTimeout(done, 3000));
  }
}

// Steps as the server names them; requests from before fennec spoke English
// keep their French names, read as the English ones.
const LEGACY_STEPS = [["Cadrage", "Framing"], ["Démarrage", "Starting"], ["Travail, essai", "Work, try"], ["Travail", "Work"],
  ["Preuve ratée : renvoyée au travail", "Proof failed: sent back to work"], ["Preuve", "Proof"], ["Ta réponse", "Your answer"]];
const stageKey = (name) => {
  const old = LEGACY_STEPS.find(([fr]) => name.startsWith(fr));
  return old ? old[1] + name.slice(old[0].length) : name;
};
// Steps in the order they run; the server keeps them sorted by name.
const stageRank = (name) => ["Discussion", "Framing", "Starting", "Work", "Review", "Proof"].findIndex((s) => stageKey(name).startsWith(s)) >>> 0;
const number = (n) => (n ?? 0).toLocaleString(lang());
const compact = (n) => (n ?? 0).toLocaleString(lang(), { notation: "compact", maximumFractionDigits: 1 });
const percent = (part, whole) => (whole ? Math.round((100 * part) / whole) : 0);
// Tokens out per second of model time; that time includes reading the prompt.
const tps = (c) => (c.ms ? Math.round((c.received * 1000) / c.ms) : 0);
const speed = (c) => (c.ms && c.received ? `≈ ${tps(c)} tokens/s` : "");

// How the work was kept safe, and what it cost: under the work, small.
function facts(item) {
  const s = item.stats;
  if (!s?.turns) return [];
  const n = s.failed_attempts;
  const safe = el("div", { className: "pills" },
    pill("🔒", t("verified offline"), "", t("The try reaches the Internet to install what the project needs; verification and proof run offline.")),
    pill("🏠", t("your home folder hidden"), "", t("Your home folder (~) is out of reach: the try reads and writes the project only.")),
    pill("📖", t("git read-only"), "", t("The project's git history is visible, but the try does not change it.")),
    n ? pill("↺", n > 1 ? t("{0} failed tries undone", n) : t("{0} failed try undone", n), "warn",
      t("These tries failed and are undone: no trace in your folder.")) : null);
  const [bar, who] = timeline(s);
  return [el("div", { className: "facts" },
    metrics(s),
    bar ? el("h3", { textContent: t("Where the time went") }) : null, bar,
    who ? el("h3", { textContent: t("Team"), title: t("The project's team that worked on this task: who does what is set on the Team page.") }) : null, who,
    el("h3", { textContent: t("Isolation") }), safe)];
}

function pill(icon, text, tone = "", title = "") {
  return el("span", { className: `pill ${tone}`, ...(title ? { title } : {}) },
    el("span", { className: "icon", textContent: icon, ariaHidden: "true" }), text);
}

// Model time, tokens in and out, cache and speed: a figure each, not a sentence.
const METRIC_TIP = {
  "Model time": "The model's generation time, without tools or waits.",
  "Calls": "How many calls were made to the model for this task.",
  "Sent": "What goes to the model: context, instructions, tool results.",
  "Generated": "What the model produced: answers, code, reasoning.",
  "Speed": "Average tokens produced per second.",
};
function metrics(c) {
  const cache = percent(c.cached, c.sent);
  return el("div", { className: "metrics" },
    metric("Model time", duration(c.ms ?? 0)),
    metric("Calls", number(c.turns)),
    metric("Sent", compact(c.sent), el("span", { className: "meter", style: `--p:${cache}%`, title: t("Share of the tokens sent already cached: cheaper, faster.") }),
      el("small", { textContent: t("{0} % cache", cache) })),
    metric("Generated", compact(c.received), c.thinking ? el("small", { textContent: t("{0} % reasoning", percent(c.thinking, c.received)), title: t("Share of the generated tokens that is internal reasoning, not the answer.") }) : null),
    metric("Speed", c.ms ? `${tps(c)}` : "—", el("small", { textContent: "tokens/s" })));
}

function metric(label, value, ...extra) {
  const tip = METRIC_TIP[label];
  return el("span", { className: "metric", ...(tip ? { title: t(tip) } : {}) }, el("small", { textContent: t(label) }), el("b", { textContent: value }), ...extra);
}

// Where the time went, one coloured segment per step, with the roles of each.
function timeline(s) {
  const stages = Object.entries(s.stages ?? {}).sort(([a], [b]) => stageRank(a) - stageRank(b));
  if (!stages.length) return null;
  const total = stages.reduce((t, [, c]) => t + (c.ms ?? 0), 0) || 1;
  const bar = el("div", { className: "timeline" }, ...stages.map(([name, c]) =>
    el("span", { className: `seg ${stageOf(name)}`, style: `flex:${Math.max(c.ms ?? 0, total / 40)}`,
      title: t(STAGE_TIP[stageOf(name)]), textContent: `${stageName(name)} ${duration(c.ms ?? 0)}` })));
  const who = el("div", { className: "who" }, ...stages.flatMap(([name, c]) =>
    (c.roles ?? []).map((r) => roleTag(r, stageOf(name)))));
  return [bar, who.children.length ? who : null];
}

const stageOf = (name) => {
  const n = stageKey(name);
  return n.startsWith("Discussion") ? "talk" : n.startsWith("Work") ? "solve" : n.startsWith("Proof") ? "prove" : n.startsWith("Review") ? "review" : "frame";
};
// A step's name in the chosen language: "Work, try 2" keeps its number.
const stageName = (name) => {
  const n = stageKey(name);
  // The framing is the design now; tests come first only in a sensitive zone.
  if (n.startsWith("Framing")) return t("Design");
  const m = n.match(/^(Work|Proof|Starting|Review) \((?:running|en cours)\)$/);
  if (m) return t("{0} (running)", t(m[1]));
  const tried = n.match(/^Work, try (\d+)$/);
  if (tried) return t("Work, try {0}", tried[1]);
  const via = n.match(/^(Via|Plugin) (.+)$/);
  return via ? `${t(via[1])} ${via[2]}` : t(n);
};
// One short sentence per stage: what it is, for the bar's segments and the role chips.
const STAGE_TIP = { talk: "Opening discussion: what you asked, and what the team took from it.", frame: "Framing: the tests that will prove the end and the scope, set before coding.", solve: "Making: the work is written and run, try after try.", review: "Review: the team's reviewer checks the work against the goal and the done when.", prove: "Proof: the result is checked with the tests and the role's commands." };
// What a role chip is: its brief if the step keeps one, else the stage's job.
const roleTag = (name, stage, brief) => el("span", { className: `rtag ${stage}`, textContent: name, title: brief ? brief.replace(/\s+/g, " ").trim().slice(0, 200) : t(STAGE_TIP[stage]) });

function duration(ms) {
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`;
}

function stepView(step) {
  if (step.said != null) return el("section", { className: "step said" },
    el("div", { className: "step-name" }, el("span", { textContent: stageName(step.name) })), el("p", { textContent: step.said }));
  const c = step.cost ?? {};
  const verdict = step.passed === true ? " ✓" : step.passed === false ? " ✗" : "";
  const messages = step.messages ?? [];
  // One line of cost in the head: the full figures sit on the Stats tab and
  // tiles per step made the journal a wall of boxes.
  const cost = c.turns ? el("span", { className: "cost",
    title: [c.turns > 1 ? t("{0} calls", number(c.turns)) : t("{0} call", number(c.turns)), t("{0} % cached", percent(c.cached, c.sent)),
      c.thinking ? t("{0} % reasoning", percent(c.thinking, c.received)) : ""].filter(Boolean).join(" · "),
    textContent: [`⏱ ${duration(c.ms ?? 0)}`, `↑ ${compact(c.sent)}`, `↓ ${compact(c.received)}`, speed(c)].filter(Boolean).join(" · ") }) : null;
  return el("section", { className: `step ${stageOf(step.name)}` },
    el("div", { className: "step-name" }, el("span", { className: `dotc ${stageOf(step.name)}` }),
      el("span", { textContent: stageName(step.name) + verdict }),
      step.model ? el("span", { className: "model", textContent: step.model }) : null,
      messages.length ? el("span", { className: "model", textContent: messages.length > 1 ? t("{0} messages", messages.length) : t("{0} message", messages.length) }) : null,
      el("span", { className: "spacer" }), cost),
    ...rolesView(step),
    ...(messages.length ? messages.map(messageView)
      : [el("div", { className: "muted", textContent: step.live
        ? t("fennec waits for the model's first answer…")
        : t("This step's messages were not kept (an older task).") })]));
}

// Who of the team took part: what each role told the model, and what it ran itself.
function rolesView(step) {
  if (!step.roles) return [];
  if (!step.roles.length) return [];
  const nodes = [el("div", { className: "who" }, ...step.roles.map((r) => roleTag(r.name, stageOf(step.name), r.brief)))];
  for (const r of step.roles) {
    if (r.brief) {
      nodes.push(el("details", { className: "message" }, el("summary", {}, el("b", { textContent: r.name }), el("span", { className: "head", textContent: t("its brief to the model") })),
        el("pre", { textContent: r.brief })));
    }
  }
  for (const c of step.checks ?? []) {
    const result = c.missing ? t("not installed, not verified") : c.exit === 0 ? t("✓ passed") : t("✗ failed (code {0}): blocks", c.exit);
    nodes.push(el("details", { className: "message" },
      el("summary", {}, el("b", { textContent: c.role }), el("span", { className: "head", textContent: t("ran \"{0}\" → {1}", c.command, result) })),
      el("pre", { textContent: c.output || t("(no output)") })));
  }
  return nodes;
}

const who = { system: "Brief", user: "Sent", assistant: "Model", tool: "Tool result" };

// What the model thought before it answered: where a made-up fact shows first.
function thinks() {
  const mark = el("span", { className: "thinks", title: t("The model reasoned before answering: open to read its reasoning."), ariaLabel: t("reasoning") });
  setIcon(mark, "think");
  return mark;
}

// A shell call reads as the command it runs, not as its JSON.
function callText(call) {
  const args = call.function?.arguments ?? "";
  try {
    const a = JSON.parse(args);
    if (typeof a?.command === "string" && Object.keys(a).length === 1) return `$ ${a.command}`;
  } catch { /* not JSON: shown as it came */ }
  return `${call.function?.name ?? t("tool")} ${args}`;
}

// The CSS heading of a block, in the chosen language.
const labelled = (node, label) => (node.dataset.label = label, node);

function messageView(m) {
  const text = typeof m.content === "string" ? m.content : m.content == null ? "" : JSON.stringify(m.content, null, 1);
  const calls = (m.tool_calls ?? []).map(callText);
  const thought = m.reasoning_content ?? m.reasoning;
  const head = (calls.join(" ; ") || text || thought || "").replace(/\s+/g, " ").slice(0, 160);
  return el("details", { className: `message ${m.role}${calls.length ? " call" : ""}` },
    el("summary", {}, el("b", { textContent: who[m.role] ? t(who[m.role]) : m.role }), thought ? thinks() : null,
      el("span", { className: "head", textContent: head })),
    thought ? labelled(el("pre", { className: "thought", textContent: thought }), t("Reasoning")) : null,
    text ? el("pre", { textContent: text }) : null,
    m.role === "user" ? thumbs(text.split("\n").filter(isImage)) : null,
    calls.length ? el("pre", { className: "calls", textContent: calls.join("\n") }) : null);
}

function diffView(diff) {
  if (!diff) return el("div", { className: "muted", textContent: t("No change.") });
  const pre = el("pre", { className: "diff" });
  for (const l of diff.split("\n")) {
    const kind = l.startsWith("diff --git") ? "file" : l.startsWith("+") && !l.startsWith("+++") ? "add"
      : l.startsWith("-") && !l.startsWith("---") ? "del" : l.startsWith("@@") ? "hunk" : "";
    if (/^(index |--- |\+\+\+ )/.test(l)) continue;
    pre.append(el("span", { className: kind, textContent: kind === "file" ? l.replace(/^diff --git a\/(\S+) .*/, "$1") : l }), "\n");
  }
  return pre;
}

// What was asked, first on the page: the title is the head above, so only
// the description is here, the plan a step shares left to the details.
function askedOf(item) {
  const step = (item.description ?? "").split("\n\n---\n")[0];
  return step ? el("div", { className: "asked" }, ...md(step)) : null;
}

// The conversation that shaped the request, the human's words on the right.
function conversation(item) {
  // The opening message is the description, shown above it.
  const said = (item.thread ?? []).filter((m, i) => i > 0 || m.who !== "human");
  // The conversation in full, what fennec read included, is a discussion
  // like the Agent's: it opens there.
  const full = item.chat ? el("a", { className: "link", href: "#chat/" + item.chat, textContent: t("Follow the conversation") }) : null;
  // The conversation waits on the person (a folder outside the project, a
  // rule's call): answered there, where the question and its buttons are.
  const p = item.asks;
  const ask = p && item.chat ? el("div", { className: "ask" },
    el("span", { className: "muted", textContent: t("fennec asks: {0}", p.path ?? p.call ?? p.target ?? "") }),
    el("a", { className: "link", href: "#chat/" + item.chat, textContent: t("Answer in the conversation") })) : null;
  if (!said.length) return full && el("div", { className: "thread" }, ask ?? full);
  return el("div", { className: "thread" }, ...said.map((m) =>
    el("div", { className: m.who === "human" ? "said human" : "said" },
      el("span", { className: "label", textContent: m.who === "human" ? t("You") : "fennec" }), ...rich(m.text))), ask ?? full);
}

// A button says the act in a word; where it goes is its title.
function landLabel({ branch, committed }) {
  if (!branch) return t("Prepare");
  return committed ? t("Push") : t("Commit");
}
function landTitle({ branch, remote, committed }) {
  if (!branch) return t("Prepare the commit");
  if (committed) return t("Push {0} to {1}", branch, remote);
  return remote ? t("Commit in {0} and push to {1}", branch, remote) : t("Commit in {0}", branch);
}

// The request for later just saved: its card says so once.
let saved = "";
function card(item) {
  // The head above the tabs says the title and the status: the card does not repeat it.
  const node = el("div", { className: "card" }, askedOf(item), thumbs(item.images));
  const actions = el("div", { className: "actions" });
  const add = (...nodes) => node.append(...nodes.filter(Boolean));

  if (item.kind === "talk") {
    add(conversation(item));
    const text = el("textarea", { placeholder: t("Your answer"), ariaLabel: t("Your answer") });
    const images = attach(text);
    add(el("div", { className: "reply" }, text, images.node));
    // Framed ahead while it waits in the Backlog: answering does not start it.
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      item.analysis ? null : item.later
        ? Object.assign(action(t("Clear enough"), "/api/frame", { id: item.id }),
          { title: t("Keep the conversation as it is: the task waits in the Backlog for you to validate it") })
        : Object.assign(action(t("Start"), "/api/frame", { id: item.id }),
          { title: t("Start the work on the conversation as it is, without another question") }),
      Object.assign(action(t("Send"), "/api/say", () => ({ id: item.id, text: images.value() }), { primary: true }),
        { title: item.analysis ? t("fennec reads your answer and goes on with its analysis")
          : item.later
          ? t("fennec reads your answer when the model is free; the task stays in the Backlog")
          : t("fennec reads your answer: it asks again if it must, otherwise it starts the work") }));
  } else if (item.kind === "validate") {
    add(conversation(item), focusBy(t("Done when"), item.done_when, "model"), ...cycleView(item, { tests: true }));
    const correction = el("textarea", { placeholder: t("What is wrong, or a detail"), hidden: true });
    const fix = el("button", { type: "button", textContent: t("Correct") });
    fix.onclick = () => {
      // Opening the field is the first click; sending is the second.
      correction.hidden = false;
      correction.rows = 6;
      correction.focus();
      fix.replaceWith(action(t("Send"), "/api/say", () => ({ id: item.id, text: images.value() })));
    };
    const images = attach(correction);
    add(el("div", { className: "reply" }, correction, images.node));
    // Outside the zones, the dev may go on without reading: marked so.
    const unread = item.cycle?.sensitive ? null
      : Object.assign(action(t("Start without validating"), "/api/start", { id: item.id }),
        { title: t("The work starts on this design as it is; the task is marked \"not validated\"") });
    actions.append(dropButton(item.id), el("span", { className: "spacer" }), fix, ...(unread ? [unread] : []),
      action(t("Validate"), "/api/answer", { id: item.id, accept: true, correction: "" }, { primary: true }));
  } else if (item.kind === "report") {
    add(conversation(item), ...reportView(item, true));
    const text = el("textarea", { placeholder: t("Ask more, or ask for another report"), ariaLabel: t("Ask more, or ask for another report"), rows: 2 });
    const images = attach(text);
    add(el("div", { className: "reply" }, text, images.node));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      Object.assign(action(t("Send"), "/api/say", () => ({ id: item.id, text: images.value() })),
        { title: t("fennec reads your answer and goes on with its analysis") }),
      Object.assign(action(t("Close"), "/api/close", { id: item.id }, { primary: true }),
        { title: t("Done with this report: it goes to the history") }));
  } else if (item.kind === "paused") {
    // A stop of the dev's loop: the code is done, they look before the rest.
    add(el("div", { className: "muted", textContent: t("Stopped after the code, as your loop asks: look at the work, then continue with the review and the proof.") }),
      ...cycleView(item), ...work(item));
    if (item.amend_error) add(el("div", { className: "bad", textContent: t("Correction not done, the work stayed as it was: {0}", t(firstLine(item.amend_error))) }));
    // A change asked here goes on to the review and the proof once done.
    const change = el("textarea", { placeholder: t("What to change"), ariaLabel: t("What to change"), hidden: true });
    const fix = el("button", { type: "button", textContent: t("Correct") });
    fix.onclick = () => {
      change.hidden = false;
      change.focus();
      fix.replaceWith(action(t("Send"), "/api/amend", () => ({ id: item.id, text: images.value() })));
    };
    const images = attach(change);
    add(el("div", { className: "reply" }, change, images.node));
    actions.append(action(t("Continue"), "/api/continue", { id: item.id }, { primary: true }), fix,
      changesButton(item, () => node.querySelector(".changes-slot")),
      codeButton(item, () => node.querySelector(".changes-slot")), el("span", { className: "spacer" }), dropButton(item.id, true));
    add(actions, el("div", { className: "error", role: "alert" }), el("div", { className: "changes-slot" }));
  } else if (item.kind === "blocked") {
    // Requests from before the conversation have the question alone.
    const last = item.thread?.at(-1);
    add(...(last?.who === "fennec" && last.text === item.question
      ? [conversation(item)] : [conversation(item), focus(t("fennec asks"), item.question)]));
    const text = el("textarea", { placeholder: t("Your answer"), ariaLabel: t("Your answer") });
    const images = attach(text);
    add(el("div", { className: "reply" }, text, images.node));
    actions.append(dropButton(item.id), el("span", { className: "spacer" }),
      action(t("Answer"), "/api/reply", () => ({ id: item.id, text: images.value() }), { primary: true }));
  } else if (item.kind === "rule") {
    add(focus(t("Proposed rule"), item.rule), el("div", { className: "muted", textContent: t("From what you said: \"{0}\"", item.heard ?? "") }));
    actions.append(action(t("No"), "/api/rule", { id: item.id, scope: "no" }),
      Object.assign(action(t("This project"), "/api/rule", { id: item.id, scope: "project" }), { title: t("Keep the rule for this project") }),
      Object.assign(action(t("Everywhere"), "/api/rule", { id: item.id, scope: "user" }, { primary: true }), { title: t("Keep the rule for all my projects") }));
  } else if (item.kind === "ready") {
    // The decision first, without scrolling: what was proven, what was not,
    // and the buttons. What fennec did and what it cost come under them.
    const tried = item.tries?.length ?? 0;
    // Commands leave no picture: their proof would otherwise hide behind "Details".
    if (tried) {
      add(el("div", { className: "proved",
        textContent: tried > 1 ? t("✓ fennec tried it: {0} tries passed (see Details)", tried) : t("✓ fennec tried it: {0} try passed (see Details)", tried) }));
    }
    const warns = (item.untried ? [t("the product has not been tried yet")] : []).concat(item.unverified ?? []);
    if (warns.length) {
      add(el("div", { className: "muted", style: "margin-top:10px", textContent: t("Not verified — look at it yourself:") }),
        el("ul", { className: "warn" }, ...warns.map((w) => el("li", {}, ...rich(w)))));
    }
    if (item.land_error) {
      add(el("div", { className: "bad", textContent: t("Commit interrupted: {0}", t(firstLine(item.land_error))) }));
      actions.append(action(t("Retry"), "/api/retry", { id: item.id }, { primary: true }));
    }
    if (item.amend_error) add(el("div", { className: "bad", textContent: t("Correction not done, the work stayed as it was: {0}", t(firstLine(item.amend_error))) }));
    if (item.disagreements?.length) {
      add(el("div", { className: "muted", style: "margin-top:10px", textContent: t("Reviewer points the dev refused — your call:") }),
        el("ul", { className: "warn" }, ...item.disagreements.map((d) => el("li", {},
          el("span", { textContent: `${d.point} — ${d.reason ?? ""}` }),
          action(t("Force the point"), "/api/disagree", { id: item.id, point: d.point, action: "force" }),
          action(t("Keep the dev's position"), "/api/disagree", { id: item.id, point: d.point, action: "keep" })))));
    }
    // "Ready" reads as "done": say where the work is until the commit.
    const commit = item.land ? `"${landLabel(item.land)}"` : t("the commit");
    // Nothing changed: what was asked already held, no commit will be made.
    const none = Array.isArray(item.files) && !item.files.length;
    add(el("div", { className: "muted", style: "margin-top:10px", textContent: none
      ? t("fennec changed nothing: what you asked already holds. {0} closes the task without a commit.", commit)
      : item.here
      ? t("The work is in your folder, on a work branch. {0} puts it on your branch; \"Undo\" gives your folder back as it was.", commit)
      : t("Your folder has not changed yet: the work waits apart. {0} puts it there.", commit) }));
    // Ways out, none forced: commit, look, ask for a change, set aside or undo.
    const change = el("textarea", { placeholder: t("What to change"), ariaLabel: t("What to change"), hidden: true });
    const fix = el("button", { type: "button", textContent: t("Correct") });
    fix.onclick = () => {
      change.hidden = false;
      change.focus();
      fix.replaceWith(action(t("Send"), "/api/amend", () => ({ id: item.id, text: images.value() })));
    };
    const images = attach(change);
    // The human review: a sensitive task commits only once it is marked done;
    // another may commit without it, marked "not reviewed".
    const c = item.cycle ?? {};
    const mustReview = c.sensitive && !c.reviewed;
    if (item.land) {
      const land = Object.assign(action(landLabel(item.land), "/api/land", { id: item.id }, { primary: !mustReview }), { title: landTitle(item.land) });
      if (mustReview) {
        land.disabled = true;
        land.title = t("Sensitive task: mark the review done first");
      }
      actions.append(land);
    }
    actions.append(Object.assign(action(c.reviewed ? t("Reviewed ✓") : t("Mark as reviewed"), "/api/reviewed",
      { id: item.id, on: !c.reviewed }, { primary: mustReview }),
      { title: c.reviewed ? t("Take the review mark back") : t("You read the changes against the design and the plan") }));
    if (item.page) actions.append(pageButton(item));
    actions.append(changesButton(item, () => node.querySelector(".changes-slot")),
      codeButton(item, () => node.querySelector(".changes-slot")), fix,
      action(t("Pass"), "/api/pass", { id: item.id }), el("span", { className: "spacer" }), dropButton(item.id, true));
    add(el("div", { className: "reply" }, change, images.node), actions, el("div", { className: "error", role: "alert" }),
      el("div", { className: "changes-slot" }), doneWhen(item), ...cycleView(item),
      c.review ? focusBy(t("What the AI review said"), c.review, "model") : null, ...capitaliseView(item),
      ...work(item), shots(item), ...facts(item));
  } else if (item.kind === "later") {
    // Kept, not started: read in the details, said again there if need be,
    // then launched.
    let images;
    let titleField;
    const save = el("button", { type: "button", textContent: t("Save"), hidden: true });
    save.onclick = async () => {
      const error = errorOf(save);
      error.textContent = "";
      try {
        // The title and the description are apart: each is sent as it stands.
        await api("/api/rewrite", { id: item.id, title: titleField.value, text: images.value() });
      } catch (e) {
        return (error.textContent = e.message);
      }
      // Read again from the file, even unchanged, with a word that it was kept.
      saved = item.id;
      detailed = "";
      refresh(true);
    };
    const edit = el("button", { type: "button", textContent: t("Edit") });
    edit.onclick = () => {
      titleField = el("input", { className: "later-title", value: item.title ?? "", ariaLabel: t("Title") });
      const text = el("textarea", { className: "later-text", defaultValue: item.description ?? "", ariaLabel: t("Description") });
      images = attach(text, item.images);
      const reply = el("div", { className: "reply" },
        el("label", { className: "muted", textContent: t("Title") }), titleField,
        el("label", { className: "muted", textContent: t("Description") }), text, images.node);
      // The fields take the place of the task as the page reads it.
      const read = node.querySelector(".asked");
      if (read) read.replaceWith(reply); else node.prepend(reply);
      edit.hidden = true;
      save.hidden = false;
      titleField.focus();
    };
    // Framed ahead: what fennec understood, for the human to validate or
    // correct. Only a validated task is Ready, and only Ready ones run alone.
    const state = item.splitting ? t("fennec splits it into smaller tasks…")
      : item.fleet ? t("✓ Validated: it runs in its turn when the Fleet runs")
      : item.framed ? t("Read what fennec understood, then validate it or correct it.")
      : item.ahead_failed ? t("fennec could not restate it: edit it to try again, or start it now.")
      : t("fennec restates it when the model is free; you validate it then.");
    // What was said already (a task made from a discussion), then where it stands.
    add(conversation(item), el("div", { className: item.fleet ? "proved" : "muted", textContent: state }),
      item.split_error ? el("div", { className: "error", textContent: t("Could not split it: {0}", item.split_error) }) : null,
      // What it builds on: the Fleet starts it once those are committed.
      item.after?.length ? el("div", { className: "muted", textContent: t("Starts once these are committed: {0}", item.after.join(", ")) }) : null,
      item.framed && item.done_when ? focus(t("What fennec understood"), item.done_when) : null,
      // The skills the work will see, picked at framing among those kept.
      item.framed && Array.isArray(item.picked) ? el("div", { className: "files" }, item.picked.length
        ? t("Skills it will use: {0}", item.picked.join(", ")) : t("No skill needed")) : null);
    let fix = null;
    if (item.framed) {
      const correction = el("textarea", { placeholder: t("What is wrong, or a detail"), hidden: true });
      const notes = attach(correction);
      add(el("div", { className: "reply" }, correction, notes.node));
      fix = el("button", { type: "button", textContent: t("Correct"), title: t("fennec restates it again with your words; it leaves Ready until you validate it") });
      fix.onclick = () => {
        correction.hidden = false;
        correction.rows = 4;
        correction.focus();
        fix.replaceWith(action(t("Send"), "/api/say", () => ({ id: item.id, text: notes.value() })));
      };
    }
    const place = item.fleet
      ? action(t("Back to the Backlog"), "/api/fleet/place", { id: item.id, action: "remove" })
      : item.framed ? action(t("Validate"), "/api/fleet/place", { id: item.id, action: "add" }, { primary: true }) : null;
    // Too big for one go: smaller tasks take its place, in order.
    const split = item.fleet ? null : action(t("Split"), "/api/split", { id: item.id });
    if (split) {
      split.title = t("fennec cuts it into smaller tasks, each after the ones it builds on; they take its place in the Backlog");
      split.disabled = item.splitting;
    }
    actions.append(...[place, fix, action(t("Start now"), "/api/launch", { id: item.id }, { primary: !place }), split, edit, save,
      action(t("Delete"), "/api/forget", { id: item.id }, { quiet: true })].filter(Boolean));
    if (saved === item.id) {
      saved = "";
      const kept = el("div", { className: "proved", textContent: t("✓ Saved") });
      actions.append(kept);
      setTimeout(() => kept.remove(), 3000);
    }
  } else if (item.kind === "failed") {
    add(stages(item), why(item.reason), conversation(item), doneWhen(item), ...work(item), shots(item), ...facts(item));
    // Back to the Backlog, as written: reworded there, launched again or validated.
    actions.append(dropButton(item.id, true), el("span", { className: "spacer" }),
      action(t("Back to the Backlog"), "/api/later", { id: item.id }),
      action(t("Retry"), "/api/retry", { id: item.id }, { primary: true }));
  }
  // What was said to the model so far, on every card with a choice to make.
  actions.querySelector(".spacer")?.after(journalButton(item, () => node.querySelector(".changes-slot")));
  // A ready card has its buttons at the top already; the others put them
  // above the stats too, so the decision does not wait for the figures.
  if (node.contains(actions)) add(details(item));
  else add(el("div", { className: "changes-slot" }), actions, el("div", { className: "error", role: "alert" }), details(item));
  return node;
}

// Why it failed in a sentence; a long tail (the command that failed, its
// output) goes in a block of its own instead of a wall of red.
function why(reason) {
  const text = explain(reason) || t("unknown reason");
  // The first colon, French spaced (" : ") or English (": ").
  const at = text.length > 140 ? text.search(/ ?: /) : -1;
  if (at < 0) return el("div", { className: "bad", textContent: text });
  const rest = text.slice(at).replace(/^ ?: /, "");
  return el("div", {}, el("div", { className: "bad", textContent: text.slice(0, at) }),
    el("pre", { className: "why", textContent: rest }));
}

// When the work started straight from the request, "done when" is the
// request itself: the title says it already.
function doneWhen(item) {
  const same = (item.done_when ?? "").trim() === (item.title ?? "").trim();
  return item.done_when && !same ? focus(t("Done when"), item.done_when) : null;
}

// A step the server says with a task's name in it.
function stepText(step) {
  const m = step.match(/^waits its turn: (.*) holds the folder$/s);
  if (m) return t("waits its turn: {0} holds the folder", m[1]);
  const after = step.match(/^waits for "(.*)"$/s);
  return after ? t("waits for \"{0}\"", after[1]) : t(step);
}

function line(item, state, dot) {
  // A state, when one is said: the requests say theirs, a discussion says none.
  const stateNode = Array.isArray(state)
    ? el("span", { className: "status " + state[1], textContent: t(state[0]) })
    : state ? el("span", { className: "state", textContent: stepText(state) }) : null;
  // Under way, the figures are the last finished step's: how long it has run says more.
  if (dot && item.since) {
    // Queued, not working: a static age, redrawn with the list, no tick.
    const elapsed = item.waiting
      ? el("span", { textContent: t("for {0}", duration(Date.now() - item.since * 1000)) })
      : el("span", { className: "elapsed" });
    if (!item.waiting) elapsed.dataset.since = item.since;
    return el("div", { className: "line" }, el("span", { className: item.waiting ? "dot idle" : "dot" }),
      el("span", { className: "muted proj", textContent: item.project }),
      el("span", { className: "what clip", title: item.title, textContent: headline(item.title) }),
      el("span", { className: "state" }, stateNode, stateNode ? " · " : null, elapsed));
  }
  const s = item.stats;
  return el("div", { className: "line" }, dot ? el("span", { className: "dot" }) : null,
    el("span", { className: "muted proj", textContent: item.project }),
    el("span", { className: "what clip", title: item.title, textContent: headline(item.title) }),
    s?.turns ? el("span", { className: "state cost", title: t("model time · tokens sent · generated") },
      el("span", { className: "mini", textContent: `⏱ ${duration(s.ms)}` }), el("span", { className: "mini", textContent: `↑ ${compact(s.sent)}` }),
      el("span", { className: "mini", textContent: `↓ ${compact(s.received)}` })) : null,
    stateNode);
}

// A finished request opens on what was asked, done and proven: read in full,
// so no toggle in front of it — that fold was the list's, not the page's. The
// title and where it stands are the head above; the box starts at the work.
function past(item) {
  const box = el("div", { className: "past" });
  box.dataset.key = "past-" + item.id;
  const steps = stages(item);
  if (steps) box.append(steps);
  if (item.kind === "dropped" && item.reason) box.append(el("div", { className: "muted", textContent: explain(item.reason) }));
  const said = askedOf(item);
  if (said) box.append(said);
  const talk = conversation(item);
  if (talk) box.append(talk);
  const done = doneWhen(item);
  if (done) box.append(done);
  if (item.kind === "analysed") box.append(...reportView(item, false));
  // What was decided before the code, and what the human skipped.
  box.append(...cycleView(item), ...work(item));
  const tried = item.tries?.length ?? 0;
  if (tried) box.append(el("div", { className: "proved", textContent: tried > 1 ? t("✓ fennec tried it: {0} tries passed", tried) : t("✓ fennec tried it: {0} try passed", tried) }));
  if (item.unverified?.length) {
    box.append(el("div", { className: "muted", textContent: t("Not verified:") }),
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
  if (item.kind === "passed") actions.append(action(t("Resume"), "/api/resume", { id: item.id }, { primary: true }));
  actions.append(action(t("Archive"), "/api/request/archive", { id: item.id }, { quiet: true }), forgetButton(item.id));
  box.append(slot, el("div", { className: "past-actions" }, actions, el("div", { className: "error", role: "alert" })));
  return box;
}

function pastState(item) {
  if (item.kind === "analysed") return "✓ " + t("analysed");
  if (item.kind === "passed") return t("set aside");
  if (item.kind === "dropped") return t("dropped");
  if (item.files?.length === 0) return "✓ " + t("nothing to change");
  const parts = [item.branch, item.remote && t("pushed to {0}", item.remote), item.commit?.slice(0, 7)];
  return "✓ commit " + parts.filter(Boolean).join(" · ");
}

// Where a request stands, in one coloured word: the list says it per row in
// the muted, the page says it in a pill the human sees at a glance.
function statusOf(item, kind) {
  if (kind === "run") return item.waiting ? [t("waiting"), "muted"] : [t("running"), "info"];
  if (kind === "past") {
    if (item.kind === "passed") return [t("set aside"), "warn"];
    if (item.kind === "dropped") return [t("dropped"), "bad"];
    return [pastState(item), "ok"];
  }
  if (item.kind === "later" && item.fleet) return [t("Ready to run"), "info"];
  const [label, color] = TAGS[item.kind];
  return [t(label), color.replace("var(--", "").replace(")", "")];
}

// A request's first paragraph: what it asks. What follows (a plan step's
// whole plan, say) is context, read in the details.
const lead = (text) => (text ?? "").trim().split(/\n\s*\n/)[0];

// The request read in full: its title and where it stands, above what it
// holds. The title is not repeated in the card, the past or the run.
function reqHead(item, kind) {
  const [label, tone] = statusOf(item, kind);
  const head = el("div", { className: "req-head" },
    el("strong", { textContent: lead(item.title) }),
    el("span", { className: `status ${tone}`, textContent: label }));
  if (kind === "past" && item.stats?.turns) {
    const s = item.stats;
    head.append(el("span", { className: "muted", textContent: `⏱ ${duration(s.ms)} · ↑ ${compact(s.sent)} · ↓ ${compact(s.received)}` }));
  }
  // The right bar (decisions and figures) folds away: the icon stays in the head.
  const facts = el("button", { type: "button", className: "link", id: "toggle-facts" });
  setIcon(facts, "panelRight");
  const paint = () => facts.setAttribute("aria-label",
    facts.setAttribute("title", document.body.classList.contains("facts-off")
      ? t("Show the cost") : t("Hide the cost")));
  paint();
  facts.onclick = () => { setFacts(!document.body.classList.contains("facts-off")); paint(); };
  head.append(facts);
  return head;
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
  const prompt = projects.length ? t("All projects") : t("No project");
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
    // The Agent mode or a page in view: no task list is current.
    const tasks = where().path !== "chat" && !PAGES.includes(where().path);
    button.setAttribute("aria-current", String(tasks && $("project").value === path));
    button.onclick = () => pick(path);
    button.querySelectorAll(".count, .dot").forEach((e) => e.remove());
    if (path && view.running.some((i) => i.path === path)) button.append(el("span", { className: "dot", title: t("fennec works on it") }));
    if (n) button.append(el("span", { className: "count", title: t("waits for you"), textContent: n }));
    return button;
  };
  // The Tasks mode stays in sight, even with one project.
  entry($("side-all"), "", view.todo);
  $("side-projects").replaceChildren(...view.projects.map((p) => projectEntry(p, entry, view.todo)));
  // The archived projects, out of the select: brought back or erased there.
  const archived = view.projectsArchived ?? [];
  $("side-archived-label").hidden = !archived.length;
  $("side-archived").replaceChildren(...archived.map(archivedProjectEntry));
  const page = PAGES.includes(where().path) ? where().path : "";
  $("open-team").setAttribute("aria-current", String(page === "team"));
  $("open-stats").setAttribute("aria-current", String(page === "stats"));
  $("open-settings").setAttribute("aria-current", String(page === "settings"));
}

// Which project is on screen: the focused rows no longer repeat it, so it is
// said once, at the top of the pane that shows it.
function sayProject(head) {
  const path = $("project").value;
  head.querySelector("strong").textContent = $("project").selectedOptions[0]?.text ?? path.split("/").pop();
  head.querySelector(".muted").textContent = path;
}
function heads() {
  const show = !!$("project").value && where().path !== "chat";
  $("list-head").hidden = !show;
  if (show) sayProject($("list-head"));
}
// The request read in full gets the head; the pages name themselves.
function detailHead(found) {
  const head = $("detail-head");
  head.hidden = !(found && $("project").value);
  if (!head.hidden) sayProject(head);
}

// A project's two exits in the list: archived (its requests go out of view
// with it) or erased, which the server refuses while it holds a request.
function sideCall(path, body) {
  const error = $("side-error");
  error.textContent = "";
  api(path, body).then(() => refresh(true)).catch((e) => { error.textContent = e.message; });
}
function projectEntry(p, entry, todo) {
  const button = el("button", { type: "button", className: "item", title: p.path },
    el("span", { className: "name", textContent: p.name }));
  const archive = el("button", { type: "button", className: "icon", ariaLabel: t("Archive the project"), title: t("Archive") });
  setIcon(archive, "inbox");
  archive.onclick = () => sideCall("/api/projects/archive", { path: p.path, on: true });
  const del = el("button", { type: "button", className: "icon", ariaLabel: t("Delete the project"), title: t("Delete") });
  setIcon(del, "trash");
  del.onclick = () => {
    const sure = el("button", { type: "button", className: "quiet", textContent: t("Confirm") });
    sure.style.color = "var(--bad)";
    sure.onclick = () => sideCall("/api/projects/remove", { path: p.path });
    del.replaceWith(sure);
  };
  entry(button, p.path, todo);
  return el("li", { className: "project" }, button, archive, del);
}
function archivedProjectEntry(p) {
  const restore = el("button", { type: "button", className: "icon", ariaLabel: t("Restore the project"), title: t("Restore") });
  setIcon(restore, "undo");
  restore.onclick = () => sideCall("/api/projects/archive", { path: p.path, on: false });
  const del = el("button", { type: "button", className: "icon", ariaLabel: t("Delete the project"), title: t("Delete") });
  setIcon(del, "trash");
  del.onclick = () => {
    const sure = el("button", { type: "button", className: "quiet", textContent: t("Confirm") });
    sure.style.color = "var(--bad)";
    sure.onclick = () => sideCall("/api/projects/remove", { path: p.path });
    del.replaceWith(sure);
  };
  return el("li", {}, el("span", { className: "name", title: p.path, textContent: p.name }), restore, del);
}

// The chosen project is the one on screen: its requests only, kept in the
// address so a reload or a bookmark comes back to it.
function focusOn(path) {
  if (owned(where().path)) return;
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
// The list is redrawn on every refresh: a section the human unfolded must
// stay unfolded, so the opened folders are remembered here.
const chatOpen = new Set();
async function showChats() {
  document.body.classList.add("chats");
  // Narrow: the list of discussions gives way only to an open one.
  document.body.classList.toggle("picked", !!where().id);
  // Coming from Team, Stats or Settings: their class hides the list column.
  document.body.classList.remove("paged");
  $("chats").hidden = false;
  $("page").hidden = true;
  $("hint").hidden = true;
  for (const name of PAGES) $(name).hidden = true;
  $("side-chats").setAttribute("aria-current", "true");
  $("side-all").setAttribute("aria-current", "false");
  let chats, archived;
  try {
    [chats, archived] = await Promise.all([api("/api/chats"), api("/api/chats/archived")]);
  } catch (e) {
    $("chat-list").replaceChildren(el("div", { className: "empty", textContent: e.message }));
    return;
  }
  const nodes = chats.length
    ? chatGroups(chats)
    : [el("div", { className: "empty", textContent: t("No discussion: \"New discussion\" starts one.") })];
  // The archived discussions, out of the groups: brought back or erased there.
  if (archived.length) nodes.push(el("div", { className: "chat-archived" },
    el("h2", { textContent: t("Archived") }), ...archived.map((c) => chatRow(c, true, true))));
  $("chat-list").replaceChildren(...nodes);
  const id = where().id;
  if (!id) {
    chatBox = null;
    detailed = "";
    return $("pick").replaceChildren(el("p", { className: "muted", textContent: chats.length
      ? t("Pick a discussion on the left to read and go on, or \"New discussion\" to start one.")
      : t("\"New discussion\" starts one.") }));
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

// A discussion of one folder: under its folder's name, the folder is not
// repeated on the row; the row offers archiving and, one click more, erasing.
function chatRow(c, grouped, restore = false) {
  const a = el("a", { className: "row", href: "#chat/" + c.id },
    line({ id: c.id, project: grouped ? "" : c.folder.split("/").pop(), title: c.title, since: c.since },
      "",
      c.status === "thinking"));
  a.setAttribute("aria-current", String(c.id === where().id));
  return rowWrap(a, rowActions(c.id, { archive: "/api/chat/archive", forget: "/api/chat/forget", restore }));
}

// One folder's discussions under its name, a project marked: folded at first,
// the count says what is inside; a click unfolds.
function chatGroup(folder, g) {
  const open = chatOpen.has(folder);
  const head = el("button", { type: "button", className: "chat-group-head", title: folder },
    el("span", { className: "what", textContent: folder.split("/").pop() }),
    g.project ? el("span", { className: "tag", style: "--c: var(--ok)", textContent: t("project") }) : null,
    el("span", { className: "count", textContent: g.rows.length }));
  head.setAttribute("aria-expanded", String(open));
  setIcon(head, "chevron");
  const body = el("div", { className: "chat-group-body", hidden: !open }, ...g.rows);
  head.onclick = () => {
    const now = body.hidden;
    body.hidden = !now;
    head.setAttribute("aria-expanded", String(now));
    if (now) chatOpen.add(folder); else chatOpen.delete(folder);
  };
  return el("div", { className: "chat-group" }, head, body);
}

// The discussions of one folder under its name, a project marked; within a
// folder, the most recent first; folders in the order of their names.
function chatGroups(chats) {
  const groups = new Map();
  for (const c of chats) {
    const g = groups.get(c.folder) ?? { project: c.project === true, rows: [] };
    g.rows.push(chatRow(c, true));
    groups.set(c.folder, g);
  }
  // By the name shown, not the path: two trees may hold a same-named folder.
  return [...groups.entries()]
    .sort(([a], [b]) => a.split("/").pop().localeCompare(b.split("/").pop()) || a.localeCompare(b))
    .map(([folder, g]) => chatGroup(folder, g));
}

function showChat(c) {
  const thinking = c.status === "thinking";
  const fresh = !chatBox || chatBox.id !== c.id;
  if (fresh) {
    const composer = chatComposer(c.id);
    const head = el("div", { className: "chat-head" });
    const thread = el("div", { className: "thread" });
    const aside = el("aside", { className: "chat-side" });
    chatBox = { id: c.id, composer, head, thread, aside, asideKey: "", threadKey: "" };
    detailed = "";
    $("pick").replaceChildren(el("div", { className: "split" },
      el("div", { className: "card chat" }, head, thread, composer.node,
        el("div", { className: "error", id: "chat-error", role: "alert" })),
      aside));
  }
  const box = chatBox;
  box.head.replaceChildren(
    el("div", { className: "meta", style: `--c: ${c.project ? "var(--ok)" : "var(--muted)"}` },
      el("span", { className: "tag", textContent: c.project ? t("Project") : t("Folder") }),
      el("span", { className: "proj", textContent: c.folder })),
    el("div", { className: "title", textContent: chatTitle(c) }));
  // What was opened stays open as new messages come in; the page follows the
  // turn to the bottom, unless the person scrolled up to read.
  const detail = $("detail");
  const follow = fresh || detail.scrollHeight - detail.scrollTop - detail.clientHeight < 80;
  // Redrawn only when it moved: a redraw every refresh would drop the text
  // the person is selecting and parse the whole exchange again.
  const threadKey = JSON.stringify([c.messages, c.status, c.pending, c.error]);
  if (box.threadKey !== threadKey) {
    box.threadKey = threadKey;
    const opened = [...box.thread.querySelectorAll("details")].map((d) => d.open);
    // A task's conversation is answered on the task's page: no edit or
    // regenerate here, whose turn would never reach the framing.
    box.thread.replaceChildren(...chatThread(c, c.task ? null : box.composer));
    if (thinking) box.thread.append(el("div", { className: "thinking" }, el("span", { className: "dot" }), t("fennec is answering…")));
    // A turn that waits on the person is the last thing on screen: on top of
    // the exchange, it would read as a turn that stopped answering.
    if (c.status === "waiting" && c.pending) {
      box.thread.append(chatQuestion(c));
      box.thread.append(el("div", { className: "thinking" }, el("span", { className: "dot" }), t("fennec waits for your answer")));
    }
    if (c.error) box.thread.append(el("div", { className: "bad", textContent: explain(c.error) }));
    box.thread.querySelectorAll("details").forEach((d, i) => { d.open = opened[i] ?? false; });
  }
  const busy = thinking || c.status === "waiting";
  box.composer.send.disabled = busy;
  box.composer.stop.hidden = !busy;
  // A task's conversation is answered on the task's page: its brief goes to
  // the framing from there.
  box.composer.node.hidden = c.task === true;
  const asideKey = JSON.stringify([c.stats, c.project, c.grants, c.write, c.criteria, c.verify, c.checks, c.task, c.request]);
  if (box.asideKey !== asideKey) {
    box.asideKey = asideKey;
    box.aside.replaceChildren(...chatAside(c));
  }
  if (follow) detail.scrollTop = detail.scrollHeight;
}

// A call named a path outside the folder the person chose: fennec asks
// before running it. "Open" opens that path's folder for the chat; the
// refusal covers this call only.
function chatQuestion(c) {
  const p = c.pending ?? {};
  const open = el("button", { type: "button", className: "primary", textContent: t("Open the folder") });
  const refuse = el("button", { type: "button", className: "quiet", textContent: t("Refuse") });
  // Set once one of the person's rules asks first.
  let chat = null;
  const decide = async (allow, forChat) => {
    $("chat-error").textContent = "";
    for (const b of [open, refuse, chat]) if (b) b.disabled = true;
    try {
      await api("/api/chat/decide", forChat ? { id: c.id, allow, for_chat: true } : { id: c.id, allow });
    } catch (e) {
      $("chat-error").textContent = e.message;
    }
  };
  open.onclick = () => decide(true);
  refuse.onclick = () => decide(false);
  // One of the person's rules asks first: once, for the rest of the chat,
  // or not at all (the call does not run, fennec does without).
  if (p.tool === "rule") {
    chat = el("button", { type: "button", textContent: t("For this discussion") });
    open.textContent = t("Allow once");
    chat.onclick = () => decide(true, true);
    const rule = t("rule \"{0}\"", p.rule?.pattern ?? "");
    const why = p.rule?.why ? `${t(p.rule.why)} (${rule})` : rule;
    return el("div", { className: "ask" },
      el("span", { className: "muted", textContent: t("fennec asks: {0}", why) }),
      el("code", { title: p.call, textContent: p.call ?? p.target }),
      el("div", { className: "chat-actions" }, open, chat, refuse));
  }
  // A plan to approve: approved, the folder opens for changes and fennec
  // applies it; otherwise the discussion goes on, read-only.
  if (p.tool === "propose_plan") {
    open.textContent = t("Apply the plan");
    refuse.textContent = t("Keep discussing");
    const actions = el("div", { className: "chat-actions" }, open);
    // A plan in steps (as fennec gave them, or else its numbered sections):
    // each step can become a task of its own, kept for later; a folder that
    // is no project yet becomes one, in place, as the project picker would
    // make it. The turn ends there.
    const steps = p.steps?.length > 1 ? p.steps : planSteps(p.plan ?? "");
    if (steps.length > 1) {
      const split = el("button", { type: "button", textContent: t("Make {0} tasks of it", steps.length) });
      if (!c.project) split.title = t("The folder becomes a project");
      split.onclick = async () => {
        $("chat-error").textContent = "";
        split.disabled = true;
        try {
          if (!c.project) await api("/api/projects", { path: c.folder, place: "here" });
          let prev = null;
          for (const s of steps) {
            // Each task holds its own step, not the whole plan again, and
            // starts once the step before it is committed.
            prev = (await api("/api/requests", { project: c.folder, text: s, later: true, after: prev ? [prev] : [] })).id;
          }
          await api("/api/chat/stop", { id: c.id });
          location.hash = hashOf(c.folder);
        } catch (e) {
          split.disabled = false;
          $("chat-error").textContent = e.message;
        }
      };
      actions.append(split);
    }
    actions.append(refuse);
    return el("div", { className: "ask plan" },
      el("span", { className: "muted", textContent: t("fennec proposes this plan and asks to change the folder") }),
      el("div", { className: "said" }, ...md(p.plan ?? "")),
      ...(p.checks?.length ? [el("span", { className: "muted", textContent: t("Checked after each change with:") }),
        ...p.checks.map((x) => el("code", { textContent: x }))] : []),
      actions);
  }
  return el("div", { className: "ask" },
    el("span", { className: "muted", textContent: t("fennec asks") }),
    el("code", { title: p.path, textContent: p.path }),
    el("span", { className: "muted", title: p.directory, textContent: p.directory }),
    open, refuse);
}

// The numbered sections of a plan ("### 1. …"), each up to the next heading
// as high as its own; what comes before or after them stays out.
function planSteps(plan) {
  const steps = [];
  let level = 0;
  for (const line of plan.split("\n")) {
    const h = line.match(/^(#{1,6})\s+(\d+[.)])?/);
    if (h && (!level || h[1].length <= level)) {
      if (h[2]) {
        level = h[1].length;
        steps.push([line.replace(/^#+\s*/, "")]);
        continue;
      }
      if (level) level = -1;
    }
    if (level > 0) steps.at(-1).push(line);
  }
  return steps.map((s) => s.join("\n").trim());
}

function chatTitle(c) {
  const first = (c.messages ?? []).find((m) => m.role === "user")?.content;
  if (!first) return t("New discussion");
  return headline(first);
}

// A name for a list: the first line written, cut at a word near 50 characters.
function headline(text) {
  const line = (text.split("\n").find((l) => l.trim()) ?? "").trim();
  if (line.length <= 50) return line;
  const cut = line.lastIndexOf(" ", 50);
  return line.slice(0, cut > 30 ? cut : 50).replace(/[\s:,;.+—-]+$/, "") + "…";
}

// The exchange: the human's words in bubbles, fennec's in Markdown, the tool
// calls folded, each with what it brought back.
function chatThread(c, acts) {
  const calls = new Map();
  for (const m of c.messages ?? []) for (const call of m.tool_calls ?? []) calls.set(call.id, call);
  const results = new Map();
  for (const m of c.messages ?? []) if (m.role === "tool") results.set(m.tool_call_id, m.content);
  const nodes = [];
  // The person's message a reply answers: the one regenerating it says again.
  let asked = null;
  for (const [i, m] of (c.messages ?? []).entries()) {
    // What fennec's harness said to the model (back to the plan, notes
    // after a long exchange) is not the person's: the Journal keeps it.
    if (m.role === "tool" || m.harness) continue;
    if (m.role === "user") {
      asked = i;
      nodes.push(el("div", { className: "msg human" },
        el("div", { className: "said human" },
          el("span", { className: "label", textContent: t("You") }), ...rich(m.content)),
        acts && msgActions(copyButton(m.content),
          iconButton("pencil", t("Edit"), () => acts.again(i, m.content)))));
      continue;
    }
    const box = el("div", { className: "said" }, el("span", { className: "label", textContent: "fennec" }));
    if (m.content) box.append(...md(m.content));
    for (const call of m.tool_calls ?? []) box.append(toolCall(call, results.get(call.id)));
    const from = asked;
    nodes.push(el("div", { className: "msg" }, box,
      acts && m.content && msgActions(copyButton(m.content),
        iconButton("replay", t("Regenerate"), () => acts.replay(from, c.messages[from].content)))));
  }
  // The last message keeps its icons in sight, as chat apps do.
  if (nodes.length) nodes.at(-1).className += " last";
  return nodes.length ? nodes : [el("div", { className: "muted", textContent: t("Write what you are after: fennec reads the folder and answers.") })];
}

// The small icons under a message, as chat apps have them.
function msgActions(...buttons) {
  return el("div", { className: "msg-actions" }, ...buttons);
}

function iconButton(icon, label, onclick) {
  const b = el("button", { type: "button", className: "quiet", ariaLabel: label, title: label });
  setIcon(b, icon);
  b.onclick = onclick;
  return b;
}

function copyButton(text) {
  const b = iconButton("copy", t("Copy"), async () => {
    try {
      await navigator.clipboard.writeText(text);
      setIcon(b, "check");
      setTimeout(() => setIcon(b, "copy"), 1500);
    } catch { /* no clipboard: nothing to say */ }
  });
  return b;
}

function toolCall(call, result) {
  const args = call.function?.arguments ?? "";
  const head = `→ ${call.function?.name ?? "?"} ${String(args).replace(/\s+/g, " ").slice(0, 110)}`;
  return el("details", { className: "message" },
    el("summary", {}, head),
    args ? el("pre", { textContent: args }) : null,
    el("pre", { textContent: result ?? "…" }));
}

function chatComposer(id) {
  const text = el("textarea", { placeholder: t("Your message"), ariaLabel: t("Your message") });
  const images = attach(text);
  const stop = el("button", { type: "button", className: "stop", textContent: t("Stop"), hidden: true });
  setIcon(stop, "stop");
  const send = el("button", { type: "button", className: "primary", textContent: t("Send") });
  // Set when one of the person's messages is being said again.
  let from = null;
  const cancel = el("button", { type: "button", className: "quiet", textContent: t("Cancel") });
  const replaying = el("div", { className: "replaying", hidden: true },
    el("span", { className: "muted", textContent: t("You are saying this message again: the rest of the exchange will be erased, changed files stay as they are.") }),
    cancel);
  const replayOff = () => { from = null; replaying.hidden = true; send.textContent = t("Send"); };
  cancel.onclick = () => { replayOff(); images.clear(); };
  const again = (index, content) => {
    from = index;
    text.value = content;
    replaying.hidden = false;
    send.textContent = t("Say again");
    text.focus();
  };
  // Regenerating a reply: the message it answers, said again as it was.
  const replay = async (index, content) => {
    $("chat-error").textContent = "";
    try { await api("/api/chat/replay", { id, index, text: content }); }
    catch (e) { $("chat-error").textContent = e.message; }
  };
  const say = async () => {
    if (!images.value().trim()) return;
    try {
      if (from === null) await api("/api/chat/say", { id, text: images.value() });
      else await api("/api/chat/replay", { id, index: from, text: images.value() });
      replayOff();
      images.clear();
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
  return { node: el("div", { className: "chat-composer" }, replaying, text,
    el("div", { className: "chat-actions" }, images.node, stop, el("span", { className: "spacer" }), send)),
    send, stop, text, again, replay };
}

function chatAside(c) {
  const nodes = [];
  // A task's conversation: read-only, it ends on the task's brief.
  if (c.task) {
    nodes.push(el("h2", { textContent: t("Task") }),
      el("p", { className: "muted", textContent: t("This discussion shapes a task: answer on its page.") }),
      el("a", { className: "link", href: hashOf(c.folder, c.request), textContent: t("Open the task") }));
    if (c.stats?.turns) nodes.push(el("h2", { textContent: t("Cost") }), metrics(c.stats));
    return nodes;
  }
  // Read-only until the person allows changes: talking about a project is
  // not asking to build it.
  const write = c.write === true;
  const toggle = el("button", { type: "button", className: write ? "quiet" : "primary",
    textContent: write ? t("Back to read-only") : t("Allow changes") });
  setIcon(toggle, write ? "lock" : "pencil");
  toggle.onclick = async () => {
    $("chat-error").textContent = "";
    try {
      await api("/api/chat/write", { id: c.id, on: !write });
      refresh(true);
    } catch (e) {
      $("chat-error").textContent = e.message;
    }
  };
  nodes.push(el("h2", { textContent: t("Mode") }),
    el("p", { className: "muted", textContent: write
      ? t("fennec changes the folder directly.")
      : t("Read-only: fennec reads, discusses and proposes a plan.") }),
    toggle);
  // Folders the person opened for this chat: a refusal was not, an opening
  // stays until taken back.
  const grants = c.grants ?? [];
  if (grants.length) {
    const ul = el("ul");
    for (const g of grants) {
      const b = el("button", { type: "button", textContent: t("Remove") });
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
    nodes.push(el("h2", { textContent: t("Opened folders") }), ul);
  }
  // The approved plan's steps: the turn works through them, one by one.
  const plan = (c.criteria ?? []).filter((s) => s.state !== "rule");
  if (plan.length) {
    const met = plan.filter((s) => s.state === "done" || s.state === "passed").length;
    const ul = el("ul", { className: "plan" });
    for (const s of plan) {
      const ok = s.state === "done" || s.state === "passed";
      const li = el("li", { className: ok ? "ok" : s.state === "failed" ? "bad" : "" },
        el("span", { className: "what", title: s.text, textContent: s.text }));
      if (ok) setIcon(li, "check");
      ul.append(li);
    }
    nodes.push(el("h2", { textContent: t("Plan · {0}/{1}", met, plan.length) }), ul);
  }
  // The dev loop: what fennec runs after each turn that changed the
  // folder, with the last results once they ran.
  const results = c.checks?.results ?? (c.verify ?? []).map((command) => ({ command }));
  if (results.length) {
    const passed = results.filter((r) => r.exit === 0).length;
    const ul = el("ul", { className: "plan" });
    for (const r of results) {
      const li = el("li", { className: r.exit === 0 ? "ok" : r.exit == null ? "" : "bad" },
        el("code", { className: "what", title: r.output ?? "", textContent: r.command }));
      if (r.exit === 0) setIcon(li, "check");
      else if (r.exit != null) setIcon(li, "close");
      ul.append(li);
      if (r.exit != null && r.exit !== 0 && r.output) ul.append(el("li", { className: "out" }, el("pre", { textContent: r.output.split("\n").slice(-8).join("\n") })));
    }
    nodes.push(el("h2", { textContent: t("Checks · {0}/{1}", passed, results.length) }), ul);
  }
  if (c.stats?.turns) nodes.push(el("h2", { textContent: t("Cost") }), metrics(c.stats));
  const button = el("button", { type: "button", className: "convert",
    textContent: c.project ? t("Make it a task") : t("Make it a project") });
  button.onclick = async () => {
    $("chat-error").textContent = "";
    try {
      if (c.project) {
        // The exchange becomes the request's opening, kept in the backlog.
        const made = await api("/api/chat/request", { id: c.id });
        location.hash = hashOf(c.folder, made.id);
      } else {
        openPicker("project", t("Make it a project"), c.folder);
      }
    } catch (e) {
      $("chat-error").textContent = explain(e.message);
    }
  };
  nodes.push(el("h2", { textContent: c.project ? t("Task") : t("Project") }), button);
  return nodes;
}

let shown = "";
async function refresh(force) {
  // Never wipe what the human is typing in the request read in full; the
  // rest keeps up. (Holding the whole page whenever focus sat in that pane
  // froze it after a mere click on a tab.)
  const typing = !force && ([...document.querySelectorAll("#detail textarea, #detail input")].some((f) => f.value !== f.defaultValue)
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
        heads();
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
    heads();
    const todo = mine(view.todo), running = mine(view.running);
    const row = (item, node, actions) => {
      const a = el("a", { className: "row", href: hashOf($("project").value, item.id) }, node);
      a.setAttribute("aria-current", String(item.id === picked));
      return actions ? rowWrap(a, actions) : a;
    };
    fill($("todo"), todo.map((i) => row(i, line(i, TAGS[i.kind][0]))), t("Nothing waits for you."));
    $("n-todo").textContent = todo.length || "";
    // Seen from another tab: what waits for the human, in the tab's title.
    const others = view.todo.length - todo.length;
    $("n-others").textContent = !others ? "" : others > 1 ? t("{0} others wait for you elsewhere", others) : t("{0} other waits for you elsewhere", others);
    const name = $("project").selectedOptions[0]?.value ? $("project").selectedOptions[0].text : "fennec";
    document.title = todo.length ? `(${todo.length}) ${name}` : name;
    $("n-running").textContent = running.length || "";
    fill($("running"), running.map((i) => row(i, line(i, i.step, true),
      rowActions(i.id, { stop: "/api/stop" }))), t("Nothing running."));
    // The Backlog, then what was validated, in the order the Fleet runs it.
    const later = mine(view.later ?? []);
    const backlog = later.filter((i) => !i.fleet);
    const ready = later.filter((i) => i.fleet).sort((a, b) => a.rank - b.rank);
    // Ticked rows survive a redraw: the list moves under the human's hand.
    ticked = new Set([...ticked].filter((id) => later.some((i) => i.id === id)));
    const picks = (name, items, label, send) => {
      $("n-" + name).textContent = items.length || "";
      const bar = $(name + "-pick");
      fill($(name), items.map((i) => {
        const box = el("input", { type: "checkbox", checked: ticked.has(i.id), ariaLabel: t("Pick {0}", headline(i.title)) });
        box.onchange = () => { box.checked ? ticked.add(i.id) : ticked.delete(i.id); showPicked(bar, items, send); };
        return el("div", { className: "pick-row" }, box, row(i, line(i, label(i)),
          rowActions(i.id, { archive: "/api/request/archive", forget: "/api/forget" })));
      }), "");
      showPicked(bar, items, send);
    };
    picks("later", backlog, (i) => i.splitting ? "being split" : i.framed ? "to validate" : i.ahead_failed ? "could not be restated" : "being restated",
      [t("Validate"), "add", (i) => i.framed]);
    picks("ready", ready, (i) => i.after?.length ? t("#{0} in line, waits for \"{1}\"", i.rank + 1, i.after[0]) : t("#{0} in line", i.rank + 1), [t("Back to the Backlog"), "remove", () => true]);
    showFleetBar(view.fleet, later.length);
    const history = mine(view.history);
    $("n-history").textContent = history.length || "";
    fill($("history"), history.map((i) => row(i, line(i, pastState(i), false),
      rowActions(i.id, { archive: "/api/request/archive", forget: "/api/forget" }))), t("Nothing finished yet."));
    // The archived requests, out of the lists: brought back or erased there.
    const archived = mine(view.archived ?? []);
    $("archived-box").hidden = !archived.length || listTab !== "history";
    $("n-archived").textContent = archived.length || "";
    fill($("archived"), archived.map((i) => row(i, line(i, pastState(i), false),
      rowActions(i.id, { archive: "/api/request/archive", forget: "/api/forget", restore: true }))), "");
    homeView = view;
    if (!typing) showDetail(todo.map((i) => [i, "todo"]).concat(later.map((i) => [i, "todo"]), running.map((i) => [i, "run"]), history.map((i) => [i, "past"]))
      .find(([i]) => i.id === picked));
    tick();
    // How it works, until the first request: the page alone must explain itself.
    $("how").hidden = view.todo.length + view.later.length + view.running.length + view.history.length > 0;
    // Its figures move as its requests do.
    if (paged && !force) showPage(paged).catch(() => {});
  } catch (e) {
    $("error").textContent = t("Server unreachable: {0}", e.message);
  }
}

// The last visit: when the page was last left. What moved since stands out.
let seenBefore = 0;
const readSeen = () => { try { seenBefore = Number(localStorage.getItem("fennec.seen")) || 0; } catch {} };
const remember = () => { try { localStorage.setItem("fennec.seen", String(Math.floor(Date.now() / 1000))); } catch {} };
readSeen();
document.addEventListener("visibilitychange", () => (document.hidden ? remember() : readSeen()));
window.addEventListener("pagehide", remember);

// Home: what moved lately, the figures that lead the work, what runs next.
// The figures walk the worlds on disk: read on entry and every minute, not
// at each refresh.
let homeStats = null, homeStatsAt = 0, homeView = null;
function drawHome(view = homeView) {
  homeView = view;
  if (!view || $("hint").hidden) return;
  if (Date.now() - homeStatsAt > 60000) {
    homeStatsAt = Date.now();
    api("/api/stats").then((s) => { homeStats = s; drawHome(view); }).catch(() => {});
  }
  const ready = view.later.filter((i) => i.fleet).sort((a, b) => a.rank - b.rank);
  const toValidate = view.later.filter((i) => !i.fleet && i.framed).length;
  const tile = (value, label, href) => el(href ? "a" : "div", { className: "tile", href }, el("b", { textContent: value }), el("span", { textContent: label }));
  const a = homeStats?.all, mem = homeStats?.memory;
  $("home-tiles").replaceChildren(
    tile(number(view.todo.length), t("for you")), tile(number(view.running.length), t("running")),
    tile(number(ready.length), t("ready to run")), tile(number(toValidate), t("to validate in the Backlog")),
    ...(a ? [tile(duration(a.ms ?? 0), t("of model · ↑ {0} ↓ {1}", compact(a.sent), compact(a.received)), "#stats"),
      tile(bytes(a.disk ?? 0), t("disk of the worlds"))] : []),
    ...(mem?.total ? [tile(`${percent(mem.total - mem.available, mem.total)} %`, t("memory used of {0}", bytes(mem.total)))] : []));
  const link = (item, state, fresh) => el("a", { className: "row" + (fresh ? " fresh" : ""), href: hashOf(item.path, item.id) }, line(item, state));
  const news = [...view.todo.map((i) => [i, statusOf(i, "todo")]), ...view.running.map((i) => [i, statusOf(i, "run")]),
    ...view.history.map((i) => [i, statusOf(i, "past")])].sort(([x], [y]) => (y.since ?? 0) - (x.since ?? 0)).slice(0, 5);
  const fresh = news.filter(([i]) => i.since > seenBefore).length;
  $("n-home-news").textContent = fresh || "";
  fill($("home-news"), news.map(([i, s]) => link(i, s, i.since > seenBefore)), t("Nothing yet."));
}

// A box's ticked tasks, validated (Backlog) or put back (Ready) at once.
let ticked = new Set();
function showPicked(bar, items, [label, action, applies]) {
  const ids = items.filter((i) => ticked.has(i.id));
  bar.hidden = !ids.length;
  if (!ids.length) return bar.replaceChildren();
  const error = el("span", { className: "error", role: "alert" });
  // Only the ticked tasks it applies to: a task not restated yet cannot be validated.
  const which = ids.filter(applies);
  const b = el("button", { type: "button", textContent: which.length === ids.length ? label : `${label} (${which.length})`, disabled: !which.length });
  b.onclick = async () => {
    try {
      for (const i of which) await api("/api/fleet/place", { id: i.id, action });
    } catch (e) {
      error.textContent = e.message;
    }
    ticked.clear();
    refresh(true);
  };
  iconize(b);
  const all = el("button", { type: "button", className: "quiet", textContent: ids.length === items.length ? t("None") : t("All") });
  all.onclick = () => {
    for (const i of items) ids.length === items.length ? ticked.delete(i.id) : ticked.add(i.id);
    refresh(true);
  };
  bar.replaceChildren(el("span", { textContent: t("{0} picked", ids.length) }), all, el("span", { className: "spacer" }), b, error);
}

// The Fleet's switch on top of the Ready tab: whether it runs, how many wait.
function showFleetBar(fleet, later) {
  const stopped = $("fleet-stopped");
  stopped.hidden = !fleet?.stopped;
  stopped.textContent = fleet?.stopped ? t("Stopped on its own: {0}. Look at the task, then start again.", t(fleet.stopped)) : "";
  const bar = $("fleet-bar");
  bar.hidden = !fleet || !(fleet.queued || later);
  if (bar.hidden) return;
  bar.classList.toggle("on", fleet.on && fleet.queued > 0);
  const toggle = el("button", { type: "button", textContent: fleet.on ? t("Stop") : t("Start") });
  setIcon(toggle, fleet.on ? "pause" : "play");
  toggle.onclick = async () => {
    try {
      await api("/api/fleet", { on: !fleet.on });
    } catch (e) {
      $("error").textContent = e.message;
    }
    refresh(true);
  };
  bar.replaceChildren(el("span", { className: "muted",
    textContent: t(fleet.on ? "running · {0} queued" : "stopped · {0} queued", fleet.queued) }), el("span", { className: "spacer" }), toggle);
}

// The right pane: the chosen request in full, a page the address names,
// or else the project's page. Drawn again only when the request changed,
// so what the human opened in it stays open.
const PAGES = ["team", "stats", "settings"];
// A discussion or a page owns the address: the project's page must not take
// it back, and a project chosen from there sends the address to it.
const owned = (path) => path === "chat" || PAGES.includes(path);
let detailed = "";
let shownPageKey = "";
function showDetail(found) {
  const page = PAGES.includes(where().path) ? where().path : "";
  detailHead(found);
  document.body.classList.toggle("picked", !!found || !!page);
  document.body.classList.toggle("paged", !!page);
  $("page").hidden = !!found || !!page || !paged;
  $("hint").hidden = !!found || !!page || !!paged;
  drawHome();
  for (const name of PAGES) $(name).hidden = name !== page;
  if (!found) {
    detailed = "";
    chatBox = null;
    if (page) {
      // A page is not a place for the compose form to sit open: it would
      // shadow the page's own project filter and take the address back.
      $("compose").close();
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
  // A queued request only ages: one redraw a minute keeps its "for" honest.
  const key = kind + (kind === "run"
    ? item.id + item.step + (item.waiting ? String(Math.floor(Date.now() / 60000)) : "")
    : JSON.stringify(item));
  if (key === detailed) return;
  detailed = key;
  const node = kind === "todo" ? card(item) : kind === "run" ? underway(item) : past(item);
  if (node.tagName === "DETAILS") node.open = true;
  const decide = decision(node, item, kind);
  // Nothing done yet on a request for later: no tabs to read.
  const main = (decide || kind === "run") && item.kind !== "later" ? tabbed(node, item) : node;
  // A committed request has nothing left to decide: tabs, but no panel.
  const panel = decide?.querySelector("button") ? decide : null;
  const facts = [...node.querySelectorAll(".facts")];
  // The right bar always exists on a request page, so it can be closed:
  // without figures yet, it says so instead of disappearing.
  const cost = facts.length ? facts : [el("div", { className: "facts" },
    el("p", { className: "muted", textContent: t("No figures yet: they show up as soon as fennec works.") }))];
  chatBox = null;
  $("pick").replaceChildren(reqHead(item, kind),
    el("div", { className: "split" }, main, el("aside", {}, panel,
      el("h2", { textContent: t("Cost and isolation") }), ...cost)));
  $("toggle-facts").hidden = false;
}

// Lucide-style strokes (24 × 24), drawn by CSS as a mask in the button's colour.
const ICONS = {
  search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
  book: '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/>',
  commit: '<circle cx="12" cy="12" r="3"/><path d="M3 12h6M15 12h6"/>',
  page: '<path d="M15 3h6v6M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  diff: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M9 10h6M12 7v6M9 17h6"/>',
  code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/>',
  fix: '<path d="M21.2 6.8a2.8 2.8 0 0 0-4-4L3.8 16.2a2 2 0 0 0-.5.8L2 21.4a.5.5 0 0 0 .6.6l4.4-1.3a2 2 0 0 0 .8-.5z"/>',
  pass: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8M10 12h4"/>',
  image: '<rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>',
  think: '<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6M10 22h4"/>',
  journal: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>',
  undo: '<path d="M3 7v6h6"/><path d="M21 17a9 9 0 0 0-15-6.7L3 13"/>',
  resume: '<path d="M6 3l14 9-14 9z"/>',
  trash: '<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="2"/>',
  replay: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
  copy: '<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  pencil: '<path d="M21.17 6.81a1 1 0 0 0-3.98-3.98L3.84 16.17a2 2 0 0 0-.5.83l-1.32 4.35a.5.5 0 0 0 .62.62l4.35-1.32a2 2 0 0 0 .83-.5z"/>',
  save: '<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><path d="M17 21v-8H7v8M7 3v5h8"/>',
  retry: '<path d="M3 12a9 9 0 0 1 15.7-6L21 8M21 3v5h-5M21 12a9 9 0 0 1-15.7 6L3 16M8 16H3v5"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  panelLeft: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>',
  panelRight: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M15 3v18"/>',
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
  chevron: '<path d="m6 9 6 6 6-6"/>',
  fleet: '<path d="M12 10.2V14M12 2v3"/><path d="M19 13V7a2 2 0 0 0-2-2H7a2 2 0 0 0-2 2v6"/><path d="M19.4 20A11.6 11.6 0 0 0 21 14l-8.2-3.6a2 2 0 0 0-1.6 0L3 14a11.6 11.6 0 0 0 2.8 7.8"/><path d="M2 21c.6.5 1.2 1 2.5 1 2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1s1.2 1 2.5 1c2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1"/>',
  up: '<path d="m5 12 7-7 7 7M12 19V5"/>',
  down: '<path d="M12 5v14M19 12l-7 7-7-7"/>',
  play: '<path d="M6 3v18l14-9z"/>',
  split: '<path d="M16 3h5v5M8 3H3v5"/><path d="M12 22v-8.3a4 4 0 0 0-1.17-2.87L3 3M15 9l6-6"/>',
  pause: '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>',
};
// A button's icon from its label, in either language.
const ICON_OF = [[/^(Commit|Push|Prepare|Pousser|Préparer)/, "commit"], [/^(Open the page|Ouvrir la page)/, "page"], [/changes|changements/, "diff"],
  [/^(Open the code|Ouvrir le code)/, "code"], [/^(Correct|Send|Corriger|Envoyer)/, "fix"], [/^(Pass|Passer)/, "pass"], [/^Journal/, "journal"],
  [/^(Undo|Annuler)$|^(Drop|Abandonner)/, "undo"], [/^(Resume|Start|Reprendre|Lancer)/, "resume"], [/^(Delete|Supprimer)/, "trash"], [/^(Save|Enregistrer)$/, "save"],
  [/^(Edit|Éditer)$/, "pencil"], [/^(Retry|Réessayer)/, "retry"], [/^(Archive|Archiver)/, "inbox"], [/^(Back to the Backlog|Remettre au Backlog)/, "clock"],
  [/^(Validate|Valider)/, "check"], [/^(Split|Découper)$/, "split"],
  [/^(Stop|Arrêter)/, "stop"], [/Fleet$/, "fleet"]];
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
let tabNow = "Summary";
function tabbed(node, item) {
  const more = [...node.querySelectorAll("details")].find((d) => d.firstChild?.textContent === t("Details"));
  more?.remove();
  const run = node.classList.contains("run");
  const live = async () => { const slot = el("div"); follow(item.id, slot); return [slot]; };
  // Each tab by its English name, shown in the chosen language.
  const panes = [["Summary", async () => [node]], ...(run ? [] : [["Changes", () => changes(item)]]),
    ["Journal", run ? live : () => journal(item)]];
  // Out of its <details>, the content keeps its look through the "more" class.
  if (more) panes.push(["Details", async () => [el("div", { className: "more" }, ...[...more.childNodes].slice(1))]]);
  const panel = el("div", { className: "tab-panel" });
  const shown = new Map();
  const bar = el("div", { className: "tabs", role: "tablist" });
  const open = async (name) => {
    tabNow = name;
    bar.querySelectorAll("button").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === name)));
    if (!shown.has(name)) {
      shown.set(name, el("div", {}, el("div", { className: "muted", textContent: t("Loading…") })));
      const box = shown.get(name);
      panes.find(([n]) => n === name)[1]().then((nodes) => box.replaceChildren(...nodes))
        .catch((e) => box.replaceChildren(el("div", { className: "bad", textContent: e.message })));
    }
    panel.replaceChildren(shown.get(name));
  };
  bar.append(...panes.map(([name]) => {
    const b = el("button", { type: "button", role: "tab", textContent: t(name) });
    b.dataset.tab = name;
    b.onclick = () => open(name);
    return b;
  }));
  open(panes.some(([n]) => n === tabNow) ? tabNow : "Summary");
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
  box.querySelectorAll("button").forEach((b) => (b.textContent === t("See the changes") || b.textContent === t("Journal")) && b.remove());
  // The correction is written where "Correct" was clicked.
  const fix = [...box.querySelectorAll("button")].find((b) => b.textContent === t("Correct"));
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
$("back").onclick = () => (location.hash = document.body.classList.contains("chats") ? "#chat" : hashOf($("project").value, ""));

function compose() {
  // A page is the primary view: the compose form does not sit over it, where
  // its project choice would take the address back from the page.
  if (PAGES.includes(where().path)) return;
  $("compose-error").textContent = "";
  $("compose").showModal();
  $("title").value = "";
  $("text").focus();
}
$("compose-open").onclick = compose;
const composed = attach($("text"));
$("text").after(composed.node);
$("compose-close").onclick = () => $("compose").close();

// Written now, worked on later: kept in the Backlog, framed ahead, validated.
$("later-add").onclick = async () => {
  $("compose-error").textContent = "";
  try {
    await api("/api/requests", { project: $("project").value, title: $("title").value, text: composed.value(), later: true });
    composed.clear();
    $("compose").close();
    refresh(true);
  } catch (e) {
    $("compose-error").textContent = e.message;
  }
};

// A question: fennec reads the project and reports, nothing is changed.
$("ask-add").onclick = async () => {
  $("compose-error").textContent = "";
  try {
    const made = await api("/api/requests", { project: $("project").value, title: $("title").value, text: composed.value(), analysis: "question" });
    composed.clear();
    $("compose").close();
    location.hash = hashOf($("project").value, made.id);
  } catch (e) {
    $("compose-error").textContent = e.message;
  }
};

$("new-request").addEventListener("submit", async (event) => {
  event.preventDefault();
  $("compose-error").textContent = "";
  try {
    const off = [...document.querySelectorAll("#chips button[aria-pressed=false]")].map((b) => b.dataset.key);
    const made = await api("/api/requests", { project: $("project").value, title: $("title").value, text: composed.value(), off });
    composed.clear();
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
  $("picker-path").textContent = data.path + " · " + (data.home ? t("your home folder: pick a folder inside it")
    : data.repo ? t("git repository")
    : data.empty ? t("empty folder") : t("not a git repository: pick a repository or an empty folder"));
  if (pickerMode === "chat") {
    // A discussion works in any folder inside the home, git or not.
    $("picker-choose").disabled = data.home;
    $("picker-choose").textContent = t("Discuss here");
  } else {
    $("picker-choose").disabled = data.home || (!data.repo && !data.empty);
    $("picker-choose").textContent = data.repo || !data.empty ? t("Add") : t("Create here");
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
  openPicker("project", t("Add a project"), pickerPath || last.replace(/\/[^/]+$/, ""));
};
$("new-chat").onclick = () => openPicker("chat", t("New discussion"), "");
$("side-chats").onclick = () => {
  location.hash === "#chat" ? refresh(true) : (location.hash = "#chat");
};
// The left bar folds to the edge: its icon reopens it from the top of the list.
$("close-side").onclick = () => setSide(true);
$("open-side").onclick = $("open-side-2").onclick = () => setSide(false);
$("picker-go").onsubmit = (e) => {
  e.preventDefault();
  browse($("picker-input").value.trim()).catch(() => ($("picker-path").textContent = t("Folder not found: {0}", $("picker-input").value)));
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
const STAGES = { frame: "framing", solve: "making", review: "review", prove: "proof" };
const order = Object.keys(STAGES);
const byStage = (a, b) => order.indexOf(a.stage) - order.indexOf(b.stage);
let team = null;

function renderTeam(view) {
  view.roles.sort(byStage);
  view.catalog.sort(byStage);
  team = view;
  const global = view.global;
  // A team named by the project but not found here: say so, and let the
  // human pick one, since fennec cannot work on the project until then.
  $("team-error").textContent = view.missing
    ? t("The team \"{0}\" is not found here: pick one so that fennec can work.", view.team)
    : "";
  const missing = view.missing ? [new Option(t("{0} (not found)", view.team), view.team)] : [];
  $("team-label").textContent = global ? t("Teams") : t("The project's team");
  $("team-name").replaceChildren(...missing, ...view.teams.map((name) =>
    new Option(name === view.default ? t("{0} (default)", name) : name, name)));
  $("team-name").value = view.team;
  $("team-roles").replaceChildren(...view.roles.map((r) => {
    const meta = el("span", { className: "muted", textContent: [STAGES[r.stage] ? t(STAGES[r.stage]) : r.stage, r.tools, r.model].filter(Boolean).join(" · ") });
    if (global) return el("li", {}, el("span", { textContent: r.name }), meta);
    const box = el("input", { type: "checkbox", checked: !r.out, ariaLabel: r.name });
    box.onchange = () => {
      const remove = team.roles.filter((x) => x.key === r.key ? !box.checked : x.out).map((x) => x.key);
      teamCall("/api/team/choose", { team: team.team, remove });
    };
    const name = el("button", { type: "button", textContent: r.name, title: t("Edit this role") });
    name.onclick = () => editRole(r);
    return el("li", { className: r.out ? "out" : "" }, box, name, meta);
  }));
  $("team-catalog").replaceChildren(new Option(t("Add a role…"), ""),
    ...view.catalog.map((r) => new Option(`${r.name} — ${STAGES[r.stage] ? t(STAGES[r.stage]) : r.stage}`, r.key)));
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
$("role-stage").replaceChildren(...Object.entries(STAGES).map(([k, label]) => new Option(t(label), k)));
async function editRole(r) {
  editing = r?.key ?? null;
  // The models the server serves; the role's own stays listed if it no longer does.
  const fallback = new Option(t("Model: the one in Settings"), "");
  $("role-model").replaceChildren(fallback);
  let served = [];
  try {
    const s = await api("/api/settings", {});
    served = (await api("/api/settings/served", { base_url: s.base_url, api_key: "" })).models;
  } catch {
    // The server does not answer: the role keeps what it has.
  }
  for (const m of new Set([...served, r?.model].filter(Boolean))) $("role-model").append(new Option(t("Model: {0}", m), m));
  $("role-model").value = r?.model ?? "";
  $("role-tools").value = r?.tools ?? "";
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
  const body = { key: editing, name: $("role-name").value, stage: $("role-stage").value, brief: $("role-brief").value, checks, model: $("role-model").value, tools: $("role-tools").value };
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

// The teams, as a page the address names: #team. Teams are global, so
// without a project the page browses them; leaving roles out, adding roles,
// and assigning a team to a project stay a project's choice.
async function showTeamPage() {
  $("team-role").hidden = $("team-save").hidden = true;
  $("team-error").textContent = "";
  const project = $("project").value;
  $("team-body").hidden = false;
  $("team-add").hidden = $("role-new").hidden = $("team-save-open").hidden = !project;
  $("team-title").textContent = project
    ? t("Team — {0}", $("project").selectedOptions[0].textContent)
    : t("Teams");
  try {
    renderTeam(await api("/api/team", project
      ? { project }
      : team?.global ? { team: team.team } : {}));
  } catch (e) {
    $("team-error").textContent = e.message;
  }
}
$("open-team").onclick = () => {
  location.hash === "#team" ? refresh(true) : (location.hash = "#team");
};
// Another team starts whole: what was left out belonged to the old one.
// Without a project the pick only browses that team's roles.
$("team-name").onchange = () => {
  if ($("project").value) teamCall("/api/team/choose", { team: $("team-name").value, remove: [] });
  else teamCall("/api/team", { team: $("team-name").value });
};
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
  $("settings-status").textContent = t("Asking the server…");
  $("settings-models").replaceChildren();
  try {
    const { models } = await api("/api/settings/served", { base_url: $("settings-url").value, api_key: $("settings-key").value });
    if (ask !== asked) return;
    $("settings-status").textContent = models.length
      ? t("Tick the models to use; none ticked: all the server's.")
      : t("The server answers but serves no model.");
    $("settings-models").replaceChildren(...models.map((m) =>
      el("li", {}, el("input", { type: "checkbox", value: m, checked: chosen.includes(m), ariaLabel: m }), m)));
  } catch (e) {
    if (ask === asked) $("settings-status").textContent = unreachable(e.message) ?? e.message;
  }
}
// The theme picker: each choice applies at once; the form's submit is for
// the model's settings only.
function buildThemePicker() {
  const themes = $("theme-picker");
  themes.replaceChildren(...THEMES.map((theme) => {
    const b = el("button", { type: "button", className: "theme" });
    b.setAttribute("aria-pressed", String(theme.id === loadTheme()));
    b.append(el("span", { className: "preview" },
      el("span", { className: "bg", style: `background: ${theme.palette["--bg"]}` }),
      el("span", { className: "accent", style: `background: ${theme.palette["--accent"]}` })),
      el("span", { className: "name", textContent: theme.name }));
    b.onclick = () => {
      saveTheme(theme.id);
      applyTheme();
      themes.querySelectorAll(".theme").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
    };
    return b;
  }));
}

// The font-scale slider: applies on every move, keeps the value for next time.
function buildFontScale() {
  const input = $("font-scale");
  input.min = FONT_MIN_PX; input.max = FONT_MAX_PX; input.step = FONT_STEP_PX;
  input.value = applyFontScale();
  const set = (v) => {
    document.documentElement.style.zoom = v / FONT_BASE_PX;
    saveFontScale(v);
  };
  input.oninput = () => {
    const v = Number(input.value);
    if (v >= FONT_MIN_PX && v <= FONT_MAX_PX) set(v);
  };
  input.onblur = () => {
    const v = Number(input.value);
    input.value = Number.isFinite(v) ? Math.min(FONT_MAX_PX, Math.max(FONT_MIN_PX, v)) : FONT_BASE_PX;
    set(Number(input.value));
  };
}

// The settings panels, one tab each: general, appearance, rules, fleet, skills, plugins, cleaning.
document.querySelectorAll("#settings-tabs [role=tab]").forEach((b) => {
  b.onclick = () => {
    document.querySelectorAll("#settings-tabs [role=tab]").forEach((tab) => tab.setAttribute("aria-selected", String(tab === b)));
    ["settings-general", "settings-appearance", "settings-rules", "settings-fleet", "settings-skills", "settings-plugins", "settings-clean"].forEach((id) => $(id).hidden = id !== b.dataset.panel);
    if (b.dataset.panel === "settings-clean") showLeftovers();
    if (b.dataset.panel === "settings-fleet") showFleetSettings();
    if (b.dataset.panel === "settings-plugins") showPlugins({});
    if (b.dataset.panel === "settings-rules") changeRules($("settings-rules"), "", "user");
    if (b.dataset.panel === "settings-rules") changeGuard();
  };
});
// What interrupted work left behind, for the human to pick and throw away.
const bytes = (n) => n >= 1e9 ? t("{0} GB", (n / 1e9).toFixed(1)) : n >= 1e6 ? t("{0} MB", Math.round(n / 1e6)) : t("{0} kB", Math.ceil(n / 1e3));
async function showLeftovers() {
  $("leftovers-error").textContent = "";
  $("leftovers-clear").disabled = true;
  let left;
  try {
    left = await api("/api/leftovers");
  } catch (e) {
    $("leftovers-error").textContent = e.message;
    return;
  }
  $("leftovers-clear").hidden = !left.length;
  if (!left.length) {
    $("leftovers").replaceChildren(el("li", { className: "muted", textContent: t("Nothing to clean.") }));
    return;
  }
  const sync = () => {
    const n = $("leftovers").querySelectorAll("input:checked").length;
    $("leftovers-clear").disabled = !n;
    $("leftovers-clear").textContent = n ? t("Delete the selection ({0})", n) : t("Delete the selection");
    setIcon($("leftovers-clear"), "trash");
  };
  $("leftovers").replaceChildren(...left.map((f) => {
    const box = el("input", { type: "checkbox", value: f.id });
    box.onchange = sync;
    const when = f.since ? new Date(f.since * 1000).toLocaleString(lang(), { dateStyle: "short", timeStyle: "short" }) : "";
    const where = f.in_folder ? t("in your folder") : t("apart, {0}", bytes(f.bytes));
    return el("li", {}, el("label", {}, box,
      el("span", {}, el("strong", { textContent: f.project.split("/").pop() }), el("span", { className: "muted", textContent: ` · ${when} · ${where}` })),
      el("span", { className: "what" }, ...(f.commits.length
        ? [t("{0} commits no branch took up: ", f.commits.length),
          ...f.commits.flatMap((c, i) => [i ? ", " : "", el("code", { textContent: c })])]
        : [t("No commit to lose.")])),
      f.checked_out ? el("span", { className: "what held",
        textContent: t("Your folder is on it: it goes back to its branch, and what is not committed goes to the git stash.") }) : null));
  }));
  sync();
}
$("leftovers-clear").onclick = async () => {
  const button = $("leftovers-clear");
  // Two clicks: what goes cannot come back.
  if (!button.classList.contains("sure")) {
    button.classList.add("sure");
    button.textContent = t("Confirm the deletion");
    setIcon(button, "trash");
    return;
  }
  button.classList.remove("sure");
  button.disabled = true;
  const ids = [...$("leftovers").querySelectorAll("input:checked")].map((b) => b.value);
  try {
    const r = await api("/api/leftovers/clear", { ids });
    await showLeftovers();
    $("leftovers-error").textContent = r.errors.join("\n");
  } catch (e) {
    $("leftovers-error").textContent = e.message;
    button.disabled = false;
  }
};

// The language picker: the page is drawn again in the chosen one.
function buildLanguage() {
  const select = $("language");
  select.replaceChildren(...LANGS.map(([id, name]) => new Option(name, id)));
  select.value = lang();
  select.onchange = () => {
    try { localStorage.setItem(LANG_KEY, select.value); } catch { /* stays as it was */ }
    location.reload();
  };
}

async function showSettingsPage() {
  buildThemePicker();
  buildFontScale();
  buildLanguage();
  try {
    const s = await api("/api/settings", {});
    chosen = s.models;
    $("settings-url").value = s.base_url;
    $("settings-tokens").value = s.max_tokens;
    $("settings-effort").value = s.effort;
    $("settings-think").value = s.think;
    $("settings-key").value = "";
    $("settings-key").placeholder = s.has_key ? t("saved (leave empty to keep it)") : "";
    $("settings-env").hidden = !s.env.length;
    $("settings-env").textContent = t("At launch, {0} goes before these settings.", s.env.join(", "));
    $("settings-error").textContent = "";
    $("settings-saved").textContent = "";
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
      max_tokens: Number($("settings-tokens").value), effort: $("settings-effort").value,
      think: $("settings-think").value });
    $("settings-key").value = "";
    $("settings-error").textContent = "";
    $("settings-saved").textContent = t("✓ Saved");
  } catch (err) {
    $("settings-saved").textContent = "";
    $("settings-error").textContent = err.message;
  }
};
// Saved until something changes again.
$("settings-form").oninput = () => ($("settings-saved").textContent = "");

// Stats: what every task cost, per day, per task, per model and per project,
// and where fennec works in each project, which can be changed here.
const PLACES = { here: "In the folder", apart: "Apart, next to it" };
const cell = (text, className = "") => el("td", { textContent: text, className });

async function showStats() {
  const range = $("stats-range"), project = $("stats-project");
  if (!project.options.length) {
    // The project list, once: a filtered answer lists only its project.
    const all = await api("/api/stats");
    project.append(new Option(t("All projects"), ""),
      ...all.projects.map((p) => new Option(p.name, p.path)));
  }
  const v = await api(`/api/stats?days=${range.value}&project=${encodeURIComponent(project.value)}`);
  const a = v.all;
  const tile = (value, label) => el("div", { className: "tile" }, el("b", { textContent: value }), el("span", { textContent: label }));
  $("stats-tiles").replaceChildren(
    tile(number(a.requests), t("tasks")),
    tile(duration(a.ms ?? 0), t("of model")),
    tile(compact(a.sent), t("tokens sent · {0} % cached", percent(a.cached, a.sent))),
    tile(compact(a.received), t("tokens generated · {0}", speed(a) || "—")));
  $("stats-days").replaceChildren(...v.by_day.map((d) => el("tr", {},
    el("th", { textContent: d.day }), cell(compact(d.sent)), cell(compact(d.received)))));
  const fastest = Math.max(1, ...v.models.map(tps));
  $("stats-models").replaceChildren(...v.models.map((m) => el("tr", {},
    el("th", { textContent: m.name }), cell(number(m.turns)), cell(compact(m.sent)),
    el("td", {}, el("span", { className: "bar", style: `--p:${percent(m.cached, m.sent)}%` }), `${percent(m.cached, m.sent)} %`),
    cell(compact(m.received)),
    el("td", {}, el("span", { className: "bar", style: `--p:${percent(tps(m), fastest)}%` }), m.ms ? `${tps(m)}/s` : "—"),
    cell(duration(m.ms ?? 0)))));
  $("stats-stages").replaceChildren(...v.stages.sort((x, y) => stageRank(x.name) - stageRank(y.name)).map((s) => el("tr", {},
    el("th", { textContent: stageName(s.name) }), cell(number(s.turns)), cell(duration(s.ms ?? 0)),
    cell(`${percent(s.ms, a.ms)} %`), cell(compact(s.sent)), cell(compact(s.received)), cell(s.ms ? `${tps(s)}/s` : "—"))));
  $("stats-projects").replaceChildren(...v.projects.map((p) => {
    const place = el("select", { ariaLabel: t("Where fennec works in {0}", p.name) },
      ...Object.entries(PLACES).map(([k, label]) => new Option(t(label), k)));
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
  $("stats-tasks").replaceChildren(...v.tasks.map((x) => el("tr", {},
    el("th", { textContent: x.title, title: x.title }), cell(x.project), cell(x.models),
    cell(compact(x.sent)), cell(compact(x.received)),
    el("td", {}, el("span", { className: "bar", style: `--p:${percent(x.cached, x.sent)}%` }), `${percent(x.cached, x.sent)} %`),
    cell(duration(x.ms ?? 0)))));
}
$("stats-range").onchange = () => showStats().catch((e) => ($("stats-error").textContent = e.message));
$("stats-project").onchange = () => showStats().catch((e) => ($("stats-error").textContent = e.message));
// One project's page, above its requests: read when it is chosen, not at each refresh.
let paged = "";
async function showPage(path) {
  paged = path;
  if (!path) return;
  $("page-error").textContent = "";
  const [stats, rules] = await Promise.all([api("/api/stats"), api("/api/rules", { project: path })]);
  if (paged !== path) return;
  const p = stats.projects.find((x) => x.path === path) ?? {};
  const tile = (value, label, tone = "") => el("div", { className: `tile ${tone}` }, el("b", { textContent: value }), el("span", { textContent: label }));
  const proven = (p.first_try ?? 0) + (p.retried ?? 0);
  $("page-tiles").replaceChildren(
    tile(number(p.landed), t("commits")),
    tile(proven ? `${percent(p.first_try, proven)} %` : "—", t("proven at the first try")),
    tile(number(p.failed), t("failures"), p.failed ? "bad" : ""),
    tile(number(p.failed_attempts), t("failed tries undone")),
    tile(duration(p.ms ?? 0), t("of model")), tile(compact(p.sent), t("tokens sent")),
    tile(bytes(p.disk ?? 0), t("disk of its worlds")));
  const place = $("page-place");
  place.replaceChildren(...Object.entries(PLACES).map(([k, label]) => new Option(t(label), k)));
  if (p.place && !PLACES[p.place]) place.append(new Option(p.place, p.place));
  place.value = p.place ?? "here";
  place.onchange = () => api("/api/projects", { path, place: place.value })
    .catch((e) => { $("page-error").textContent = e.message; place.value = p.place; });
  showRules($("page-rules"), path, "project", rules);
  showStops(path, await api("/api/stops", { project: path }));
  showKnowledge(path, await api("/api/knowledge", { project: path }));
}

// What the project knows of itself (vision, zones, decisions) and the
// analyses that propose it: adopt a project without a vision, audit one with.
function showKnowledge(path, k) {
  $("page-knowledge").replaceChildren(k.vision
    ? t("Vision written · {0} sensitive zones · {1} decisions", k.zones ?? 0, k.decisions)
    : t("No architecture vision yet: fennec can read the project and propose one, with its sensitive zones and first decisions."),
  ...(k.zones_error ? [el("div", { className: "bad", textContent: k.zones_error })] : []));
  const start = (kind, label, title, text) => {
    const b = el("button", { type: "button", textContent: label, title, className: kind === "adopt" ? "primary" : "" });
    b.onclick = async () => {
      $("page-error").textContent = "";
      try {
        const made = await api("/api/requests", { project: path, analysis: kind, title: label, text });
        location.hash = hashOf(path, made.id);
      } catch (e) {
        $("page-error").textContent = e.message;
      }
    };
    return b;
  };
  $("page-knowledge-actions").replaceChildren(k.vision
    ? start("audit", t("Audit the architecture"), t("fennec reads the code, sets it against the vision and the decisions, and suggests improvements"),
      t("Audit the architecture of this project and suggest improvements."))
    : start("adopt", t("Adopt this project"), t("fennec reads the project and proposes its vision, sensitive zones and first decisions; you correct and accept them"),
      t("Adopt this project: propose its architecture vision, its sensitive zones and its first decisions.")));
}

// The dev's loop on the project: where its tasks stop and wait for them.
// A sensitive task stops after its design whatever this says; the commit
// always waits.
function showStops(path, stops) {
  const row = (stop, label, help) => {
    const b = el("button", { type: "button", className: "switch" });
    b.setAttribute("role", "switch");
    b.setAttribute("aria-checked", String(stops[stop]));
    b.setAttribute("aria-label", label);
    b.onclick = async () => {
      try {
        showStops(path, await api("/api/stops", { project: path, stop, on: !stops[stop] }));
      } catch (e) {
        $("page-error").textContent = e.message;
      }
    };
    return el("li", {}, el("strong", { textContent: label }), el("span", { className: "what", textContent: help }), b);
  };
  $("page-stops").replaceChildren(
    row("design", t("After the design"), t("Wait for your validation of the design and plan. A sensitive task always waits.")),
    row("code", t("After the code"), t("Wait before the AI review and the proof, so you look at the work first.")),
    el("li", { className: "muted", textContent: t("Before the commit: always.") }));
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
      const b = el("button", { type: "button", className: "switch" });
      b.setAttribute("role", "switch");
      b.setAttribute("aria-checked", String(s.chosen));
      b.setAttribute("aria-label", t(s.chosen ? "Disable the skill {0}" : "Enable the skill {0}", s.name));
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
      ? t("No skill matches the search.") : t("No skill on this machine.") })]));
}
$("skill-search").oninput = drawSkillList;

// The person's plugins: each says when it speaks; one is tried with a test
// message before trusting it with real questions.
const PLUGIN_EVENTS = { started: "starts", question: "has a question", ready: "is ready", failed: "fails",
  landed: "is committed", before_land: "before each commit, which it may block" };
async function showPlugins(change) {
  $("plugins-error").textContent = $("plugins-saved").textContent = "";
  let got;
  try {
    got = await api("/api/plugins", change);
  } catch (e) {
    $("plugins-error").textContent = e.message;
    return;
  }
  if (change.test && got.tried) $("plugins-error").textContent = `${change.name} : ${got.tried}`;
  else if (change.test) $("plugins-saved").textContent = t("Test message sent by {0}.", change.name);
  $("plugins-list").replaceChildren(...got.plugins.map((p) => {
    if (p.error) return el("li", {}, el("strong", { textContent: p.name }), el("span", { className: "what error", textContent: p.error }));
    const when = p.events.map((e) => (PLUGIN_EVENTS[e] ? t(PLUGIN_EVENTS[e]) : e)).join(", ");
    const test = el("button", { type: "button", className: "quiet", ariaLabel: t("Try {0}", p.name), title: t("Send a test message") });
    setIcon(test, "send");
    test.onclick = async () => {
      test.disabled = true;
      await showPlugins({ name: p.name, test: true });
    };
    const on = el("button", { type: "button", className: "switch" });
    on.setAttribute("role", "switch");
    on.setAttribute("aria-checked", String(p.on));
    on.setAttribute("aria-label", t(p.on ? "Disable the plugin {0}" : "Enable the plugin {0}", p.name));
    on.onclick = () => showPlugins({ name: p.name, on: !p.on });
    // A plugin that relays decisions from a chat says for whom, and whether it listens.
    const relay = !p.relays ? "" : " · " + t("relays your decisions from the chat, for {0}", p.allow.length ? p.allow.join(", ") : t("nobody (\"allow\" is empty)"))
      + (!p.on ? "" : p.listening ? " · " + t("listening") : " · " + t("stopped: disable it, then enable it again"));
    return el("li", { title: p.from }, el("strong", { textContent: p.name }),
      el("span", { className: "what", textContent: `${p.description ? p.description + " · " : ""}${t("when a task {0}", when)}${relay}` }), test, on);
  }), ...(got.plugins.length ? [] : [el("li", { className: "muted", textContent: t("No plugin in ~/.config/fennec/plugins.") })]));
}

// The rules said in words, a project's on its page and the human's own in
// the settings: one list and one form each, the scope fixed by where it is.
function showRules(box, path, scope, rules) {
  rules = rules.filter((r) => r.scope === scope);
  const count = box.querySelector(".count");
  if (count) {
    count.textContent = rules.length;
    count.hidden = !rules.length;
  }
  box.querySelector("ul").replaceChildren(...rules.length ? [] : [el("li", { className: "empty muted", textContent: t("No rule yet.") })], ...rules.map((r) => {
    const remove = el("button", { type: "button", className: "quiet", ariaLabel: t("Remove the rule"), title: t("Remove the rule") });
    setIcon(remove, "close");
    remove.onclick = () => changeRules(box, path, scope, { change: "remove", text: r.text });
    return el("li", {}, el("span", {}, ...rich(r.text)), remove);
  }));
}

async function changeRules(box, path, scope, change = {}) {
  const error = box.querySelector(".error") ?? $("page-error");
  try {
    showRules(box, path, scope, await api("/api/rules", { project: path, scope, ...change }));
    error.textContent = "";
    return true;
  } catch (e) {
    error.textContent = explain(e.message);
    return false;
  }
}
for (const [id, scope] of [["page-rules", "project"], ["settings-rules", "user"]]) {
  const box = $(id), input = box.querySelector("input");
  box.querySelector("form").onsubmit = async (e) => {
    e.preventDefault();
    if (await changeRules(box, scope === "user" ? "" : paged, scope, { change: "add", text: input.value })) input.value = "";
  };
}
// The rules on what agents run, read and write, as the person's file holds
// them: removing one names its place and its pattern, so an older list
// removes nothing by mistake.
const GUARD_EFFECT = { deny: "Denied", ask: "Asks", allow: "Allowed" };
const GUARD_ON = { command: "command", read: "read", write: "write", file: "read, write" };
async function changeGuard(change = {}) {
  try {
    const { rules } = await api("/api/guard", change);
    $("guard-error").textContent = "";
    $("guard-count").textContent = rules.length;
    $("guard-list").replaceChildren(...rules.length ? [] : [el("li", { className: "empty muted", textContent: t("No rule: isolation alone protects.") })],
      ...rules.map((r, i) => {
        const remove = iconButton("close", t("Remove the rule"), () => changeGuard({ remove: i, pattern: r.pattern }));
        return el("li", {},
          el("span", { className: `effect ${r.effect}`, textContent: GUARD_EFFECT[r.effect] ? t(GUARD_EFFECT[r.effect]) : r.effect }),
          el("span", { className: "muted", textContent: GUARD_ON[r.on] ? t(GUARD_ON[r.on]) : r.on }),
          el("span", {}, el("code", { textContent: r.pattern }),
            r.why ? el("span", { className: "why", textContent: " — " + t(r.why) }) : null,
            r.project ? el("span", { className: "why", textContent: " · " + r.project }) : null),
          remove);
      }));
    return true;
  } catch (e) {
    $("guard-error").textContent = explain(e.message);
    return false;
  }
}
$("guard-form").onsubmit = async (e) => {
  e.preventDefault();
  const add = { effect: $("guard-effect").value, on: $("guard-on").value, pattern: $("guard-pattern").value, why: $("guard-why").value };
  if (await changeGuard({ add })) $("guard-pattern").value = $("guard-why").value = "";
};
$("to-user-rules").onclick = () => {
  location.hash = "#settings";
  document.querySelector('#settings-tabs [data-panel="settings-rules"]').click();
};
$("page-team").onclick = () => $("open-team").click();
$("page-open").onclick = async () => {
  try {
    const c = await api("/api/open-project", { project: paged });
    $("page-error").textContent = c.opened ? "" : t("VS Code not found: open {0} in your editor.", c.folder);
  } catch (e) {
    $("page-error").textContent = e.message;
  }
};

// The Fleet: one queue of tasks across projects, started on their own.
$("fleet-at-once").replaceChildren(...Array.from({ length: 10 }, (_, i) => new Option(String(i + 1), String(i + 1))));
async function fleetDo(path, body) {
  $("fleet-error").textContent = "";
  try {
    await api(path, body);
    await showFleetSettings();
    refresh(true);
  } catch (e) {
    $("fleet-error").textContent = e.message;
  }
}
$("fleet-at-once").onchange = () => fleetDo("/api/fleet", { at_once: Number($("fleet-at-once").value) });
$("fleet-on-fail").onchange = () => fleetDo("/api/fleet", { on_fail: $("fleet-on-fail").value });
$("fleet-on-success").onchange = () => fleetDo("/api/fleet", { on_success: $("fleet-on-success").value });
// The Fleet's settings, in the settings tab: the values the queue runs with.
async function showFleetSettings() {
  try {
    const v = await api("/api/fleet");
    for (const [id, value] of [["fleet-at-once", String(v.at_once)], ["fleet-on-fail", v.on_fail], ["fleet-on-success", v.on_success]]) {
      if (document.activeElement !== $(id)) $(id).value = value;
    }
  } catch (e) {
    $("fleet-error").textContent = e.message;
  }
}

// The task list, one step at a time: the tab the human picked shows its box.
let listTab = "todo";
function listTabs(name) {
  listTab = name;
  document.querySelectorAll("#list-tabs [role=tab]").forEach((b) => b.setAttribute("aria-selected", String(b.dataset.tab === name)));
  document.querySelectorAll(".box[data-tab]").forEach((box) => { box.hidden = box.dataset.tab !== name; });
}
document.querySelectorAll("#list-tabs [role=tab]").forEach((b) => { b.onclick = () => listTabs(b.dataset.tab); });
listTabs("todo");

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
    $("chips").replaceChildren(el("span", { className: "muted", textContent: t("Team for this task:") }),
      ...roles.map((r) => {
        const chip = el("button", { type: "button", className: `rtag ${r.stage} chip`, textContent: r.name,
          title: t("Click to do without this role on this task") });
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
  // Choosing a project while a discussion or a page is open takes its address
  // back; the address doing it keeps the page honest.
  if (owned(where().path)) {
    location.hash = hashOf($("project").value, "");
    return;
  }
  picked = "";
  showChips();
  focusOn($("project").value);
  refresh(true);
});

// Which build runs, so the human knows they test the latest: a newer commit
// on its branch, or changes left out of it, are said.
async function showVersion() {
  try {
    const v = await api("/api/version");
    const built = new Date(v.built * 1000).toLocaleString(lang(), { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
    $("version").title = v.branch ? t("Built on {0} from branch {1}", built, v.branch) : t("Built on {0}", built);
    $("version").replaceChildren(el("div", { textContent: `v${v.version} · ${v.commit}` }),
      el("div", {}, built,
        v.dirty ? el("span", { className: "dirty", textContent: " · " + t("+changes"), title: t("Built with changes not committed yet") }) : ""),
      v.latest && v.latest !== v.commit
        ? el("div", { className: "stale", textContent: t("{0} is at {1}: build again", v.branch, v.latest),
          title: t("This build is no longer the latest: just build, then just service-restart") })
        : "");
  } catch {
    // An older binary has no version: nothing to say.
  }
}
showVersion();
setInterval(showVersion, 60000);
