import type { AssistantItem, ThreadItem } from './fold';

export type ThreadFilter = 'all' | 'mine' | 'tools' | 'thinking';

/** What the thread shows under each filter; display only, the log is untouched. */
export function filterThread(items: ThreadItem[], filter: ThreadFilter): ThreadItem[] {
  if (filter === 'all') return items;
  if (filter === 'mine') {
    return items.filter((item) => item.kind === 'user' && !item.isSystemGenerated);
  }
  const kind = filter === 'tools' ? 'tool' : 'thinking';
  const assistantOnly = (item: AssistantItem): ThreadItem[] => {
    const segments = item.segments.filter((segment) => segment.kind === kind);
    return segments.length === 0 ? [] : [{ ...item, segments }];
  };
  return items.flatMap<ThreadItem>((item) => (item.kind === 'assistant' ? assistantOnly(item) : []));
}
