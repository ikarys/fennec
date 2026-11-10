/** Opens the project's folder in the human's terminal or linked IDE. */
import { useState } from "react";
import { openProjectTerminal } from "../../lib/api";
import { ExternalIcon, TerminalIcon } from "../ui/Icons";
import { Button, buttonClass } from "../ui/Button";
import { useOpenInEditorLabel } from "../../lib/editors";
import { useT } from "../../i18n";

export function ProjectOpenActions({
  projectId,
  editorUrl,
}: {
  projectId: string;
  editorUrl: string;
}) {
  const t = useT();
  const openInEditor = useOpenInEditorLabel();
  const [error, setError] = useState<string | null>(null);

  const handleOpenTerminal = () => {
    setError(null);
    openProjectTerminal(projectId).catch((failure) =>
      setError(failure instanceof Error ? failure.message : t("error")),
    );
  };

  return (
    <span className="flex items-center gap-1">
      <Button
        variant="ghost"
        size="sm"
        iconOnly
        onClick={handleOpenTerminal}
        title={t("openTerminalInProject")}
        aria-label={t("openTerminalInProject")}
      >
        <TerminalIcon className="h-4 w-4" />
      </Button>
      <a
        href={editorUrl}
        title={openInEditor}
        aria-label={openInEditor}
        className={buttonClass({
          variant: "ghost",
          size: "sm",
          iconOnly: true,
        })}
      >
        <ExternalIcon className="h-4 w-4" />
      </a>
      {error !== null ? (
        <span className="text-xs text-err">{error}</span>
      ) : null}
    </span>
  );
}
