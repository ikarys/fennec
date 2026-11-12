// Fails when a component outside src/components/ui hand-writes the look of a
// primitive (docs/devs/ui.md): the look must live in one place, or screens drift.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = new URL('../src/', import.meta.url).pathname;
const primitives = join(root, 'components/ui');

// What a class string looks like when it restyles a primitive by hand. A raw
// <button> stays allowed for a surface that is itself the target (a list row,
// a tab), which only takes a background when hovered or selected; a border
// on top of the background is a button's look.
const RULES = [
  { primitive: 'Button variant="primary"', test: (c) => /\bbg-accent\b/.test(c) && /\btext-on-accent\b/.test(c) },
  { primitive: 'Button', test: (c, tag) => (tag === 'button' || tag === 'a') && /(^|[\s"'`])bg-raised\b/.test(c) && /(^|[\s"'`])border(\s|$)/.test(c) },
  { primitive: 'SectionLabel', test: (c) => /\buppercase\b/.test(c) && /\btracking-/.test(c) && /\bfont-semibold\b/.test(c) }
];

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (path === primitives) continue;
    if (statSync(path).isDirectory()) yield* files(path);
    else if (path.endsWith('.tsx')) yield path;
  }
}

const problems = [];
for (const path of files(root)) {
  const source = readFileSync(path, 'utf8');
  // Every string literal: class names also live in ternaries and templates.
  for (const match of source.matchAll(/"[^"\n]*"|'[^'\n]*'|`[^`]*`/g)) {
    // The element the string belongs to: the last tag opened before it.
    const tag = /<([a-zA-Z]+)[^<]*$/.exec(source.slice(Math.max(0, match.index - 600), match.index))?.[1];
    const rule = RULES.find((r) => r.test(match[0], tag));
    if (rule) {
      const line = source.slice(0, match.index).split('\n').length;
      problems.push(`${relative(process.cwd(), path)}:${line}: use ${rule.primitive} from components/ui instead of its classes`);
    }
  }
}

if (problems.length > 0) {
  console.error(problems.join('\n'));
  console.error(`\n${problems.length} hand-styled primitive(s); see docs/devs/ui.md.`);
  process.exit(1);
}
