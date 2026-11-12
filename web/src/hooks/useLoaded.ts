import { useEffect, useState, type DependencyList, type Dispatch, type SetStateAction } from 'react';

// Last loaded value per cache key for this page load only; a page shown again renders its last figures at once, then refreshes.
const cache = new Map<string, unknown>();

/**
 * What a component loads from the server when it mounts or its inputs change:
 * the value, or the error's message. A late answer for inputs that changed since
 * is dropped, and the previous value is cleared so another project's figures never show.
 * With a cacheKey, the last value for that key is kept while the fresh load runs,
 * so a page shown again renders its last figures at once; the key must be covered
 * by the caller's deps.
 */
export function useLoaded<T>(
  load: () => Promise<T>,
  deps: DependencyList,
  cacheKey?: string,
): { data: T | null; error: string | null; loading: boolean; setData: Dispatch<SetStateAction<T | null>> } {
  const [data, setData] = useState<T | null>(
    cacheKey !== undefined ? (cache.get(cacheKey) as T | undefined) ?? null : null
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (cacheKey !== undefined) {
      setData((cache.get(cacheKey) as T | undefined) ?? null);
    } else {
      setData(null);
    }
    setError(null);
    load()
      .then((value) => {
        if (!cancelled) {
          if (cacheKey !== undefined) cache.set(cacheKey, value);
          setData(value);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      });
    return () => {
      cancelled = true;
    };
    // The caller's deps say when to load again; `load` is a fresh closure each render.
    // A cacheKey must be covered by deps: a different key comes with different deps.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  return { data, error, loading: data === null && error === null, setData };
}
