// Incoming contributions: branches someone else wrote, adopted into a session so they go through the same integration gate as an agent's work.
import { useEffect, useState } from 'react';
import { adoptContribution, getContributions, type Contribution } from '../../lib/api';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Section } from '../ui/Section';
import { CommitIcon, ReviewIcon } from '../ui/Icons';
import { useT } from '../../i18n';

export function ContributionsSection({
  projectId,
  onOpenSession,
}: {
  projectId: string;
  onOpenSession: (id: string) => void;
}) {
  const t = useT();
  const [contributions, setContributions] = useState<Contribution[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyBranch, setBusyBranch] = useState<string | null>(null);
  const [reviewError, setReviewError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setReviewError(null);
    setBusyBranch(null);

    getContributions(projectId)
      .then((items) => {
        if (cancelled) return;
        setContributions(items);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : t('unknownError'));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [projectId]);

  const review = async (branch: string) => {
    setBusyBranch(branch);
    setReviewError(null);
    try {
      const session = await adoptContribution(projectId, branch);
      onOpenSession(session.id);
    } catch (err) {
      setReviewError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setBusyBranch(null);
    }
  };

  return (
    <Section Icon={CommitIcon} title={t('contributionsTitle')} description={t('contributionsDescription')} flush>
      {loading ? (
        <p className='px-4 py-3 text-xs text-muted'>{t('loading')}</p>
      ) : error ? (
        <p className='px-4 py-3 text-xs text-err'>{error}</p>
      ) : contributions.length === 0 ? (
        <p className='px-4 py-3 text-xs text-muted'>{t('contributionsEmpty')}</p>
      ) : (
        <ul className='divide-y divide-border'>
          {contributions.map((c) => (
            <li
              key={c.branch}
              onClick={() => {
                if (busyBranch) return;
                void review(c.branch);
              }}
              className={`group relative flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors before:absolute before:inset-y-0 before:left-0 before:w-0.5 before:bg-accent before:transition-opacity hover:bg-accent-soft hover:before:opacity-100 ${
                busyBranch === c.branch ? 'bg-accent-soft before:opacity-100' : 'before:opacity-0'
              }`}
            >
              <span className='min-w-0 grow'>
                <span className='block truncate font-mono text-sm text-text group-hover:text-accent-strong' title={c.branch}>
                  {c.branch}
                </span>
                <span className='block truncate text-xs text-muted'>{c.subject}</span>
              </span>
              <span className='flex w-24 shrink-0 items-center justify-end'>
                <Badge tone='info'>{t('contributionsAhead', { count: c.ahead })}</Badge>
              </span>
              <span className='flex w-10 shrink-0 items-center justify-end'>
                <Button
                  variant='soft'
                  size='sm'
                  iconOnly
                  title={t('contributionsReview', { branch: c.branch })}
                  aria-label={t('contributionsReview', { branch: c.branch })}
                  disabled={busyBranch !== null}
                  onClick={(event) => {
                    event.stopPropagation();
                    if (busyBranch) return;
                    void review(c.branch);
                  }}
                >
                  <ReviewIcon className='h-3.5 w-3.5' />
                </Button>
              </span>
            </li>
          ))}
        </ul>
      )}
      {reviewError ? <p className='text-xs text-err'>{reviewError}</p> : null}
    </Section>
  );
}
