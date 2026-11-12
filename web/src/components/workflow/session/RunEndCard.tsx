import { useState, type ReactNode } from "react";
import type { Lang, MsgKey } from "../../../i18n/messages";
import type { ThreadItem, WorkflowNodeStat } from "../../../lib/fold";
import { formatClockTime, formatDuration } from "../../../lib/format";
import {
  ActivityIcon,
  AlertIcon,
  CheckIcon,
  ChevronRightIcon,
  ClockIcon,
  CloseIcon,
  CpuIcon,
  FileIcon,
  PauseIcon,
  TokensIcon,
  WandIcon,
} from "../../ui/Icons";
import { Badge } from "../../ui/Badge";
import { Button } from "../../ui/Button";
import { Card } from "../../ui/Card";
import { Tile } from "../../ui/Tile";
import { useRunUsage } from "../../session/CallUsage";
import { useIntegrationInvite } from "../../session/IntegrationInvite";
import { TryOrIntegrate } from "../../session/TryOrIntegrate";
import { useLang, useT } from "../../../i18n";

type WorkflowEndItem = Extract<ThreadItem, { kind: "workflow-end" }>;

/** The outcome's form: a tinted header, a state icon, a word. */
function outcomeView(
  outcome: WorkflowEndItem["outcome"],
  t: (key: MsgKey, vars?: Record<string, string | number>) => string,
): { header: string; tone: string; icon: ReactNode; text: string } {
  switch (outcome) {
    case "done":
      return {
        header: "bg-ok-soft",
        tone: "text-ok",
        icon: <CheckIcon className="h-4 w-4" />,
        text: t("workflowDone"),
      };
    case "blocked":
      return {
        header: "bg-err-soft",
        tone: "text-err",
        icon: <AlertIcon className="h-4 w-4" />,
        text: t("workflowBlocked"),
      };
    case "stopped":
      return {
        header: "bg-raised",
        tone: "text-muted",
        icon: <PauseIcon className="h-4 w-4" />,
        text: t("workflowStopped"),
      };
    case "error":
      return {
        header: "bg-err-soft",
        tone: "text-err",
        icon: <CloseIcon className="h-4 w-4" />,
        text: t("workflowError"),
      };
  }
}

function NodeTrack({
  node,
  lang,
  axisMs,
  nodeTotalMs,
}: {
  node: WorkflowNodeStat;
  lang: Lang;
  axisMs: number;
  nodeTotalMs: number;
}) {
  // Older logs have no wall-clock timeline: fall back to the share bar.
  if (axisMs <= 0 || node.spans.length === 0) {
    return (
      <span className="relative h-1 w-full overflow-hidden rounded-full bg-faint/20">
        <span
          className="block h-full rounded-full bg-accent"
          style={{
            width:
              node.workingMs > 0 && nodeTotalMs > 0
                ? `${Math.max((node.workingMs / nodeTotalMs) * 100, 1)}%`
                : "0%",
          }}
        />
      </span>
    );
  }

  return (
    <span className="relative h-1 w-full overflow-hidden rounded-full bg-faint/20">
      {node.spans.map((span, index) => {
        // Keep the segment inside the track even if its duration overshoots the end.
        const left = Math.min(Math.max((span.offsetMs / axisMs) * 100, 0), 100);
        const width = Math.min(
          Math.max((span.durationMs / axisMs) * 100, 0.5),
          Math.max(0, 100 - left),
        );
        return (
          <span
            key={index}
            className="absolute h-full rounded-full bg-accent"
            title={formatDuration(lang, span.durationMs)}
            style={{ left: `${left}%`, width: `${width}%` }}
          />
        );
      })}
    </span>
  );
}

export function RunEndCard({
  item,
  latest,
}: {
  item: WorkflowEndItem;
  latest: boolean;
}) {
  const lang = useLang();
  const t = useT();
  const outcome = outcomeView(item.outcome, t);
  const stats = item.stats;
  const usage = useRunUsage(item.runId);
  const invite = useIntegrationInvite();
  // Most runs need only the outcome: the numbers wait behind an icon until asked for.
  const [showStats, setShowStats] = useState(false);

  const startAt = stats?.startedAt;
  const endAt = stats?.finishedAt;
  const totalMs = startAt != null && endAt != null ? endAt - startAt : null;

  // The criteria figure is the run's verdict: it takes a tone when it says one.
  const criteriaTone =
    stats && stats.criteriaTotal > 0
      ? stats.criteriaPassed === stats.criteriaTotal
        ? ("ok" as const)
        : ("err" as const)
      : ("neutral" as const);
  // Without criteria, 0/0 says nothing passed: say plainly that nothing was verified.
  const criteriaDetail = !stats
    ? undefined
    : stats.criteriaTotal === 0
      ? t("noCriteria")
      : t("runEndCriteriaDetail", {
          verified: stats.criteriaVerified,
          verifiedPlural: stats.criteriaVerified > 1 ? "s" : "",
          indicative: stats.criteriaTotal - stats.criteriaVerified,
          indicativePlural:
            stats.criteriaTotal - stats.criteriaVerified > 1 ? "s" : "",
        });

  const nodeTotalMs = stats
    ? stats.nodes.reduce((sum, node) => sum + node.workingMs, 0)
    : 0;
  // The timeline ends with the last pass: the wait for the human's final decision would
  // flatten the work into slivers; the header's start and end still show it.
  const axisMs = stats
    ? Math.max(
        0,
        ...stats.nodes.flatMap((node) =>
          node.spans.map((span) => span.offsetMs + span.durationMs),
        ),
      )
    : 0;

  return (
    <Card className="overflow-hidden">
      {/* The outcome first: a tinted header carrying the state, the time span and the provenance. */}
      <div className={`border-b border-border px-4 py-2.5 ${outcome.header}`}>
        <div className="flex items-center gap-2">
          <span
            className={`flex items-center gap-2 text-sm font-semibold ${outcome.tone}`}
          >
            {outcome.icon}
            {outcome.text}
          </span>
          {stats && stats.criteriaTotal === 0 ? (
            <Badge tone="neutral" title={t("noCriteria")}>
              {t("nothingVerified")}
            </Badge>
          ) : (
            <Badge tone="ok" title={t("verifiedFromLog")}>
              {t("verified")}
            </Badge>
          )}
          {stats && (
            <Button
              variant="icon"
              iconOnly
              className="ml-auto"
              title={t(showStats ? "runEndHideStats" : "runEndShowStats")}
              aria-pressed={showStats}
              onClick={() => setShowStats((shown) => !shown)}
            >
              <ActivityIcon />
            </Button>
          )}
        </div>
        {startAt != null && endAt != null && (
          <div
            className="mt-0.5 pl-6 text-xs text-muted tabular-nums"
            title={t("runEndDurationTitle")}
          >
            {t("runEndStartEnd", {
              start: formatClockTime(lang, startAt),
              end: formatClockTime(lang, endAt),
            })}
            {totalMs != null
              ? t("runEndDurationSuffix", {
                  duration: formatDuration(lang, totalMs),
                })
              : ""}
          </div>
        )}
      </div>

      {item.reason && (
        <div className="px-4 pt-2.5 text-sm text-muted">{item.reason}</div>
      )}

      {stats && showStats && (
        <div className="flex flex-col">
          <div className="grid grid-cols-2 gap-2 p-3 sm:grid-cols-3">
            <Tile
              size="sm"
              icon={<ClockIcon />}
              figure={formatDuration(lang, stats.workingMs)}
              label={t("runEndWorkingTime")}
              detail={
                totalMs != null
                  ? t("runEndOfDuration", {
                      duration: formatDuration(lang, totalMs),
                    })
                  : undefined
              }
            />
            <Tile
              size="sm"
              icon={<WandIcon />}
              figure={usage?.calls ?? "–"}
              label={t("runEndModelCalls")}
            />
            <Tile
              size="sm"
              icon={<TokensIcon />}
              figure={usage?.generated ?? "–"}
              label={t("runEndGeneratedTokens")}
            />
            <Tile
              size="sm"
              icon={<CpuIcon />}
              figure={usage?.read ?? "–"}
              label={t("runEndReadTokens")}
              detail={usage?.cache}
            />
            <Tile
              size="sm"
              icon={<FileIcon />}
              figure={stats.filesChanged}
              label={t("panelModifiedFiles")}
            />
            <Tile
              size="sm"
              tone={criteriaTone}
              active={criteriaTone !== "neutral"}
              icon={<CheckIcon />}
              figure={`${stats.criteriaPassed}/${stats.criteriaTotal}`}
              label={t("panelCriteria")}
              detail={criteriaDetail}
            />
          </div>

          {stats.nodes.length > 0 && (
            <details className="group border-t border-border">
              <summary className="flex cursor-pointer list-none items-center gap-2 px-4 py-2 text-xs text-muted select-none hover:text-text [&::-webkit-details-marker]:hidden">
                <ChevronRightIcon className="h-3 w-3 shrink-0 transition-transform group-open:rotate-90" />
                <span>{t("runEndTimeDetail")}</span>
                <span className="ml-auto tabular-nums text-faint">
                  {t("runEndNodesSummary", {
                    duration: formatDuration(lang, stats.workingMs),
                    count: stats.nodes.length,
                    plural: stats.nodes.length > 1 ? "s" : "",
                  })}
                </span>
              </summary>
              <div className="flex flex-col px-4 pb-3">
                {stats.nodes.map((node) => (
                  <div
                    key={node.nodeId}
                    className="grid grid-cols-[140px_1fr_72px] items-center gap-2.5 py-1 text-xs"
                  >
                    <span className="truncate text-text" title={node.label}>
                      {node.label}
                      {node.runs > 1 && (
                        <span className="ml-1.5 text-muted tabular-nums">
                          ×{node.runs}
                        </span>
                      )}
                    </span>
                    <NodeTrack
                      node={node}
                      lang={lang}
                      axisMs={axisMs}
                      nodeTotalMs={nodeTotalMs}
                    />
                    <span className="whitespace-nowrap text-right text-muted tabular-nums">
                      {formatDuration(lang, node.workingMs)}
                    </span>
                  </div>
                ))}
              </div>
            </details>
          )}
        </div>
      )}

      {latest && item.outcome === "done" && invite !== null && (
        <div className="flex items-center justify-between gap-3 border-t border-border bg-accent-soft px-4 py-2.5">
          <span className="text-xs text-muted">{t("runEndTryHint")}</span>
          <TryOrIntegrate invite={invite} primary="try" />
        </div>
      )}
    </Card>
  );
}
