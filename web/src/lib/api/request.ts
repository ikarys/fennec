import type { WorkflowIssue } from "../workflow";

export type IntegrationConflict = { files: string[]; command: string };

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    headers: {
      "Content-Type": "application/json",
    },
    ...init,
  });

  if (!response.ok) {
    let message = response.statusText;
    let errors: WorkflowIssue[] | undefined;
    let conflict: IntegrationConflict | undefined;
    try {
      const body = (await response.json()) as {
        error?: string;
        errors?: WorkflowIssue[];
        conflict?: IntegrationConflict;
      };
      if (body.error) {
        message = body.error;
      }
      if (Array.isArray(body.errors)) {
        errors = body.errors;
      }
      if (body.conflict) {
        conflict = body.conflict;
      }
    } catch {
      // Keep the status text when the error body is not JSON.
    }
    const error = new Error(message);
    if (errors) {
      (error as { issues?: WorkflowIssue[] }).issues = errors;
    }
    if (conflict) {
      (error as { conflict?: IntegrationConflict }).conflict = conflict;
    }
    throw error;
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const text = await response.text();
  if (!text) {
    return undefined as T;
  }

  return JSON.parse(text) as T;
}
