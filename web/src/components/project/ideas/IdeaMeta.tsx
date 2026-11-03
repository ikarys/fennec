/**
 * The idea's run meta: duration, tokens, criteria, decisions, and git counts.
 */
import { formatDuration, formatTokens } from '../../../lib/format';
import type { IdeaBoard, IdeaView } from '../../../lib/ideas';
import { CheckIcon, ClockIcon, CommitIcon, DecisionIcon, FileIcon, TokensIcon } from '../../Icons';
import { Stat } from '../../ui/Stat';
import { useT, useLang } from '../../../i18n';

/**
 * The prop is the fields this component reads, so both board ideas and the
 * cross-project backlog summaries can be passed without casts.
 */
export function IdeaMeta({
  idea,
  caps,
}: {
  idea: Pick<
    IdeaView,
    'durationMs' | 'autonomous' | 'tokens' | 'criteria' | 'decisions' | 'commits' | 'files'
  >;
  caps: IdeaBoard['queue']['caps'];
}) {
  const t = useT();
  const lang = useLang();
  return (
    <>
      {idea.durationMs != null ? (
        <Stat
          icon={<ClockIcon />}
          title={
            idea.autonomous && caps.maxMinutes > 0
              ? t('durationOnCap', { duration: formatDuration(lang, idea.durationMs), max: caps.maxMinutes })
              : t('durationPlain', { duration: formatDuration(lang, idea.durationMs) })
          }
        >
          {formatDuration(lang, idea.durationMs)}
        </Stat>
      ) : null}
      {idea.tokens != null ? (
        <Stat
          icon={<TokensIcon />}
          title={
            idea.autonomous && caps.maxTokens > 0
              ? t('tokensOnCap', { tokens: formatTokens(lang, idea.tokens), max: formatTokens(lang, caps.maxTokens) })
              : t('tokensPlain', { tokens: formatTokens(lang, idea.tokens) })
          }
        >
          {formatTokens(lang, idea.tokens)}
        </Stat>
      ) : null}
      {idea.criteria && idea.criteria.passed + idea.criteria.failed + idea.criteria.pending > 0 ? (
        <Stat
          icon={<CheckIcon />}
          tone={
            idea.criteria.failed > 0
              ? 'err'
              : idea.criteria.passed > 0 && idea.criteria.pending === 0
                ? 'ok'
                : 'neutral'
          }
          title={t('criteriaTitle', {
            passed: idea.criteria.passed,
            failed: idea.criteria.failed,
            pending: idea.criteria.pending,
            s: idea.criteria.passed > 1 || idea.criteria.failed > 1 ? 's' : ''
          })}
        >
          {`${idea.criteria.passed}/${idea.criteria.passed + idea.criteria.failed + idea.criteria.pending}`}
        </Stat>
      ) : null}
      {idea.decisions.length > 0 ? (
        <Stat
          icon={<DecisionIcon />}
          title={`${t('decisionsTitle', { count: idea.decisions.length, s: idea.decisions.length > 1 ? 's' : '' })}\n${idea.decisions.map((d) => `${d.title} : ${d.choice}`).join('\n')}`}
        >
          {idea.decisions.length}
        </Stat>
      ) : null}
      {idea.commits != null ? (
        <>
          <Stat
            icon={<CommitIcon />}
            title={t('commitsCount', { count: idea.commits, s: idea.commits > 1 ? 's' : '' })}
          >
            {idea.commits}
          </Stat>
          {idea.files != null ? (
            <Stat
              icon={<FileIcon />}
              title={t('filesModified', { count: idea.files, s: idea.files > 1 ? 's' : '' })}
            >
              {idea.files}
            </Stat>
          ) : null}
        </>
      ) : null}
    </>
  );
}
