import { useEffect, useId, useRef, useState } from 'react';
import { useT } from '../../i18n';

type MermaidModule = typeof import('mermaid');
let mermaidReady: Promise<MermaidModule['default']> | null = null;

// Mermaid is heavy; load it only when a diagram is actually rendered.
function loadMermaid(): Promise<MermaidModule['default']> {
  if (!mermaidReady) {
    mermaidReady = import('mermaid').then(({ default: mermaid }) => {
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: 'strict',
        theme: 'default',
        // The image is drawn apart from the page: a common font keeps the measured labels fitting.
        fontFamily: 'arial, sans-serif',
        htmlLabels: false,
        flowchart: { htmlLabels: false },
      });
      return mermaid;
    });
  }
  return mermaidReady;
}

/** The diagram's natural width: mermaid's SVG says width="100%", which an image in a shrinking box turns into 0. */
function naturalWidth(svg: string): number | undefined {
  const width = Number(/viewBox="[-\d.]+ [-\d.]+ ([\d.]+) /.exec(svg)?.[1]);
  return width > 0 ? width : undefined;
}

export function MermaidDiagram({ source }: { source: string }) {
  const t = useT();
  const idBase = useId().replace(/[^a-zA-Z0-9-]/g, '');
  const counterRef = useRef(0);
  const [state, setState] = useState<
    { status: 'loading' } | { status: 'ready'; svg: string } | { status: 'error'; message: string }
  >({ status: 'loading' });

  useEffect(() => {
    let active = true;
    counterRef.current += 1;
    const id = `mermaid-${idBase}-${counterRef.current}`;
    setState({ status: 'loading' });
    loadMermaid()
      .then(async (mermaid) => {
        // parse throws on a syntax error before render would leave its error element in the page.
        await mermaid.parse(source);
        const { svg } = await mermaid.render(id, source);
        if (active) setState({ status: 'ready', svg });
      })
      .catch((err: unknown) => {
        if (active) setState({ status: 'error', message: err instanceof Error ? err.message : 'unknown' });
      });
    return () => {
      active = false;
    };
  }, [source, idBase]);

  if (state.status === 'loading') {
    return <div className="text-sm text-muted">{t('loading')}</div>;
  }

  if (state.status === 'error') {
    return (
      <div className="text-sm text-err">
        <div>{t('mermaidInvalid')}: {state.message}</div>
        <pre className="overflow-x-auto bg-code p-3 font-mono text-xs text-muted">{source}</pre>
      </div>
    );
  }

  return (
    // An image never runs scripts, so a model-written diagram cannot act with fennec's origin; the white card keeps the default theme readable in dark themes.
    <div className="inline-block rounded-md bg-white p-4">
      <img
        src={'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(state.svg)}
        alt={t('preview')}
        width={naturalWidth(state.svg)}
        className="max-w-full"
      />
    </div>
  );
}
