import { useMemo } from 'react';
import { useT } from '../../i18n';

export function JsonView({ source }: { source: string }) {
  const t = useT();
  const formatted = useMemo(() => {
    try {
      return { ok: true as const, text: JSON.stringify(JSON.parse(source), null, 2) };
    } catch {
      return { ok: false as const, text: source };
    }
  }, [source]);

  return (
    <pre className="overflow-x-auto whitespace-pre font-mono text-xs text-text">
      {!formatted.ok ? <span className="block text-err">{t('jsonInvalid')}</span> : null}
      {formatted.text}
    </pre>
  );
}
