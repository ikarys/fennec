// A tool card and a thinking segment follow the "open by default" settings
// live, instead of freezing their open state at mount or staying open after a
// run_command finishes. The toggle button's aria-expanded is the contract.
import { describe, it, expect } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { ToolCard } from './ToolCard';
import { ThreadItemView } from './Thread';
import { DisplayPrefsProvider } from '../../lib/display';
import type { DisplayPrefs } from '../../lib/api';
import type { AssistantItem, ToolCallView } from '../../lib/threadView';

function prefs(toolResultsOpen: boolean, reasoningOpen: boolean): DisplayPrefs {
  return { reasoningOpen, toolResultsOpen, fontSize: 14, language: 'en', sounds: true };
}

/** The toggle button's open state in the rendered markup. */
function expanded(markup: string): boolean {
  return /aria-expanded="true"/.test(markup);
}

const runningCommand: ToolCallView = {
  id: 't-run',
  name: 'run_command',
  args: { command: 'ls' },
  status: 'running',
  stdout: '',
  stderr: '',
  output: ''
};

const finishedCommand: ToolCallView = {
  id: 't-done',
  name: 'run_command',
  args: { command: 'ls' },
  status: 'success',
  stdout: '',
  stderr: '',
  output: 'ok',
  exitCode: 0
};

const failedReport: ToolCallView = {
  id: 't-report',
  name: 'submit_report',
  args: {},
  status: 'error',
  stdout: '',
  stderr: '',
  output: "step 1 has no non-empty 'title' string."
};

const thinkingItem: AssistantItem = {
  kind: 'assistant',
  id: 'a-1',
  segments: [{ kind: 'thinking', id: 'th-1', content: 'reasoning' }],
  done: true
};

function renderTool(tool: ToolCallView, toolResultsOpen: boolean): string {
  return renderToStaticMarkup(
    <DisplayPrefsProvider value={prefs(toolResultsOpen, false)}>
      <ToolCard tool={tool} />
    </DisplayPrefsProvider>
  );
}

function renderThinking(reasoningOpen: boolean): string {
  return renderToStaticMarkup(
    <DisplayPrefsProvider value={prefs(false, reasoningOpen)}>
      <ThreadItemView item={thinkingItem} model="test" running={false} showHeader={false} />
    </DisplayPrefsProvider>
  );
}

describe('tool results open by default', () => {
  it('a running command stays open', () => {
    expect(expanded(renderTool(runningCommand, false))).toBe(true);
  });

  it('a finished command is closed when the setting is off', () => {
    expect(expanded(renderTool(finishedCommand, false))).toBe(false);
  });

  it('a failed report is closed when the setting is off', () => {
    expect(expanded(renderTool(failedReport, false))).toBe(false);
  });

  it('a finished command is open when the setting is on', () => {
    expect(expanded(renderTool(finishedCommand, true))).toBe(true);
  });
});

describe('reasoning open by default', () => {
  it('a thinking segment is closed when the setting is off', () => {
    expect(expanded(renderThinking(false))).toBe(false);
  });

  it('a thinking segment is open when the setting is on', () => {
    expect(expanded(renderThinking(true))).toBe(true);
  });
});
