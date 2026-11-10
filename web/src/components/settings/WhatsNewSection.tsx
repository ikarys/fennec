// Latest changelog release shown in settings.
import { useEffect, useState } from 'react';
import { Section } from '../ui/Section';
import { Badge } from '../ui/Badge';
import { StarIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { getChangelog, markChangelogSeen, CHANGELOG_SEEN_EVENT, type Changelog } from '../../lib/api';

export function WhatsNewSection() {
  const t = useT();
  const [changelog, setChangelog] = useState<Changelog | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getChangelog()
      .then((data) => {
        if (cancelled) return;
        // The "New" badges keep the count read before marking seen: this visit still shows them.
        setChangelog(data);
        if (data.unseen > 0) {
          void markChangelogSeen()
            .then(() => {
              window.dispatchEvent(new Event(CHANGELOG_SEEN_EVENT));
            })
            .catch(() => {});
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setLoadError(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (loadError) {
    return <p className="text-xs text-danger">{loadError}</p>;
  }
  if (!changelog) {
    return null;
  }

  const newestRelease = changelog.releases[0];

  return (
    <Section Icon={StarIcon} title={t('whatsNewTitle')} description={t('whatsNewDescription')}>
      {newestRelease ? (
        <div className="flex flex-col gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-sm">{newestRelease.version}</span>
            {changelog.unseen > 0 && <Badge tone="accent">{t('whatsNewBadge')}</Badge>}
            {newestRelease.date && <span className="text-xs text-muted">{newestRelease.date}</span>}
          </div>
          {newestRelease.sections.map((section, sectionIndex) => (
            <div key={sectionIndex} className="flex flex-col gap-1">
              <span className="text-xs font-medium text-muted uppercase">{section.title}</span>
              <ul className="list-disc pl-5 text-sm">
                {section.items.map((item, itemIndex) => (
                  <li key={itemIndex}>{item}</li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      ) : (
        <p className="text-xs text-muted">{t('whatsNewEmpty')}</p>
      )}
    </Section>
  );
}
