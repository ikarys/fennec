export type PreviewKind = 'html' | 'markdown';

/** The kind of a file path or a fenced block's language, undefined when it has no preview. */
export function previewKindOfPath(path: string): PreviewKind | undefined {
  const dot = path.lastIndexOf('.');
  if (dot === -1) return undefined;
  const extension = path.slice(dot + 1).toLowerCase();
  if (extension === 'html' || extension === 'htm') return 'html';
  if (extension === 'md' || extension === 'markdown') return 'markdown';
  return undefined;
}

export function previewKindOfLanguage(lang: string | undefined): PreviewKind | undefined {
  if (!lang) return undefined;
  const normalized = lang.trim().toLowerCase();
  if (normalized === 'html') return 'html';
  if (normalized === 'markdown' || normalized === 'md') return 'markdown';
  return undefined;
}

function cleanRelative(relative: string): string | undefined {
  const segments = relative.split('/');
  const kept: string[] = [];
  for (const segment of segments) {
    if (segment === '' || segment === '.') continue;
    if (segment === '..') return undefined;
    kept.push(segment);
  }
  return kept.length > 0 ? kept.join('/') : undefined;
}

/** The path relative to the project's workdir, or undefined when it lies outside. Accepts absolute paths and relative ones (kept as they are, `./` removed). */
export function relativeToWorkdir(path: string, workdir: string): string | undefined {
  const normalizedWorkdir = workdir.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!normalizedWorkdir) return undefined;
  const normalizedPath = path.replace(/\\/g, '/');
  const isAbsolute = normalizedPath.startsWith('/') || /^[A-Za-z]:/.test(normalizedPath);
  if (isAbsolute) {
    if (!normalizedPath.startsWith(normalizedWorkdir + '/')) return undefined;
    return cleanRelative(normalizedPath.slice(normalizedWorkdir.length + 1));
  }
  return cleanRelative(normalizedPath);
}

/** The preview route of a project-relative path, each segment URI-encoded. */
export function previewUrl(sessionId: string, relativePath: string): string {
  const segments = relativePath.split('/').filter((segment) => segment.length > 0);
  const encoded = segments.map(encodeURIComponent).join('/');
  return `/api/sessions/${encodeURIComponent(sessionId)}/preview/${encoded}`;
}
