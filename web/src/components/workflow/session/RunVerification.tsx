/** What fennec verified for the latest run: criteria with provenance, tests protected, waivers, files outside the plan. */
import { createContext, useContext } from "react";
import type { CheckedCriterion, Verification } from "../../../lib/api";
import type { MsgKey } from "../../../i18n/messages";
import { useT } from "../../../i18n";
import { Badge } from "../../ui/Badge";
import { SectionLabel } from "../../ui/SectionLabel";
import { CheckIcon, CloseIcon } from "../../ui/Icons";
import { CopyButton } from "../../session/CopyButton";

export const VerificationContext = createContext<Verification | null>(null);

export function useRunVerification(runId: string): Verification | null {
  const verification = useContext(VerificationContext);
  return verification && verification.runId === runId ? verification : null;
}

function criterionBadge(
  criterion: CheckedCriterion,
  t: (key: MsgKey, vars?: Record<string, string | number>) => string,
) {
  const outcome = criterion.outcome;

  if (outcome.kind === "verified") {
    const title = `${outcome.command} · ${t("runVerificationExit", {
      code: outcome.exitCode ?? "–",
    })}`;
    return outcome.passed ? (
      <Badge tone="ok" appearance="pill" title={title}>
        {t("verified")}
      </Badge>
    ) : (
      <Badge tone="err" appearance="pill" title={title}>
        {t("runVerificationFailed")}
      </Badge>
    );
  }

  if (outcome.kind === "judged") {
    return (
      <Badge tone="attention" appearance="pill" title={t("indicativeTitle")}>
        {outcome.passed ? (
          <CheckIcon className="h-3 w-3 text-ok" />
        ) : (
          <CloseIcon className="h-3 w-3 text-err" />
        )}
        {t("indicative")}
      </Badge>
    );
  }

  return (
    <Badge tone="neutral" appearance="pill">
      {t("runVerificationPending")}
    </Badge>
  );
}

export function RunVerification({ verification }: { verification: Verification }) {
  const t = useT();

  if (
    verification.criteria.length === 0 &&
    verification.testsProtected === 0 &&
    verification.waiversAsked === 0
  ) {
    return null;
  }

  return (
    <div className="border-t border-border px-4 py-2.5">
      <div className="flex items-center gap-2">
        <SectionLabel as="span">{t("runVerificationTitle")}</SectionLabel>
        {verification.block != null && <CopyButton text={verification.block} />}
      </div>

      {verification.criteria.length > 0 && (
        <div className="mt-2 flex flex-col gap-1.5">
          {verification.criteria.map((criterion) => (
            <div
              key={criterion.id}
              className="grid grid-cols-[auto_1fr_auto] items-start gap-x-2 gap-y-0.5"
            >
              <span className="font-mono text-[0.6875rem] text-muted">
                {criterion.id}
              </span>
              <div className="min-w-0">
                <div className="text-xs text-text">{criterion.description}</div>
                {criterion.outcome.kind === "verified" && (
                  <div
                    className="truncate font-mono text-[0.6875rem] text-muted"
                    title={criterion.outcome.command}
                  >
                    {criterion.outcome.command}
                  </div>
                )}
              </div>
              {criterionBadge(criterion, t)}
            </div>
          ))}
        </div>
      )}

      <div className="mt-2 text-xs text-muted">
        {t("runVerificationTests", { count: verification.testsProtected })}
        <span className="mx-1">·</span>
        {verification.waiversAsked === 0
          ? t("runVerificationNoWaivers")
          : t("runVerificationWaivers", {
              asked: verification.waiversAsked,
              granted: verification.waiversGranted,
            })}
        {verification.outsidePlan != null && (
          <>
            <span className="mx-1">·</span>
            {verification.outsidePlan.length === 0 ? (
              <span className="text-ok">{t("runVerificationOutsideNone")}</span>
            ) : (
              <span>
                <span className="text-attention">
                  {t("runVerificationOutside", {
                    count: verification.outsidePlan.length,
                  })}
                </span>{" "}
                <span className="font-mono break-words text-text">
                  {verification.outsidePlan.join(", ")}
                </span>
              </span>
            )}
          </>
        )}
      </div>
    </div>
  );
}
