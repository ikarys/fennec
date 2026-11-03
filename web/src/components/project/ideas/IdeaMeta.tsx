/**
 * The idea's run meta: duration, tokens, criteria, decisions, and git counts.
 */
import { formatDuration, formatTokens } from '../../../lib/format';
import type { IdeaBoard, IdeaView } from '../../../lib/ideas';
import { useLang } from '../../../i18n';
import { CheckIcon, ClockIcon, CommitIcon, DecisionIcon, FileIcon, TokensIcon } from '../../Icons';
import { Stat } from '../../ui/Stat';

// French run labels keep the singular for both 0 and 1.
function frSingularOrPlural(count: number, singular: string): string {
  return count === 0 || count === 1 ? singular : `${singular}s`;
}

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
  const lang = useLang();
  return (
    <>
      {idea.durationMs != null ? (
        <Stat
          icon={<ClockIcon />}
          title={
            idea.autonomous && caps.maxMinutes > 0
              ? `Durée ${formatDuration(lang, idea.durationMs)} sur un plafond de ${caps.maxMinutes} min (mode nuit)`
              : `Durée ${formatDuration(lang, idea.durationMs)}`
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
              ? `${formatTokens(lang, idea.tokens)} tokens sur un plafond de ${formatTokens(lang, caps.maxTokens)}`
              : `${formatTokens(lang, idea.tokens)} tokens`
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
          title={`Critères : ${idea.criteria.passed} ${frSingularOrPlural(idea.criteria.passed, 'validé')} · ${idea.criteria.failed} ${frSingularOrPlural(idea.criteria.failed, 'échoué')} · ${idea.criteria.pending} en attente`}
        >
          {`${idea.criteria.passed}/${idea.criteria.passed + idea.criteria.failed + idea.criteria.pending}`}
        </Stat>
      ) : null}
      {idea.decisions.length > 0 ? (
        <Stat
          icon={<DecisionIcon />}
          title={`${idea.decisions.length} ${frSingularOrPlural(idea.decisions.length, 'décision')}\n${idea.decisions.map((d) => `${d.title} : ${d.choice}`).join('\n')}`}
        >
          {idea.decisions.length}
        </Stat>
      ) : null}
      {idea.commits != null ? (
        <>
          <Stat
            icon={<CommitIcon />}
            title={`${idea.commits} ${frSingularOrPlural(idea.commits, 'commit')}`}
          >
            {idea.commits}
          </Stat>
          {idea.files != null ? (
            <Stat
              icon={<FileIcon />}
              title={`${idea.files} ${frSingularOrPlural(idea.files, 'fichier')} ${frSingularOrPlural(idea.files, 'modifié')}`}
            >
              {idea.files}
            </Stat>
          ) : null}
        </>
      ) : null}
    </>
  );
}
