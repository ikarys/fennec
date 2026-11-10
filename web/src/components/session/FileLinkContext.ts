// The link from a file path shown in a session to that file in the editor page, as the session's branch has it; null outside a session's project.
import { createContext, useContext } from 'react';

export const FileLinkContext = createContext<((path: string) => string) | null>(null);

export function useFileLink() {
  return useContext(FileLinkContext);
}
