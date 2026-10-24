# Uses a page the way its user would: real mouse clicks and key presses
# through the browser's DevTools pipe, then a screenshot. Stdlib only; runs
# inside the sandbox. Arguments: url, steps as a JSON list of strings.
# Writes the PNGs back to back on stdout: [after] without steps, [before,
# after] with them; and one stderr line `proof={"sizes": [...], "text": n}`
# or `proof={"error": why}`.
import base64, json, os, subprocess, sys, time

url, steps = sys.argv[1], json.loads(sys.argv[2])
to_browser, browser_in = os.pipe()
browser_out, from_browser = os.pipe()


def pipe_fds():
    os.dup2(to_browser, 3)
    os.dup2(from_browser, 4)
    # Pipes are close-on-exec, and dup2 onto the same number keeps the flag.
    os.set_inheritable(3, True)
    os.set_inheritable(4, True)


try:
    browser = subprocess.Popen(
        [os.environ.get("FENNEC_OS_BROWSER", "chromium"), "--headless=new", "--no-sandbox",
         "--disable-gpu", "--hide-scrollbars", "--window-size=1280,800",
         "--remote-debugging-pipe", "--user-data-dir=/tmp/proof-browser", "about:blank"],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        preexec_fn=pipe_fds, close_fds=False)
except FileNotFoundError:
    # Not the page's fault: this machine cannot look at it.
    print('proof={"error": "no browser", "missing": true}', file=sys.stderr)
    sys.exit(1)
os.close(to_browser)
os.close(from_browser)
out = os.fdopen(browser_in, "wb")
inbox = os.fdopen(browser_out, "rb")
seq = 0
session = None


def cdp(method, **params):
    global seq
    seq += 1
    msg = {"id": seq, "method": method, "params": params}
    if session:
        msg["sessionId"] = session
    out.write(json.dumps(msg).encode() + b"\0")
    out.flush()
    buf = b""
    while True:
        chunk = inbox.read1(65536)
        if not chunk:
            raise RuntimeError("the browser closed")
        buf += chunk
        while b"\0" in buf:
            raw, buf = buf.split(b"\0", 1)
            reply = json.loads(raw)
            if reply.get("id") == seq:
                if "error" in reply:
                    raise RuntimeError(f"{method}: {reply['error'].get('message')}")
                return reply.get("result", {})


def js(expr):
    r = cdp("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
    if "exceptionDetails" in r:
        raise RuntimeError(r["exceptionDetails"].get("text", "script error"))
    return r["result"].get("value")


def settle():
    for _ in range(50):
        if js("document.readyState") == "complete":
            break
        time.sleep(0.1)
    time.sleep(0.5)


# Down to the lowest visible element: a short page shot at window height is
# mostly blank, and shrunk into a card its content becomes unreadable.
BOTTOM = """Math.min(innerHeight, Math.ceil(Math.max(40, ...[...document.body.querySelectorAll('*')]
  .map(e => e.getBoundingClientRect()).filter(b => b.width && b.height).map(b => b.bottom))) + 16)"""


def shot():
    clip = {"x": 0, "y": 0, "width": js("innerWidth"), "height": js(BOTTOM), "scale": 1}
    return base64.b64decode(cdp("Page.captureScreenshot", format="png", clip=clip)["data"])


# What a user can aim at: the visible interactive elements, by name.
LABELS = """
[...document.querySelectorAll('button, a, [role="button"], input, textarea, select, [contenteditable]')]
.filter(e => { const b = e.getBoundingClientRect(); return b.width > 0 && b.height > 0; })
.map(e => (e.innerText || e.getAttribute('aria-label') || e.placeholder || '').trim().slice(0, 40))
.filter(t => t)
"""


# The element a user would aim at: a visible one showing the text (the
# innermost), or a field by placeholder, label, name or CSS selector.
FIND = """(([what, field]) => {
  const seen = el => { const b = el.getBoundingClientRect(); return b.width > 0 && b.height > 0; };
  let el = null;
  if (field) {
    const fields = [...document.querySelectorAll("input, textarea, select, [contenteditable]")].filter(seen);
    el = fields.find(f => f.placeholder === what || f.getAttribute("aria-label") === what || f.name === what
      || [...(f.labels || [])].some(l => l.innerText.trim() === what));
  }
  if (!el) {
    const all = [...document.body.querySelectorAll("*")].filter(seen);
    // An icon-only button is named by its aria-label or title.
    const exact = all.filter(e => (e.innerText && e.innerText.trim() === what)
      || e.getAttribute("aria-label") === what || e.title === what);
    const near = all.filter(e => e.innerText && e.innerText.includes(what));
    const pick = list => list.find(e => !list.some(o => o !== e && e.contains(o)));
    el = pick(exact) || pick(near);
  }
  if (!el) { try { el = document.querySelector(what); } catch (e) {} }
  if (!el || !seen(el)) return null;
  el.scrollIntoView({block: "center", behavior: "instant"});
  const b = el.getBoundingClientRect(), x = b.x + b.width / 2, y = b.y + b.height / 2;
  const hit = document.elementFromPoint(x, y);
  return [x, y, !!hit && (hit === el || el.contains(hit) || hit.contains(el))];
})(%s)"""


def aim(what, field=False):
    found = js(FIND % json.dumps([what, field]))
    if not found:
        labels = js(LABELS) or []
        shown = []
        for t in labels:
            if t not in shown:
                shown.append(t)
        named = ", ".join(shown[:20]) if shown else "no visible label"
        raise RuntimeError(
            f"nothing on the page shows « {what} » (the page shows: {named})"
        )
    if not found[2]:
        raise RuntimeError(f"« {what} » is covered by something else")
    for kind in ("mouseMoved", "mousePressed", "mouseReleased"):
        cdp("Input.dispatchMouseEvent", type=kind, x=found[0], y=found[1], button="left", clickCount=1)


KEYS = {"Enter": ("\r", 13), "Tab": ("\t", 9), "Escape": ("", 27), "Backspace": ("", 8)}


# A user retypes a field, they do not grow it: the typed text replaces what
# is there. The click left the field focused; this selects its content so
# the next insert overwrites it.
SELECT = """((what) => {
  const seen = el => { const b = el.getBoundingClientRect(); return b.width > 0 && b.height > 0; };
  const fields = [...document.querySelectorAll("input, textarea, select, [contenteditable]")].filter(seen);
  const el = fields.find(f => f.placeholder === what || f.getAttribute("aria-label") === what || f.name === what
    || [...(f.labels || [])].some(l => l.innerText.trim() === what));
  if (!el) return false;
  el.focus();
  if (el.select) { el.select(); return true; }
  const range = document.createRange(); range.selectNodeContents(el);
  const sel = window.getSelection(); sel.removeAllRanges(); sel.addRange(range);
  return true;
})(%s)"""


def step(s):
    verb, _, arg = s.partition(" ")
    arg = arg.strip()
    if verb == "click":
        aim(arg)
    elif verb == "type":
        target, sep, text = arg.partition("=>")
        if not sep:
            raise RuntimeError("type needs <field> => <text>")
        aim(target.strip(), field=True)
        if not js(SELECT % json.dumps(target.strip())):
            raise RuntimeError(f"no field named « {target.strip()} » to type into")
        cdp("Input.insertText", text=text.strip())
    elif verb == "key":
        text, code = KEYS.get(arg, (arg, 0))
        for kind in ("keyDown", "keyUp"):
            cdp("Input.dispatchKeyEvent", type=kind, key=arg, text=text if kind == "keyDown" else "",
                windowsVirtualKeyCode=code)
    else:
        raise RuntimeError(f"unknown step: {verb}")
    time.sleep(0.3)


try:
    target = cdp("Target.createTarget", url="about:blank")["targetId"]
    session = cdp("Target.attachToTarget", targetId=target, flatten=True)["sessionId"]
    cdp("Page.enable")
    cdp("Page.navigate", url=url)
    settle()
    shots = [shot()] if steps else []
    for n, s in enumerate(steps, 1):
        try:
            step(s)
        except RuntimeError as e:
            raise RuntimeError(f"step {n} ({s}): {e}")
    settle()
    shots.append(shot())
    text = len("".join(js("document.body ? document.body.innerText : ''").split()))
    for png in shots:
        sys.stdout.buffer.write(png)
    sys.stdout.flush()
    print("proof=" + json.dumps({"sizes": [len(p) for p in shots], "text": text}), file=sys.stderr)
except Exception as e:
    print("proof=" + json.dumps({"error": str(e)}), file=sys.stderr)
finally:
    browser.kill()
