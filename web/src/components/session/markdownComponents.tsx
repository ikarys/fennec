import type { ReactNode } from 'react';
import { CodeBlockWithPreview } from '../preview/CodeBlockWithPreview';
import { previewKindOfLanguage, type BlockPreviewKind } from '../../lib/preview';

interface HastText {
  type: 'text';
  value: string;
}

interface HastElement {
  type: 'element';
  properties?: {
    className?: string | string[];
  };
  children?: unknown[];
}

function isHastText(node: unknown): node is HastText {
  return typeof node === 'object' && node !== null && (node as HastText).type === 'text';
}

function isHastElement(node: unknown): node is HastElement {
  return typeof node === 'object' && node !== null && (node as HastElement).type === 'element';
}

function previewFromPre(node: HastElement | undefined): { kind: BlockPreviewKind; source: string } | undefined {
  if (!node) return undefined;
  const child = node.children?.[0];
  if (!isHastElement(child)) return undefined;
  const rawClasses = child.properties?.className;
  const classes = Array.isArray(rawClasses) ? rawClasses : rawClasses ? [rawClasses] : [];
  const language = classes.find((value) => value.startsWith('language-'))?.slice('language-'.length);
  const kind = previewKindOfLanguage(language);
  if (!kind) return undefined;
  const source = (child.children ?? [])
    .filter(isHastText)
    .map((text) => text.value)
    .join('');
  return { kind, source };
}

export const markdownComponents = {
  pre: ({ children, node }: { children?: ReactNode; node?: HastElement }) => {
    const preview = previewFromPre(node);
    if (preview) {
      return <CodeBlockWithPreview kind={preview.kind} source={preview.source} />;
    }
    return (
      <pre className="my-2 overflow-x-auto rounded-md bg-code p-3 font-mono text-xs text-muted">{children}</pre>
    );
  },
  code: ({ className, children }: { className?: string; children?: ReactNode }) => {
    const isBlock = className?.includes('language-');
    if (isBlock) {
      return <code className="font-mono text-xs">{children}</code>;
    }
    return (
      <code className="rounded bg-raised px-1 py-0.5 font-mono text-xs text-text">{children}</code>
    );
  },
  // Tailwind's preflight unstyles headings; previewed documents rely on them.
  h1: ({ children }: { children?: ReactNode }) => <h1 className="mt-4 mb-2 text-xl font-semibold">{children}</h1>,
  h2: ({ children }: { children?: ReactNode }) => <h2 className="mt-4 mb-2 text-lg font-semibold">{children}</h2>,
  h3: ({ children }: { children?: ReactNode }) => <h3 className="mt-3 mb-1.5 text-base font-semibold">{children}</h3>,
  p: ({ children }: { children?: ReactNode }) => <p className="my-2 leading-relaxed">{children}</p>,
  // Tailwind's preflight removes list markers; plans are lists of lists.
  ul: ({ children }: { children?: ReactNode }) => <ul className="my-2 list-disc pl-5">{children}</ul>,
  ol: ({ children }: { children?: ReactNode }) => <ol className="my-2 list-decimal pl-5">{children}</ol>,
  li: ({ children }: { children?: ReactNode }) => <li className="my-0.5 leading-relaxed">{children}</li>,
  a: ({ children, href }: { children?: ReactNode; href?: string }) => (
    <a href={href} className="text-accent-strong underline" target="_blank" rel="noreferrer">
      {children}
    </a>
  ),
  table: ({ children }: { children?: ReactNode }) => (
    // Wide tables scroll inside the thread instead of being cut by it.
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-sm">{children}</table>
    </div>
  ),
  th: ({ children }: { children?: ReactNode }) => (
    <th className="border border-border bg-raised px-2 py-1 text-left">{children}</th>
  ),
  td: ({ children }: { children?: ReactNode }) => <td className="border border-border px-2 py-1">{children}</td>
};
