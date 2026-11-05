import { describe, it, expect } from 'vitest';
import { createThreadFold, type ThreadItem, type ToolCallView } from './fold';

const ev = (type: string, data: Record<string, unknown> = {}) => ({ type, data });

type SegmentLike = { kind: string; content: string };

const segmentsOf = (items: ThreadItem[], id: string): SegmentLike[] => {
  const item = items.find((candidate) => candidate.id === id);
  if (!item || item.kind !== 'assistant') return [];
  return item.segments as SegmentLike[];
};

const toolOf = (items: ThreadItem[], id: string): ToolCallView | undefined => {
  for (const item of items) {
    if (item.kind !== 'assistant') continue;
    for (const segment of item.segments) {
      if (segment.kind === 'tool' && segment.tool.id === id) return segment.tool;
    }
  }
  return undefined;
};

describe('createThreadFold', () => {
  it('returns a new view object on each call', () => {
    const fold = createThreadFold();
    expect(fold.view()).not.toBe(fold.view());
  });

  it('adds a user message as a user item', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'u1', role: 'user', content: 'hello' }));
    expect(fold.view().items).toEqual([
      { kind: 'user', id: 'u1', content: 'hello', isSystemGenerated: false, attachments: [] }
    ]);
  });

  it('adds an assistant message with no segments', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    expect(fold.view().items).toEqual([{ kind: 'assistant', id: 'a1', segments: [], done: false }]);
  });

  it('accumulates deltas into one text segment', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('message.delta', { messageId: 'a1', content: 'Hello ' }));
    fold.apply(ev('message.delta', { messageId: 'a1', content: 'world' }));
    const segments = segmentsOf(fold.view().items, 'a1');
    expect(segments).toHaveLength(1);
    expect(segments[0]).toMatchObject({ kind: 'text', content: 'Hello world' });
  });

  it('adds thinking as a thinking segment before text', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('message.delta', { messageId: 'a1', content: 'text' }));
    fold.apply(ev('message.thinking', { messageId: 'a1', content: 'think' }));
    const segments = segmentsOf(fold.view().items, 'a1');
    expect(segments).toMatchObject([
      { kind: 'thinking', content: 'think' },
      { kind: 'text', content: 'text' }
    ]);
  });

  it('marks an assistant message done and clears live speed', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('message.delta', { messageId: 'a1', content: 'a', tokensPerSecond: 12 }));
    let item = fold.view().items[0] as { liveSpeed?: number; done: boolean };
    expect(item.liveSpeed).toBe(12);
    expect(item.done).toBe(false);
    fold.apply(ev('message.done', { messageId: 'a1' }));
    item = fold.view().items[0] as { liveSpeed?: number; done: boolean };
    expect(item.done).toBe(true);
    expect(item.liveSpeed).toBeUndefined();
  });

  it('creates a running tool segment inside an assistant item', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'read_file', arguments: { path: 'a.ts' } }
    }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.status).toBe('running');
    expect(tool?.args).toEqual({ path: 'a.ts' });
    expect(tool?.stdout).toBe('');
    expect(tool?.stderr).toBe('');
    expect(tool?.output).toBe('');
  });

  it('appends stdout and stderr to a tool call', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'run_command', arguments: { command: 'ls' } }
    }));
    fold.apply(ev('tool.output', { toolCallId: 't1', stream: 'stdout', content: 'out1' }));
    fold.apply(ev('tool.output', { toolCallId: 't1', stream: 'stderr', content: 'err' }));
    fold.apply(ev('tool.output', { toolCallId: 't1', stream: 'stdout', content: 'out2' }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.stdout).toBe('out1out2');
    expect(tool?.stderr).toBe('err');
  });

  it('sets success, duration, truncated, and output on tool.result', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'read_file', arguments: { path: 'a.ts' } }
    }));
    fold.apply(ev('tool.result', {
      toolCallId: 't1',
      result: { success: true, output: 'done', durationMs: 5, truncated: true }
    }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.status).toBe('success');
    expect(tool?.output).toBe('done');
    expect(tool?.durationMs).toBe(5);
    expect(tool?.truncated).toBe(true);
    expect(tool?.error).toBeUndefined();
  });

  it('sets error status and error message on a failed tool.result', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'run_command', arguments: { command: 'false' } }
    }));
    fold.apply(ev('tool.result', {
      toolCallId: 't1',
      result: { success: false, output: '', error: 'boom', durationMs: 1, truncated: false }
    }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.status).toBe('error');
    expect(tool?.error).toBe('boom');
    expect(tool?.truncated).toBe(false);
  });

  it('parses exit code from run_command output', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'run_command', arguments: { command: 'false' } }
    }));
    fold.apply(ev('tool.result', {
      toolCallId: 't1',
      result: { success: false, output: '[Exit code: 2]', durationMs: 1, truncated: false }
    }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.exitCode).toBe(2);
  });

  it('ignores tool.output for an unknown tool call', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.output', { toolCallId: 'nope', stream: 'stdout', content: 'x' }));
    expect(fold.view().items).toHaveLength(0);
  });

  it('ignores tool.result for an unknown tool call', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.result', { toolCallId: 'nope', result: { success: true } }));
    expect(fold.view().items).toHaveLength(0);
  });

  it('sets a pending question from chat.ask_user', () => {
    const fold = createThreadFold();
    fold.apply(ev('chat.ask_user', {
      callId: 'q1',
      question: 'What?',
      type: 'choice',
      options: [
        { value: 'a', label: 'A', description: 'd' },
        { value: 'b' }
      ]
    }));
    expect(fold.view().pendingQuestion).toEqual({
      callId: 'q1',
      question: 'What?',
      type: 'choice',
      options: [
        { value: 'a', label: 'A', description: 'd' },
        { value: 'b', label: 'b' }
      ]
    });
  });

  it('keeps a pending question after a user message', () => {
    const fold = createThreadFold();
    fold.apply(ev('chat.ask_user', { callId: 'q1', question: 'What?' }));
    fold.apply(ev('message.start', { messageId: 'u1', role: 'user', content: 'answer' }));
    expect(fold.view().pendingQuestion?.callId).toBe('q1');
  });

  it('clears a pending question when a new turn starts', () => {
    const fold = createThreadFold();
    fold.apply(ev('chat.ask_user', { callId: 'q1', question: 'What?' }));
    fold.apply(ev('running.changed', { isRunning: true }));
    expect(fold.view().pendingQuestion).toBeUndefined();
  });

  it('clears a pending question when its tool result arrives', () => {
    const fold = createThreadFold();
    fold.apply(ev('chat.ask_user', { callId: 'q1', question: 'What?' }));
    fold.apply(ev('tool.result', { toolCallId: 'q1', result: { success: true } }));
    expect(fold.view().pendingQuestion).toBeUndefined();
  });

  it('shows a rule.blocked on the tool card', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'run_command', arguments: { command: 'rm -rf' } }
    }));
    fold.apply(ev('rule.blocked', { callId: 't1', pattern: 'rm', scope: 'global' }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.blockedBy).toEqual({ pattern: 'rm', scope: 'global' });
  });

  it('replaces a propose_plan call by a plan segment on plan.proposed', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 'p1', name: 'propose_plan', arguments: {} }
    }));
    fold.apply(ev('plan.proposed', {
      callId: 'p1',
      plan: {
        title: 'Ship',
        goal: 'Goal',
        steps: [
          { title: 'Add code', files: ['a.ts'], change: 'implement' },
          { title: 'Docs', files: [], change: 'update' }
        ],
        doneWhen: [
          { text: 'tests pass', check: 'cargo test' },
          { text: 'docs updated' }
        ],
        risks: ['Risk one']
      },
      markdown: '## Plan: Ship'
    }));
    const item = fold.view().items[0];
    expect(item.kind).toBe('assistant');
    if (item.kind !== 'assistant') return;
    expect(item.segments).toEqual([
      {
        kind: 'plan',
        id: 'p1',
        plan: {
          kind: 'plan',
          title: 'Ship',
          goal: 'Goal',
          steps: [
            { number: 1, title: 'Add code', files: ['a.ts'], change: 'implement', verify: '' },
            { number: 2, title: 'Docs', files: [], change: 'update', verify: '' }
          ],
          doneWhen: ['tests pass — check: `cargo test`', 'docs updated'],
          risks: ['Risk one']
        },
        markdown: '## Plan: Ship'
      }
    ]);
    expect(toolOf(fold.view().items, 'p1')).toBeUndefined();
  });

  it('ignores a plan.proposed whose call is not in the thread', () => {
    const fold = createThreadFold();
    fold.apply(ev('plan.proposed', {
      callId: 'p1',
      plan: { title: 'Ship', goal: 'Goal', steps: [], doneWhen: [], risks: [] }
    }));
    expect(fold.view().items).toHaveLength(0);
  });

  it('folds write_file into modified files and content preview', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'write_file', arguments: { path: 'a.ts', content: 'line1\nline2' } }
    }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.contentPreview).toEqual(['line1', 'line2']);
    expect(tool?.addedLines).toBe(2);
    expect(tool?.removedLines).toBe(0);
    expect(fold.view().modifiedFiles).toEqual([{ path: 'a.ts', added: 2, removed: 0 }]);
  });

  it('accumulates modified file lines for repeated writes', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'write_file', arguments: { path: 'a.ts', content: 'one\ntwo' } }
    }));
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't2', name: 'write_file', arguments: { path: 'a.ts', content: 'three' } }
    }));
    expect(fold.view().modifiedFiles).toEqual([{ path: 'a.ts', added: 3, removed: 0 }]);
  });

  it('folds edit_file into modified files and diff', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: {
        id: 't1',
        name: 'edit_file',
        arguments: { path: 'a.ts', old_string: 'old1\nold2', new_string: 'new1' }
      }
    }));
    const tool = toolOf(fold.view().items, 't1');
    expect(tool?.diff).toEqual({ removed: ['old1', 'old2'], added: ['new1'] });
    expect(tool?.addedLines).toBe(1);
    expect(tool?.removedLines).toBe(2);
    expect(fold.view().modifiedFiles).toEqual([{ path: 'a.ts', added: 1, removed: 2 }]);
  });

  it('counts read_file without modifying files', () => {
    const fold = createThreadFold();
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'read_file', arguments: { path: 'a.ts' } }
    }));
    expect(fold.view().modifiedFiles).toHaveLength(0);
  });

  it('attaches preflight results to an assistant', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('criteria.preflight', {
      messageId: 'a1',
      results: [
        { criterionId: 'c1', command: 'cmd', verdict: 'passesAlready', exitCode: 0, output: 'ok' },
        { criterionId: 'c2', command: 'cmd2', verdict: 'fails', output: 'no' }
      ]
    }));
    const assistant = fold.view().items[0] as { preflight?: unknown[] };
    expect(assistant.preflight).toEqual([
      { criterionId: 'c1', command: 'cmd', verdict: 'passesAlready', exitCode: 0, output: 'ok' },
      { criterionId: 'c2', command: 'cmd2', verdict: 'fails', exitCode: undefined, output: 'no' }
    ]);
  });

  it('cuts the view back to before a rewound message', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'u1', role: 'user', content: 'one' }));
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('session.rewound', { messageId: 'a1' }));
    expect(fold.view().items).toEqual([
      { kind: 'user', id: 'u1', content: 'one', isSystemGenerated: false, attachments: [] }
    ]);
  });

  it('clears all items when rewound to an unknown message', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'u1', role: 'user', content: 'one' }));
    fold.apply(ev('session.rewound', { messageId: 'zz' }));
    expect(fold.view().items).toHaveLength(0);
  });

  it('clears a pending question when its tool is rewound away', () => {
    const fold = createThreadFold();
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('tool.call', {
      messageId: 'a1',
      toolCall: { id: 't1', name: 'ask_user', arguments: { question: 'Q' } }
    }));
    fold.apply(ev('chat.ask_user', { callId: 't1', question: 'Q' }));
    fold.apply(ev('session.rewound', { messageId: 'a1' }));
    expect(fold.view().pendingQuestion).toBeUndefined();
    expect(fold.view().items).toHaveLength(0);
  });

  it('adds a compaction note and removes the summarized assistant', () => {
    const fold = createThreadFold('fr');
    fold.apply(ev('message.start', { messageId: 'a1', role: 'assistant' }));
    fold.apply(ev('message.delta', { messageId: 'a1', content: 'summary' }));
    fold.apply(ev('context.compacted', { beforeTokens: 100, afterTokens: 20, summary: 'summary' }));
    const items = fold.view().items;
    expect(items).toHaveLength(1);
    const note = items[0] as { kind: string; detail?: string; text: string };
    expect(note.kind).toBe('note');
    expect(note.detail).toBe('summary');
    expect(note.text).toMatch(/^Contexte compacté · .* → ~.* tokens$/);
  });

  it('sets running state from running.changed', () => {
    const fold = createThreadFold();
    fold.apply(ev('running.changed', { isRunning: true }));
    expect(fold.view().running).toBe(true);
    fold.apply(ev('running.changed', { isRunning: false }));
    expect(fold.view().running).toBe(false);
  });

  it('sets lastDone from chat.done without a note for complete', () => {
    const fold = createThreadFold();
    fold.apply(ev('chat.done', { reason: 'complete' }));
    expect(fold.view().lastDone).toBe('complete');
    expect(fold.view().items).toHaveLength(0);
  });

  it('adds a stopped note from chat.done', () => {
    const fold = createThreadFold('fr');
    fold.apply(ev('chat.done', { reason: 'stopped' }));
    expect(fold.view().lastDone).toBe('stopped');
    expect(fold.view().items[0]).toMatchObject({ kind: 'note', text: 'Arrêtée' });
  });

  it('adds a truncated note from chat.done', () => {
    const fold = createThreadFold('fr');
    fold.apply(ev('chat.done', { reason: 'truncated' }));
    expect(fold.view().items[0]).toMatchObject({ kind: 'note', text: 'Tronquée' });
  });

  it('adds an error note from chat.done', () => {
    const fold = createThreadFold('fr');
    fold.apply(ev('chat.done', { reason: 'error' }));
    expect(fold.view().items[0]).toMatchObject({ kind: 'note', text: 'Erreur' });
  });

  it('adds an error item from chat.error', () => {
    const fold = createThreadFold();
    fold.apply(ev('chat.error', { error: 'boom', recoverable: true }));
    expect(fold.view().items).toEqual([{ kind: 'error', id: 'error-1', message: 'boom' }]);
  });

  it('sets mode from mode.changed', () => {
    const fold = createThreadFold();
    fold.apply(ev('mode.changed', { mode: 'builder' }));
    expect(fold.view().mode).toBe('builder');
  });

  it('sets name from session.name_generated', () => {
    const fold = createThreadFold();
    fold.apply(ev('session.name_generated', { name: 'Title' }));
    expect(fold.view().name).toBe('Title');
  });

  it('feeds workflow events into the run view', () => {
    const fold = createThreadFold();
    fold.apply(ev('workflow.started', {
      runId: 'r1',
      workflow: { id: 'w', name: 'W', nodes: [], edges: [] },
      task: 't',
      stepByStep: false
    }));
    expect(fold.view().run?.runId).toBe('r1');
    expect(fold.view().run?.task).toBe('t');
  });

  it('adds a workflow started note and task user item', () => {
    const fold = createThreadFold();
    fold.apply(ev('workflow.started', {
      runId: 'r1',
      workflow: { id: 'w', name: 'W', nodes: [], edges: [] },
      task: 't',
      stepByStep: false
    }));
    expect(fold.view().items).toEqual([
      { kind: 'note', id: 'workflow-started-1', text: 'Workflow « W »' },
      { kind: 'user', id: 'workflow-task-1', content: 't', isSystemGenerated: false, attachments: [] }
    ]);
  });

  it('adds a workflow node item for an agent node', () => {
    const fold = createThreadFold();
    fold.apply(ev('workflow.started', {
      runId: 'r1',
      workflow: {
        id: 'w',
        name: 'W',
        nodes: [{ id: 'a', kind: 'agent', name: 'A', position: { x: 0, y: 0 } }],
        edges: []
      },
      task: 't',
      stepByStep: false
    }));
    fold.apply(ev('workflow.node.started', {
      runId: 'r1',
      nodeId: 'a',
      pass: 1,
      childSessionId: 's1',
      startedAt: 123
    }));
    const nodeItem = fold.view().items.find((item) => item.kind === 'workflow-node');
    expect(nodeItem).toEqual({
      kind: 'workflow-node',
      id: 'wf-r1-a-1',
      nodeId: 'a',
      label: 'A',
      nodeKind: 'agent',
      pass: 1,
      childSessionId: 's1',
      startedAt: 123
    });
  });
});
