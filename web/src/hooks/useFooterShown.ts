// Whether the app's footer (the status bar) is shown: hidden by default, the human's choice kept across reloads.
import { useCallback, useState } from 'react';

const STORAGE_KEY = 'fennec.footer';

function readInitialFooterShown(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === '1';
  } catch {
    return false;
  }
}

export function useFooterShown(): [boolean, () => void] {
  const [footerShown, setFooterShown] = useState<boolean>(() => readInitialFooterShown());

  const toggleFooter = useCallback(() => {
    setFooterShown((current) => {
      const next = !current;
      try {
        localStorage.setItem(STORAGE_KEY, next ? '1' : '0');
      } catch {
        // A blocked storage only loses the choice on reload.
      }
      return next;
    });
  }, []);

  return [footerShown, toggleFooter];
}
