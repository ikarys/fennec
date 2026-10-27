import type { WorkflowNode } from '../../../lib/workflow';

interface ShapeProps {
  node: WorkflowNode;
}

export function StartShape({ node }: ShapeProps) {
  return (
    <div className="rounded-full bg-raised px-4 py-2 text-[13px] font-medium text-text">
      {node.name ?? node.id}
    </div>
  );
}

export function EndShape({ node }: ShapeProps) {
  const outcome = node.outcome as string | undefined;
  const isBlocked = outcome === 'blocked';
  return (
    <div
      className={`rounded-full px-4 py-2 text-[13px] font-medium ${
        isBlocked ? 'bg-err/20 text-err' : 'bg-ok-soft text-ok'
      }`}
    >
      {node.name ?? node.id}
    </div>
  );
}

export function AgentShape({ node }: ShapeProps) {
  const agent = node.agent as string | undefined;
  const prompt = node.prompt as string | undefined;
  return (
    <div className="w-48 rounded-lg border border-border bg-panel px-3 py-2">
      <div className="text-[13px] font-medium text-text">{node.name ?? node.id}</div>
      {agent && <div className="text-xs text-accent">{agent}</div>}
      {prompt && <div className="mt-1 truncate text-xs text-muted">{prompt}</div>}
    </div>
  );
}

export function ShellShape({ node }: ShapeProps) {
  const command = node.command as string | undefined;
  return (
    <div className="w-48 rounded-lg bg-code px-3 py-2 font-mono">
      <div className="text-[13px] font-medium text-text">{node.name ?? node.id}</div>
      <div className="mt-1 truncate text-xs text-muted">$ {command ?? ''}</div>
    </div>
  );
}

export function CriteriaShape({ node }: ShapeProps) {
  const plan = node.plan as string | undefined;
  return (
    <div className="w-48 rounded-lg border border-ok/30 bg-code px-3 py-2 font-mono">
      <div className="text-[13px] font-medium text-text">{node.name ?? node.id}</div>
      <div className="mt-1 truncate text-xs text-muted">plan : {plan ?? ''}</div>
    </div>
  );
}

export function HumanShape({ node }: ShapeProps) {
  const question = node.question as string | undefined;
  return (
    <div
      className="flex h-20 w-48 items-center justify-center bg-accent-soft px-6"
      style={{ clipPath: 'polygon(25% 0%, 75% 0%, 100% 50%, 75% 100%, 25% 100%, 0% 50%)' }}
    >
      <div className="text-center">
        <div className="text-[13px] font-medium text-text">{node.name ?? node.id}</div>
        {question && <div className="mt-0.5 truncate text-xs text-muted">{question}</div>}
      </div>
    </div>
  );
}

export function ConditionShape({ node }: ShapeProps) {
  const value = node.value as string | undefined;
  const test = node.test as string | undefined;
  const against = node.against as string | undefined;
  return (
    <div className="relative flex h-24 w-48 items-center justify-center">
      <div
        className="absolute inset-0 bg-raised"
        style={{ clipPath: 'polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)' }}
      />
      <div className="relative z-10 px-4 text-center">
        <div className="text-[13px] font-medium text-text">{node.name ?? node.id}</div>
        <div className="mt-0.5 text-xs text-muted">
          {value ?? ''} {test ?? ''} {against ?? ''}
        </div>
      </div>
    </div>
  );
}

export function ParallelShape({ node }: ShapeProps) {
  return (
    <div className="h-8 w-56 rounded-md bg-raised px-3 py-1 text-[13px] font-medium text-text">
      {node.name ?? node.id}
    </div>
  );
}

export function JoinShape({ node }: ShapeProps) {
  return (
    <div className="h-8 w-56 rounded-md bg-raised px-3 py-1 text-[13px] font-medium text-text">
      {node.name ?? node.id}
    </div>
  );
}
