// Annotated screenshots for the fennec end-user guide.
// Run from web/: node scripts/doc-shots.mjs (just shots)
import { chromium } from 'playwright';
import { execFileSync, spawn } from 'node:child_process';
import { access, cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const FENNEC_BIN = process.env.FENNEC_BIN ?? join(repo, 'target/release/fennec');
const MODEL_URL = process.env.DOC_SHOTS_MODEL_URL ?? 'http://127.0.0.1:8000/v1';
const OUT = process.env.DOC_SHOTS_OUT ?? join(repo, 'docs/users/src/images');
const PORT = 7799;
const BASE = `http://127.0.0.1:${PORT}`;

async function waitForServer(url) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try { if ((await fetch(url)).status === 200) return; } catch {}
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`fennec did not answer at ${url} within 15s`);
}

const clearMarks = (page) => page.evaluate(() => document.querySelectorAll('.doc-mark').forEach((el) => el.remove()));

async function mark(page, items) {
  await clearMarks(page);
  for (const [locator, number, pad = 4, below = false] of items) {
    // A pair of locators frames the area from the first to the last one.
    const [a, b] = Array.isArray(locator) ? locator : [locator, locator];
    const [boxA, boxB] = [await a.first().boundingBox(), await b.last().boundingBox()];
    if (!boxA || !boxB) continue;
    const box = { x: boxA.x, y: boxA.y, width: boxB.x + boxB.width - boxA.x, height: boxB.y + boxB.height - boxA.y };
    await page.evaluate(({ x, y, w, h, pad, number, below }) => {
      const boxEl = document.createElement('div');
      boxEl.className = 'doc-mark';
      Object.assign(boxEl.style, { position: 'fixed', left: `${x - pad}px`, top: `${y - pad}px`, width: `${w + pad * 2}px`, height: `${h + pad * 2}px`, border: '3px solid #ff2d78', borderRadius: '8px', boxShadow: '0 0 0 4px rgba(255,45,120,.25)', zIndex: '99999', pointerEvents: 'none' });
      const badge = document.createElement('div');
      badge.className = 'doc-mark';
      badge.textContent = String(number);
      const left = below ? x + w / 2 - 14 : Math.min(Math.max(x - pad - 14, 2), window.innerWidth - 30);
      const top = below ? y + h + pad + 6 : Math.min(Math.max(y - pad - 14, 2), window.innerHeight - 30);
      Object.assign(badge.style, { position: 'fixed', left: `${left}px`, top: `${top}px`, width: '28px', height: '28px', background: '#ff2d78', color: 'white', font: '700 16px/28px system-ui, sans-serif', textAlign: 'center', borderRadius: '50%', boxShadow: '0 2px 6px rgba(0,0,0,.5)', zIndex: '100000', pointerEvents: 'none' });
      // A native <dialog> sits in the top layer: marks go inside it to show above it.
      (document.querySelector('dialog[open]') ?? document.body).append(boxEl, badge);
    }, { x: box.x, y: box.y, w: box.width, h: box.height, pad, number, below });
  }
}

async function shot(page, name, items = []) {
  await mark(page, items);
  const path = join(OUT, `${name}.png`);
  await page.screenshot({ path });
  await clearMarks(page);
  console.log(path);
}

async function connectModel(page) {
  await page.goto(`${BASE}/#/`);
  const fix = page.getByText('Fix in Settings').first();
  await fix.waitFor();
  await shot(page, 'connect-model-1', [[fix, 1]]);
  await fix.click();
  const baseUrlInput = page.locator('input').nth(1);
  await baseUrlInput.waitFor();
  const apiKeySelect = page.locator('select').nth(1);
  await shot(page, 'connect-model-2', [[baseUrlInput, 1], [apiKeySelect, 2]]);
  await apiKeySelect.selectOption({ label: 'Stored key (encrypted)' });
  const passwordInput = page.locator('input[type=password]');
  await passwordInput.waitFor();
  await shot(page, 'connect-model-3', [[passwordInput, 1]]);
  await apiKeySelect.selectOption({ label: 'None' });
  await baseUrlInput.fill(MODEL_URL);
  await page.keyboard.press('Tab');
  const star = page.getByTitle('Use by default in new sessions').first();
  await star.waitFor({ timeout: 15_000 });
  const heart = page.getByTitle('Show in the composer').first();
  const fetchButton = page.getByTitle('Fetch the model list (/models)');
  await shot(page, 'connect-model-4', [[star, 1], [heart, 2], [fetchButton, 3]]);
  await star.click();
  await page.waitForTimeout(500);
  await page.goto(`${BASE}/#/`);
  await page.waitForTimeout(1000);
  await shot(page, 'connect-model-5', [[page.getByText('A model answers').locator('..'), 1]]);
}

// A small repository for the guide to open, under the blank HOME. The HOME gets a git identity:
// fennec commits the agent's work under the human's, and without one there is nothing to integrate.
async function demoRepo(home) {
  await writeFile(join(home, '.gitconfig'), '[user]\n\tname = Demo\n\temail = demo@example.com\n');
  const repo = join(home, 'projects', 'todo-api');
  await mkdir(join(repo, 'src'), { recursive: true });
  await writeFile(join(repo, '.gitignore'), '__pycache__/\n');
  await writeFile(join(repo, 'README.md'), '# todo-api\n\nA small to-do list API.\n');
  await writeFile(join(repo, 'src', 'main.py'), 'def list_todos():\n    return []\n');
  const git = (...args) => execFileSync('git', args, { cwd: repo, env: { ...process.env, HOME: home } });
  git('init', '-q', '-b', 'main');
  git('add', '.');
  git('commit', '-q', '-m', 'Initial commit');
  return repo;
}

async function openProject(page) {
  await page.goto(`${BASE}/#/`);
  const open = page.getByRole('button', { name: 'Open a project', exact: true }).first();
  await open.waitFor();
  await shot(page, 'open-project-1', [[open, 1]]);
  await open.click();
  const path = page.locator('#project-dialog-path');
  await path.waitFor();
  const projects = page.getByRole('dialog').getByRole('button', { name: 'projects', exact: true });
  await projects.waitFor();
  await projects.dblclick();
  const folder = page.getByRole('dialog').getByRole('button', { name: /^todo-api/ });
 
  await folder.waitFor();
  await shot(page, 'open-project-2', [[path, 1], [folder, 2]]);
  await folder.click();
  const target = page.getByRole('dialog').getByText('Folder:', { exact: true }).locator('..');
  const confirm = page.getByRole('button', { name: 'Open this folder' });
  await shot(page, 'open-project-3', [[target, 1], [confirm, 2]]);
  await shot(page, 'open-project-new', [[page.locator('#project-dialog-new-name').locator('..'), 1]]);
  await confirm.click();
  const newSession = page.locator('button', { hasText: 'New session' });
  await newSession.waitFor();
  await shot(page, 'open-project-4', [[page.getByRole('heading', { name: 'todo-api' }).getByText('todo-api'), 1], [newSession, 2]]);
}

async function remoteAccess(page) {
  await page.goto(`${BASE}/#/settings/sandbox`);
  const nav = page.getByRole('link', { name: /Sandbox & network/ });
  const password = page.getByLabel('Remote password');
  await password.waitFor();
  await password.scrollIntoViewIfNeeded();
  await password.fill('correct horse battery');
  const savePassword = page.getByRole('button', { name: 'Save password' });
  await shot(page, 'remote-access-1', [[nav, 1], [password, 2], [savePassword, 3]]);
  await savePassword.click();
  const remote = page.getByRole('switch', { name: 'Allow remote connections' });
  await remote.click();
  await page.waitForTimeout(300); // the switch animates
  const save = page.getByRole('button', { name: 'Save', exact: true });
  await shot(page, 'remote-access-2', [[page.getByText('password set', { exact: true }), 1], [remote, 2], [save, 3]]);
  await save.click();
  const restartNeeded = page.getByText('restart to apply', { exact: true });
  await restartNeeded.waitFor();
  await shot(page, 'remote-access-3', [[restartNeeded, 1], [page.getByRole('button', { name: 'Restart the server' }), 2]]);
}

// The Changes page of the session, read in a second tab: once integrated, the worktree is gone and the page is empty.
async function changesPage(page) {
  const id = page.url().match(/#\/s\/([^/?]+)/)[1];
  const side = await page.context().newPage();
  await side.goto(`${BASE}/#/s/${id}/changes`);
  const files = side.locator('div.w-96 button');
  await files.first().waitFor();
  await files.filter({ hasText: 'src/main.py' }).click();
  const diff = side.locator('table');
  await diff.waitFor();
  await side.mouse.move(700, 880);
  await shot(side, 'review-1', [[side.locator('div.w-96'), 1], [side.locator('.min-w-0.grow.flex-col.overflow-hidden'), 2], [side.getByText(/^verified$/i).first(), 3]]);
  await side.close();
}

// A real run against the model: each human gate of Plan, build, verify, then the integration.
async function firstSession(page) {
  await page.locator('button', { hasText: 'New session' }).click();
  const composer = page.getByPlaceholder('Describe the workflow task…');
  await composer.waitFor();
  await composer.fill('Add an add_todo(title) function to src/main.py that stores the to-do in an in-memory list, and make list_todos return that list.');
  const workflow = page.getByText('Plan, build, verify', { exact: true }).last();
  const model = page.locator('button[title*=" · "]').last(); // title: provider · model
  await shot(page, 'first-session-1', [[composer, 1], [workflow, 2], [model, 3]]);
  await composer.press('Enter');

  const go = page.getByRole('button', { name: 'Go', exact: true });
  await go.waitFor({ timeout: 900_000 });
  await page.waitForTimeout(1000);
  const runRail = page.getByText('Plan, build, verify', { exact: true }).first();
  await shot(page, 'first-session-2', [[page.getByText('C1', { exact: true }).first(), 1], [page.getByPlaceholder('Comment (optional)…'), 2], [page.getByRole('button', { name: 'Revise', exact: true }), 3], [go, 4]]);
  await go.click();

  const validate = page.getByRole('button', { name: 'Validate', exact: true });
  await validate.waitFor({ timeout: 900_000 });
  await page.waitForTimeout(1000);
  await shot(page, 'first-session-3', [[page.getByText('Validate the tests', { exact: true }).first(), 1], [page.getByRole('button', { name: 'Rework', exact: true }), 2], [validate, 3]]);
  await validate.click();

  const review = page.getByRole('button', { name: 'Review and integrate…', exact: true }).last(); // the decision bar's
  // The builder may ask to touch a protected test; the guide keeps the tests as validated.
  const refuse = page.getByRole('button', { name: 'Refuse', exact: true }).first();
  for (const deadline = Date.now() + 2_400_000; !(await review.isVisible()); await page.waitForTimeout(1000)) {
    if (Date.now() > deadline) throw new Error('No integration offered after the run');
    if (await refuse.isVisible()) await refuse.click();
  }
  await page.waitForTimeout(1000);
  await shot(page, 'first-session-4', [[runRail, 1], [review, 2]]);
  await review.click();
  const dialog = page.getByRole('dialog');
  const integrate = dialog.getByRole('button', { name: 'Integrate', exact: true });
  const message = dialog.getByRole('textbox').first();
  await message.waitFor();
  await page.waitForTimeout(2000); // the preview and its blockers load
  // A real run may touch files no plan step names: the human accepts them, here on the guide's behalf.
  const accepts = await dialog.getByLabel('I accept it').all();
  for (const box of accepts) await box.check();
  await integrate.click({ trial: true, timeout: 300_000 }); // enabled once the server's checks are in
  await shot(page, 'first-session-5', [[message, 1], ...accepts.map((box) => [box, 2]), [integrate, 3]]);
  await changesPage(page);
  await integrate.click();
  const result = page.getByText(/^Integrated:/);
  await result.waitFor();
  await shot(page, 'first-session-6', [[result, 1]]);
}

const centre = async (locator) => {
  const box = await locator.boundingBox();
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
};

async function drag(page, from, to) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
}

// A point on an edge's path, to click it: a loop's bounding box is mostly empty.
const edgePoint = (page, id) => page.evaluate((id) => {
  const path = document.querySelector(`.react-flow__edge[data-id="${id}"] path`);
  const point = path.getPointAtLength(path.getTotalLength() / 2);
  const m = path.getScreenCTM();
  return { x: point.x * m.a + m.e, y: point.y * m.d + m.f };
}, id);

const flowNode = (page, id) => page.locator(`.react-flow__node[data-id="${id}"]`);

async function connect(page, from, port, to) {
  const source = flowNode(page, from).locator(port ? `.react-flow__handle.source[data-handleid="${port}"]` : '.react-flow__handle.source');
  await drag(page, await centre(source), await centre(flowNode(page, to).locator('.react-flow__handle.target')));
}

async function zoom(page, at, deltaY) {
  await page.mouse.move(at.x, at.y);
  for (let i = 0; i < 3; i++) { await page.mouse.wheel(0, deltaY); await page.waitForTimeout(150); }
  await page.waitForTimeout(400);
}

// Pick a workflow in the composer, read the embedded one, then draw and test a new one.
async function workflows(page) {
  await page.locator('button', { hasText: 'New session' }).first().click();
  const picker = page.getByText('Plan, build, verify', { exact: true }).last();
  await picker.waitFor();
  await picker.click();
  await page.waitForTimeout(300);
  await shot(page, 'workflows-1', [[picker, 1], [page.getByText('Quick build', { exact: true }).last(), 2]]);
  await page.keyboard.press('Escape');

  await page.goto(`${BASE}/#/workflows`);
  const embedded = page.getByRole('button', { name: /^Plan, build, verify/ });
  await embedded.waitFor();
  await shot(page, 'workflows-2', [[embedded, 1]]);
  await embedded.click();
  const pane = page.locator('.react-flow__pane');
  await pane.waitFor();
  await page.waitForTimeout(800);
  const palette = page.getByText('Nodes', { exact: true }).locator('..');
  const save = page.getByRole('button', { name: 'Save', exact: true });
  await shot(page, 'workflows-3', [[palette, 1], [page.locator('.react-flow').first(), 2], [page.getByText('Loop (return)').locator('..'), 3], [save, 4]]);

  await page.waitForTimeout(1000); // fitView settles
  await zoom(page, await centre(flowNode(page, 'approve')), -150);
  await flowNode(page, 'approve').click();
  await page.waitForTimeout(300);
  await shot(page, 'workflows-4', [[flowNode(page, 'tests'), 1], [flowNode(page, 'approve'), 2], [page.getByText('Choices', { exact: true }).locator('..'), 3]]);
  const revise = await edgePoint(page, 'approve-Revise-plan');
  await page.mouse.click(revise.x, revise.y);
  await page.waitForTimeout(300);
  await shot(page, 'workflows-5', [[page.locator('.react-flow__edge[data-id="approve-Revise-plan"]'), 1], [page.getByText('Max passes', { exact: true }).locator('..'), 2], [page.getByText('Pause before the target').locator('..'), 3]]);

  await page.goto(`${BASE}/#/workflows`);
  const name = page.getByPlaceholder('new-workflow');
  await name.waitFor();
  await name.fill('build-and-test');
  const create = page.getByRole('button', { name: 'New workflow', exact: true });
  await shot(page, 'workflows-6', [[name, 1], [create, 2]]);
  await create.click();
  await flowNode(page, 'done').waitFor();
  await page.waitForTimeout(800);
  await zoom(page, await centre(pane), 200);
  const start = await centre(flowNode(page, 'start'));
  await drag(page, await centre(flowNode(page, 'done')), { x: start.x, y: start.y + 420 });
  const paneBox = await pane.boundingBox();
  const at = (dy, dx = 0) => ({ targetPosition: { x: start.x - paneBox.x + dx, y: start.y - paneBox.y + dy } });
  await page.getByRole('button', { name: 'Agent', exact: true }).dragTo(pane, at(110));
  await page.getByRole('button', { name: 'Shell', exact: true }).dragTo(pane, at(250));
  await flowNode(page, 'shell').waitFor();
  await connect(page, 'start', null, 'agent');
  await connect(page, 'agent', null, 'shell');
  await connect(page, 'shell', 'success', 'done');
  await connect(page, 'shell', 'failure', 'agent');
  await page.locator('.react-flow__pane').click({ position: { x: 400, y: 150 } }); // deselect: the side panel shows the workflow
  await page.waitForTimeout(1000); // the editor validates 400 ms after the last change
  await shot(page, 'workflows-7', [[palette, 1], [flowNode(page, 'agent'), 2], [flowNode(page, 'shell'), 3]]);

  const nameField = page.getByText('Name', { exact: true }).locator('..').getByRole('textbox');
  await flowNode(page, 'agent').click();
  await nameField.fill('Build');
  await flowNode(page, 'shell').click();
  await nameField.fill('Run the tests');
  const command = page.getByText('Command', { exact: true }).locator('..').getByRole('textbox');
  await command.fill('python -m pytest -q');
  await page.waitForTimeout(1000);
  await shot(page, 'workflows-8', [[flowNode(page, 'shell'), 1], [command, 2]]);

  // A loop needs a cap: once spent, the shell's exhausted port leads to an end that says stuck.
  const failure = await edgePoint(page, 'shell-failure-agent');
  await page.mouse.click(failure.x, failure.y);
  const maxPasses = page.locator('aside input[type=number]');
  await maxPasses.fill('3');
  await flowNode(page, 'shell').locator('[data-handleid="exhausted"]').waitFor();
  await page.getByRole('button', { name: 'End', exact: true }).dragTo(pane, at(400, -220));
  await connect(page, 'shell', 'exhausted', 'end');
  await flowNode(page, 'end').click();
  await nameField.fill('Stuck');
  await page.locator('aside select').selectOption('blocked');
  await page.waitForTimeout(1000);
  await shot(page, 'workflows-9', [[page.locator('.react-flow__edge[data-id="shell-failure-agent"]'), 1], [flowNode(page, 'shell').locator('[data-handleid="exhausted"]'), 2], [flowNode(page, 'end'), 3]]);

  await save.click();
  const test = page.getByRole('button', { name: 'Test step by step' });
  await test.click({ trial: true }); // enabled once saved
  await shot(page, 'workflows-10', [[page.getByLabel('Save scope'), 1], [save, 2], [test, 3]]);
  await test.click();
  const stepByStep = page.getByText('Step by step ✕', { exact: true });
  await stepByStep.waitFor();
  await shot(page, 'workflows-11', [[page.getByText('build-and-test', { exact: true }).last(), 1], [stepByStep, 2]]);
}

async function tasks(page) {
  const project = await page.evaluate(async () => (await (await fetch('/api/projects')).json()).find((p) => p.name === 'todo-api'));
  await page.goto(`${BASE}/#/project/${project.id}/tasks`);
  const newIdea = page.getByRole('button', { name: 'New idea' });
  await newIdea.waitFor();
  await page.waitForTimeout(500);
  const queueCard = page.getByText('0 ready', { exact: true }).locator('../..');
  await shot(page, 'tasks-1', [[newIdea, 1], [queueCard, 2]]);

  await newIdea.click();
  const ideaText = page.getByPlaceholder(/The idea, in a few lines/);
  await ideaText.fill('Add a due date to todos\nEach todo can carry an optional due date; GET /todos?overdue=true lists the late ones.');
  await shot(page, 'tasks-2', [[ideaText, 1], [page.getByRole('button', { name: 'Add the idea' }), 2]]);
  await page.getByRole('button', { name: 'Add the idea' }).click();

  const turn = page.getByRole('button', { name: 'Turn into a task' });
  await turn.waitFor();
  await page.waitForTimeout(500);
  await shot(page, 'tasks-3', [[page.getByRole('button', { name: 'Edit', exact: true }), 1], [turn, 2], [page.getByRole('button', { name: 'Actions' }), 3]]);

  await turn.click();
  const objective = page.getByText('Objective', { exact: true }).locator('../..');
  await objective.waitFor();
  await page.waitForTimeout(500);
  await shot(page, 'tasks-4', [[objective, 1], [page.getByText(/^required \d\/2/), 2], [page.getByRole('button', { name: 'Suggest with the model' }).first(), 3], [page.getByRole('tab', { name: 'Framing' }), 4]]);

  await page.getByPlaceholder('Why now, what exists, what the agent should read first.').fill('app.py holds the routes; tests live in tests/.');
  await page.getByRole('button', { name: 'Add a criterion' }).click();
  const criterion = page.getByPlaceholder('A criterion, one line');
  await criterion.fill('GET /todos?overdue=true returns only late todos');
  await page.getByPlaceholder('Check').fill('python -m pytest -q');
  await page.getByRole('button', { name: 'Add a criterion' }).scrollIntoViewIfNeeded();
  await page.waitForTimeout(1200); // the sheet saves after a pause in typing
  const ready = page.getByRole('button', { name: 'Ready', exact: true }).last();
  await shot(page, 'tasks-5', [[criterion, 1], [page.getByPlaceholder('Check'), 2], [ready, 3]]);

  await page.getByRole('tab', { name: 'Framing' }).click();
  const frame = page.getByRole('button', { name: 'Frame with the agent' });
  await frame.waitFor();
  await shot(page, 'tasks-6', [[frame, 1]]);
  await page.getByRole('tab', { name: 'Sheet' }).click();

  await ready.click();
  await page.waitForTimeout(800);
  await page.getByRole('dialog').getByRole('button', { name: 'Close' }).click();
  await page.getByText('Overview', { exact: true }).last().click();
  const next = page.getByText(/^Next: Add a due date/).first();
  await next.waitFor();
  await page.mouse.move(700, 700);
  await shot(page, 'tasks-7', [[next, 1], [page.locator('label', { hasText: 'Night mode' }), 2], [page.getByRole('button', { name: 'Night mode caps (Settings)' }), 3], [page.getByRole('button', { name: 'Start the queue' }), 4]]);

  await page.getByRole('button', { name: 'Night mode caps (Settings)' }).click();
  const minutes = page.getByText('Max duration per idea (minutes)', { exact: true }).locator('../..');
  await minutes.waitFor();
  await shot(page, 'tasks-8', [[minutes, 1], [page.getByText('Max tokens per idea', { exact: true }).locator('../..'), 2]]);
}

// Two plugins: turnstats, the example of docs/devs/plugins.md taken from the page itself so the
// guide shows the code it documents, and the resources example shipped in examples/plugins.
async function installPlugins(home) {
  const agents = join(home, '.agents');
  const page = await readFile(join(repo, 'docs/devs/plugins.md'), 'utf8');
  const turnstats = page.match(/```python\n([\s\S]*?)```/)[1];
  await mkdir(join(agents, 'plugins/turnstats'), { recursive: true });
  await writeFile(join(agents, 'plugins/turnstats/turnstats.py'), turnstats);
  await cp(join(repo, 'examples/plugins/resources'), join(agents, 'plugins/resources'), { recursive: true });
  const resources = JSON.parse(await readFile(join(repo, 'examples/plugins/resources/plugin.json'), 'utf8'));
  const definitions = [
    { name: 'turnstats', command: 'python3', args: ['turnstats.py'], env: {}, icon: 'activity',
      blocks: [{ id: 'status', title: 'Turnstats', scope: 'global' }, { id: 'tools', title: 'Tool calls', scope: 'session' }] },
    resources,
  ];
  await writeFile(join(agents, 'plugins.json'), JSON.stringify(definitions, null, 2));
}

async function plugins(page) {
  await installPlugins(HOME);
  await page.goto(`${BASE}/#/settings/plugins`);
  const row = page.getByText('turnstats', { exact: true });
  await row.waitFor();
  await shot(page, 'plugins-1', [[row.locator('../..'), 1], [page.getByTitle('Network').first(), 2], [page.getByTitle('Enable', { exact: true }).first(), 3]]);
  for (let i = 0; i < 2; i++) {
    await page.getByTitle('Enable', { exact: true }).first().click();
    await page.waitForTimeout(1000);
  }
  const running = page.getByText('running', { exact: true });
  await running.waitFor();
  const ram = page.getByText(/RAM/).last();
  await ram.waitFor({ timeout: 15_000 });
  // The CPU figure needs two resource samples, five seconds apart.
  await page.getByText(/\d %/).first().waitFor({ timeout: 15_000 });
  await page.mouse.move(700, 700);
  await shot(page, 'plugins-2', [[page.getByText('Enabled', { exact: true }).first(), 1], [page.getByText('Turnstats', { exact: true }).locator('../..'), 2], [ram.locator('..'), 3]]);
}

// The screens of the tour pages, on the state firstSession leaves: one session, one plan, one integration.
async function tour(page) {
  const project = await page.evaluate(async () => (await (await fetch('/api/projects')).json()).find((p) => p.name === 'todo-api'));
  const idle = () => page.mouse.move(700, 880);

  await page.goto(`${BASE}/#/`);
  const bar = page.getByRole('banner');
  const newChat = bar.getByRole('button', { name: 'New chat', exact: true });
  await newChat.waitFor();
  await idle();
  await shot(page, 'ways-1', [[newChat, 1, 3, true], [bar.getByRole('button', { name: 'Projects', exact: true }), 2, 3, true], [bar.getByRole('button', { name: 'Tasks', exact: true }), 3, 3, true], [bar.getByRole('button', { name: 'Workflows', exact: true }), 4, 3, true]]);

  await page.goto(`${BASE}/#/project/${project.id}/sessions`);
  const sessionsTab = page.getByRole('tab', { name: /^Sessions/ });
  const row = page.locator('li.cursor-pointer').first();
  await row.waitFor();
  await idle();
  await shot(page, 'sessions-1', [[sessionsTab, 1], [row, 2], [page.getByRole('button', { name: 'New session', exact: true }), 3]]);

  await row.click();
  const composerBox = page.getByText('Message', { exact: true }).locator('..');
  await composerBox.waitFor();
  const doneWhen = page.getByRole('main').getByText('Done when', { exact: true }).last().locator('../..');
  await doneWhen.waitFor({ timeout: 30000 });
  const card = doneWhen.locator('xpath=ancestor::div[contains(@class,"overflow-hidden")][1]');
  await card.waitFor({ timeout: 30000 });
  await page.waitForTimeout(1500);
  await idle();
  await shot(page, 'sessions-2', [[page.getByRole('main').locator('div.overflow-y-auto').first(), 1], [composerBox, 2], [(() => { const tabs = page.getByRole('navigation', { name: 'Session context' }).getByRole('button'); return [tabs, tabs]; })(), 3, 2]]);

  const follow = page.getByRole('switch', { name: /auto-follow/i });
  if (await follow.count()) await follow.uncheck();
  const steps = doneWhen.locator('xpath=..');
  await doneWhen.evaluate((el) => el.scrollIntoView({ block: 'end' }));
  await page.waitForTimeout(500);
  await shot(page, 'plans-1', [[steps, 1], [doneWhen, 2]]);

  const rail = page.getByRole('complementary', { name: 'Work' });
  const criterion = rail.locator('details', { hasText: 'C1' }).first();
  await criterion.waitFor();
  await criterion.scrollIntoViewIfNeeded();
  await idle();
  await shot(page, 'plans-2', [[criterion, 1], [rail.getByText(/^(verified|indicative)$/).first(), 2]]);

  // The module map reads Rust and TS/JS: the understanding shots use a clone of this repository.
  const clone = join(HOME, 'projects', 'fennec');
  execFileSync('git', ['clone', '-q', repo, clone], { env: { ...process.env, HOME } });
  const own = await page.evaluate(async (workdir) => (await fetch('/api/projects', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ workdir }) })).json(), clone);

  await page.goto(`${BASE}/#/project/${own.id}/map`);
  const mapTab = page.getByRole('tab', { name: 'Module map' });
  await page.getByText('Modules', { exact: true }).first().waitFor();
  await page.getByText(/^[1-9]\d* entry point/).waitFor({ timeout: 60_000 });
  await idle();
  await shot(page, 'understanding-1', [[mapTab, 1], [page.locator('section').first(), 2]]);

  await page.goto(`${BASE}/#/project/${own.id}/risks`);
  const risksTab = page.getByRole('tab', { name: 'Risks' });
  const zones = page.getByText('Risk zones', { exact: true }).locator('../../..');
  await zones.waitFor();
  await page.waitForTimeout(1500);
  await idle();
  await shot(page, 'understanding-2', [[risksTab, 1], [zones, 2]]);

  await page.goto(`${BASE}/#/edit/${project.id}/${encodeURIComponent('src/main.py')}`);
  const tree = page.getByRole('navigation', { name: 'Files' });
  const fileTab = page.getByRole('tab', { name: /main\.py/ }).last();
  const code = page.locator('.cm-editor');
  await code.waitFor();
  await idle();
  await shot(page, 'files-1', [[tree, 1], [fileTab, 2], [code, 3]]);

  // The commit flow: one line typed in README.md, saved on its own, then reset so later shots are unaffected.
  await tree.getByText('README.md', { exact: true }).click();
  await page.getByRole('button', { name: 'Edit the file', exact: true }).click();
  await code.locator('.cm-content').click();
  await page.keyboard.press('Control+End');
  await page.keyboard.type('\nThe add_todo route appends a to-do.');
  const changed = page.getByRole('checkbox', { name: 'README.md' });
  await changed.waitFor({ timeout: 15_000 });
  await page.getByRole('checkbox', { name: 'README.md', checked: true }).waitFor();
  const message = page.getByPlaceholder('Commit message');
  await message.fill('docs: mention add_todo');
  await idle();
  await shot(page, 'files-2', [[page.getByRole('button', { name: 'Back to reading', exact: true }), 1], [changed.locator('..'), 2], [message, 3], [page.getByRole('button', { name: 'Commit', exact: true }), 4]]);
  await page.getByRole('button', { name: 'Reset to the last commit', exact: true }).click();
  await page.getByRole('button', { name: 'Reset', exact: true }).click();
  await page.getByText('No changed file.').waitFor();

  await page.goto(`${BASE}/#/insight`);
  const projectSelect = page.getByRole('combobox');
  const period = page.getByRole('group', { name: 'Period' });
  const cards = page.locator('div.grid', { hasText: 'Calls' }).first();
  const chart = page.locator('div.p-4', { hasText: 'Input per day' }).last();
  await chart.waitFor();
  await page.waitForTimeout(1000);
  await idle();
  await shot(page, 'insight-1', [[projectSelect, 1], [period, 2], [cards, 3], [chart, 4]]);
}

const allScenes = [connectModel, openProject, firstSession, workflows, tasks, plugins, tour, remoteAccess];
// DOC_SHOTS_SCENES=connectModel,remoteAccess runs a subset (connectModel first: the others need a model).
const scenes = process.env.DOC_SHOTS_SCENES ? allScenes.filter((s) => process.env.DOC_SHOTS_SCENES.split(',').includes(s.name)) : allScenes;
let HOME;

const main = async () => {
  await access(FENNEC_BIN).catch(() => { throw new Error(`Fennec binary not found: ${FENNEC_BIN}`); });
  await mkdir(OUT, { recursive: true });
  const tmp = await mkdtemp(join(tmpdir(), 'fennec-shots-'));
  const home = join(tmp, 'home');
  await mkdir(home);
  await mkdir(join(tmp, 'store'));
  HOME = home;
  await demoRepo(home);
  const server = spawn(FENNEC_BIN, ['--port', String(PORT)], { cwd: tmp, env: { ...process.env, HOME: home, FENNEC_DB: join(tmp, 'store', 'fennec.db') }, stdio: 'ignore' });
  let browser;
  try {
    await waitForServer(BASE);
    browser = await chromium.launch();
    const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, colorScheme: 'dark' });
    const page = await context.newPage();
    for (const scene of scenes) await scene(page);
  } finally {
    if (browser) await browser.close();
    server.kill();
    await rm(tmp, { recursive: true, force: true });
  }
};

main().catch((err) => { console.error(err); process.exit(1); });
