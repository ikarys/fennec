import { createContext, useContext } from 'react';

export const CanvasOpenerContext = createContext<((artifactId: number) => void) | undefined>(undefined);
export const CanvasSessionContext = createContext<string | undefined>(undefined);

export function useCanvasOpener(): ((artifactId: number) => void) | undefined {
  return useContext(CanvasOpenerContext);
}

export function useCanvasSession(): string | undefined {
  return useContext(CanvasSessionContext);
}
