import { useEffect, useState } from "react";
import { Badge, type BadgeTone } from "../ui/Badge";
import { Button, buttonClass } from "../ui/Button";
import { ListCard } from "../ui/ListCard";
import {
  getSetup,
  type Setup,
  type SetupStatus,
  type SetupCheckId,
} from "../../lib/api/setup";
import { useT } from "../../i18n";

const TONES: Record<SetupStatus, BadgeTone> = {
  ok: "ok",
  todo: "attention",
  warn: "info",
};
const STATUS_KEYS = {
  ok: "setupStatusOk",
  todo: "setupStatusTodo",
  warn: "setupStatusWarn",
} as const satisfies Record<SetupStatus, string>;
const LABEL_KEYS = {
  model: "setupModel",
  sandbox: "setupSandbox",
  git: "setupGit",
  project: "setupProject",
  scanners: "setupScanners",
} as const satisfies Record<SetupCheckId, string>;

/**
 * The first-run checklist on the home page, shown while a required check is
 * missing (the server decides); it disappears by itself once everything
 * required is in place; each line says what is missing and links where to
 * fix it.
 */
export function SetupChecklist({
  projectCount,
  onOpenProject,
}: {
  projectCount: number;
  onOpenProject: () => void;
}) {
  const t = useT();
  const [setup, setSetup] = useState<Setup | null>(null);

  useEffect(() => {
    let cancelled = false;
    getSetup()
      .then((result) => {
        if (!cancelled) setSetup(result);
      })
      .catch(() => {
        // The checklist is a hint; ignore failures.
      });
    return () => {
      cancelled = true;
    };
  }, [projectCount]);

  if (!setup || setup.complete) return null;

  const done = setup.checks.filter((check) => check.status === "ok").length;

  return (
    <ListCard
      title={t("setupTitle")}
      description={t("setupDescription")}
      count={`${done} / ${setup.checks.length}`}
    >
      <ul className="m-0 list-none divide-y divide-border p-0">
        {setup.checks.map((check) => (
          <li key={check.id} className="flex items-center gap-3 px-4 py-2.5">
            <Badge tone={TONES[check.status]} pill>
              {t(STATUS_KEYS[check.status])}
            </Badge>
            <div className="min-w-0 grow">
              <div className="text-sm text-text">{t(LABEL_KEYS[check.id])}</div>
              {check.detail && (
                <div
                  className="truncate text-xs text-muted"
                  title={check.detail}
                >
                  {check.detail}
                </div>
              )}
            </div>
            {check.status !== "ok" &&
              (check.id === "project" ? (
                <Button variant="secondary" size="sm" onClick={onOpenProject}>
                  {t("openProject")}
                </Button>
              ) : check.id === "git" ? null : (
                <a
                  className={buttonClass({ variant: "ghost", size: "sm" })}
                  href={
                    check.id === "model"
                      ? "#/settings/models"
                      : "#/settings/security"
                  }
                >
                  {t("setupFix")}
                </a>
              ))}
          </li>
        ))}
      </ul>
    </ListCard>
  );
}
