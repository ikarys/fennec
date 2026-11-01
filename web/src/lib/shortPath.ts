/**
 * A long path shortened for a one-line card: the home as `~`, the middle
 * elided. The caller keeps the full path in a tooltip.
 */
export function shortPath(path: string): string {
  let display = path;
  const home = display.match(/^\/home\/[^/]+\/(.*)$/);
  if (home) {
    display = `~/${home[1]}`;
  }

  if (display.length <= 48) {
    return display;
  }

  const segments = display.split('/').filter((segment) => segment.length > 0);
  if (segments.length <= 3) {
    return display;
  }

  const first = display.startsWith('/') ? `/${segments[0]}` : segments[0];
  const lastTwo = segments.slice(-2).join('/');
  return `${first}/…/${lastTwo}`;
}
