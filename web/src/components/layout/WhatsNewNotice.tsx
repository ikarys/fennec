import { useEffect, useState } from 'react';
import { useT } from '../../i18n';
import { StarIcon } from '../ui/Icons';
import { Badge } from '../ui/Badge';
import { buttonClass } from '../ui/Button';
import { getChangelog, CHANGELOG_SEEN_EVENT } from '../../lib/api';

export function WhatsNewNotice() {
  const t = useT();
  const [unseen, setUnseen] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const changelog = await getChangelog();
        if (!cancelled) {
          setUnseen(changelog.unseen);
        }
      } catch {
        if (!cancelled) {
          setUnseen(null);
        }
      }
    };
    const onFocus = () => {
      void load();
    };
    const onSeen = () => {
      setUnseen(0);
    };

    void load();
    window.addEventListener('focus', onFocus);
    window.addEventListener(CHANGELOG_SEEN_EVENT, onSeen);

    return () => {
      cancelled = true;
      window.removeEventListener('focus', onFocus);
      window.removeEventListener(CHANGELOG_SEEN_EVENT, onSeen);
    };
  }, []);

  if (unseen === null || unseen === 0) {
    return null;
  }

  return (
    <a
      href="#/settings/about"
      className={buttonClass({ variant: 'ghost', size: 'sm' }, 'text-accent')}
      title={t('whatsNewNoticeTitle', { count: String(unseen) })}
      aria-label={t('whatsNewNoticeTitle', { count: String(unseen) })}
    >
      <span className="flex items-center gap-1">
        <StarIcon className="h-4 w-4" />
        <Badge tone="accent">{unseen}</Badge>
      </span>
    </a>
  );
}
