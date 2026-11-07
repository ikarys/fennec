// Project page indicators tab: verified figures drawn from recorded facts.
import { useEffect, useState, type ReactNode } from 'react';
import { Badge } from '../ui/Badge';
import { Section } from '../ui/Section';
import { Tile } from '../ui/Tile';
import {
  AlertIcon,
  BulbIcon,
  CoinIcon,
  CommitIcon,
  DecisionIcon,
  EyeIcon,
  FileIcon,
  ProjectIcon,
  ReviewIcon,
} from '../Icons';
import {
  getProjectIndicators,
  type IndicatorId,
  type IndicatorView,
} from '../../lib/api';
import { useT } from '../../i18n';

const ICONS: Record<IndicatorId, ReactNode> = {
  integrity: <CommitIcon />,
  sensitiveReview: <AlertIcon />,
  understanding: <BulbIcon />,
  architecture: <ProjectIcon />,
  security: <EyeIcon />,
  cost: <CoinIcon />,
  transmission: <FileIcon />,
  traceability: <DecisionIcon />,
  reviewLoad: <ReviewIcon />,
};

export function IndicatorsSection({ projectId }: { projectId: string }) {
  const t = useT();
  const [data, setData] = useState<IndicatorView[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setData(null);
    getProjectIndicators(projectId)
      .then((indicators) => {
        if (!cancelled) {
          setData(indicators);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : t('unknownError'));
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [projectId]);

  if (loading) return <p className="text-muted">{t('loading')}</p>;
  if (error) return <p className="text-err">{error}</p>;
  if (!data) return <p className="text-muted">{t('loading')}</p>;

  return (
    <Section
      title={t('indicatorsTitle')}
      description={t('indicatorsHint')}
      actions={
        <Badge tone="ok" appearance="pill" title={t('verifiedFromLog')}>
          {t('verified')}
        </Badge>
      }
    >
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-3">
        {data.map((indicator) => (
          <Tile
            key={indicator.id}
            icon={ICONS[indicator.id]}
            figure={indicator.figure}
            label={indicator.label}
            detail={indicator.detail}
            tone={indicator.tone}
            active={false}
          />
        ))}
      </div>
    </Section>
  );
}
