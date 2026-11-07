import { useEffect, useState, type DependencyList, type Dispatch, type SetStateAction } from 'react';

/**
 * What a component loads from the server when it mounts or its inputs change:
 * the value, or the error's message. A late answer for inputs that changed since
 * is dropped, and the previous value is cleared so another project's figures never show.
 */
export function useLoaded<T>(
  load: () => Promise<T>,
  deps: DependencyList,
): { data: T | null; error: string | null; loading: boolean; setData: Dispatch<SetStateAction<T | null>> } {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setData(null);
    setError(null);
    load()
      .then((value) => {
        if (!cancelled) setData(value);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
    // The caller's deps say when to load again; `load` is a fresh closure each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return { data, error, loading: data === null && error === null, setData };
}
