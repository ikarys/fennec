import { createContext, useContext } from 'react';
import type { CallUsage, NodeUsage, RunUsage, WorkflowUsage } from '../../lib/api';

/** The ledger's figures for each answer of the thread, by message id. */
export const CallUsageContext = createContext<ReadonlyMap<string, CallUsage>>(new Map());

export function useCallUsage(messageId: string): CallUsage | undefined {
  return useContext(CallUsageContext).get(messageId);
}

/** The ledger's figures for the workflow runs of the thread. */
export const WorkflowUsageContext = createContext<WorkflowUsage>({ nodes: [], runs: [] });

export function useNodeUsage(itemId: string): NodeUsage | undefined {
  return useContext(WorkflowUsageContext).nodes.find((node) => node.itemId === itemId);
}

export function useRunUsage(runId: string): RunUsage | undefined {
  return useContext(WorkflowUsageContext).runs.find((run) => run.runId === runId);
}
