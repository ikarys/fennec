// The link from a file path shown in a session to that file: the session's diff while its branch has commits, else the editor; null outside a session's project.
import { createContext, useContext } from 'react';

export const FileLinkContext = createContext<((path: string) => string) | null>(null);

export function useFileLink() {
  return useContext(FileLinkContext);
}
