import { useEffect, useState } from 'react';

export function useFetchedText(url: string | undefined, reloadKey: unknown): { text: string | null; loading: boolean; error: string | null } {
  const [text, setText] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!url) {
      setText(null);
      setLoading(false);
      setError(null);
      return;
    }
    let active = true;
    setLoading(true);
    setError(null);
    setText(null);
    fetch(url)
      .then(async (response) => {
        if (!response.ok) {
          let message = response.statusText;
          try {
            const body = (await response.json()) as { error?: string };
            if (body.error) message = body.error;
          } catch {
            // Keep the status text when the error body is not JSON.
          }
          throw new Error(message);
        }
        return response.text();
      })
      .then((value) => {
        if (active) {
          setText(value);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setError(err instanceof Error ? err.message : 'unknown');
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, [url, reloadKey]);

  return { text, loading, error };
}
