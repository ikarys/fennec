// node ui/plan.test.js — a plan splits into its numbered sections (heading
// marks dropped, the first line being the task's title), each up to
// the next heading as high as its own; the title, "Hors périmètre" and the
// questions after them stay out. A plan without numbered steps has none.
const assert = require("assert");
const src = require("fs").readFileSync(__dirname + "/app.js", "utf8");
const grab = (name) => src.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n}\\n`))[0];
const planSteps = eval(`(${grab("planSteps")})`);

const plan = `## Plan : durcir
intro
### 1. Async
- worker
#### 1.1 détail
- timeout
### 2) Stockage
- JSON
### Hors périmètre
- React
### 3. Pas une étape
### Questions`;
const steps = planSteps(plan);
assert.deepStrictEqual(steps, [
  "1. Async\n- worker\n#### 1.1 détail\n- timeout",
  "2) Stockage\n- JSON",
]);
assert.deepStrictEqual(planSteps("Juste du texte\n- a\n- b"), []);
console.log("ok plan");
