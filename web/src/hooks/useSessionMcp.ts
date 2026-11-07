import { useEffect, useState } from 'react';
import { getDraftMcp, getSessionMcp, setSessionMcp } from '../lib/api';
import type { McpSessionView } from '../lib/api';

export function useSessionMcp(
  sessionId: string | null,
  refreshKey: number
): {
  mcpView: McpSessionView | null;
  draftMcpDisabled: string[] | null;
  toggleMcp: (name: string, active: boolean) => void;
} {
  const [mcpView, setMcpView] = useState<McpSessionView | null>(null);
  const [draftMcpDisabled, setDraftMcpDisabled] = useState<string[] | null>(null);

  // The composer's MCP dropdown: the session's effective MCP state, refetched
  // when the session changes or a turn ends (a global flip on the Settings
  // page shows up on the next turn), the toggle handler updates it directly.
  // A draft asks for the view the first turn would get, so the picker is not
  // empty before the session exists.
  useEffect(() => {
    let active = true;
    if (!sessionId) {
      setDraftMcpDisabled(null);
      getDraftMcp(null)
        .then((data) => {
          if (active) setMcpView(data);
        })
        .catch(() => {
          if (active) setMcpView(null);
        });
      return () => {
        active = false;
      };
    }
    getSessionMcp(sessionId)
      .then((data) => {
        if (active) setMcpView(data);
      })
      .catch(() => {
        if (active) setMcpView(null);
      });
    return () => {
      active = false;
    };
  }, [sessionId, refreshKey]);

  function toggleMcp(name: string, active: boolean) {
    if (!mcpView) return;
    // The disabled set is the full set (last wins), so it must include hidden
    // servers too: a hidden server left out would silently turn on.
    const disabled = mcpView.servers
      .filter((server) => (server.name === name ? !active : !server.active))
      .map((server) => server.name);
    if (sessionId) {
      setSessionMcp(sessionId, disabled)
        .then(setMcpView)
        .catch(() => {
          // Keep the optimistic view; the next fetch corrects it.
        });
    } else {
      setDraftMcpDisabled(disabled);
      getDraftMcp(disabled)
        .then(setMcpView)
        .catch(() => {
          // Keep the optimistic view; the next fetch corrects it.
        });
    }
  }

  return { mcpView, draftMcpDisabled, toggleMcp };
}
