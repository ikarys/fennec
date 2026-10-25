import { createContext, useContext } from 'react';
import type { PreviewKind } from '../../lib/preview';

export interface PreviewTarget {
  /** Project-relative path. */
  path: string;
  kind: PreviewKind;
}

/** Opens a file in the session's preview pane; undefined outside a session page. */
export const PreviewContext = createContext<((target: PreviewTarget) => void) | undefined>(undefined);
export const PreviewWorkdirContext = createContext<string | undefined>(undefined);

export function usePreviewOpener(): ((target: PreviewTarget) => void) | undefined {
  return useContext(PreviewContext);
}

export function usePreviewWorkdir(): string | undefined {
  return useContext(PreviewWorkdirContext);
}
