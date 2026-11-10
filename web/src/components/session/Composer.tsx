import type { WorkflowSummary } from "../../lib/workflow";
import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type {
  Agent,
  AttachmentInput,
  McpSessionView,
  ModelChoice,
  ModelChoices,
  ModelRef,
  ReasoningEffort,
  SlashCommand,
} from "../../lib/api";
import { IMAGE_TYPES } from "../../lib/attachments";
import {
  CloseIcon,
  PauseIcon,
  PaperclipIcon,
  SendIcon,
  StopIcon,
} from "../ui/Icons";
import { matchCommands, SlashMenu } from "./SlashMenu";
import { ModePicker } from "./ModePicker";
import { McpPicker } from "./McpPicker";
import { ModelPicker, modelShortName } from "./ModelPicker";
import { EffortPicker } from "./EffortPicker";
import { Button } from "../ui/Button";
import {
  WorkflowPicker,
  type WorkflowSelection,
} from "../workflow/session/WorkflowPicker";
import { useT } from "../../i18n";

interface ComposerProps {
  text: string;
  onTextChange: (text: string) => void;
  onSend: () => void;
  onStop: () => void;
  /** Set while a workflow run works: pause it at its next edge. */
  onPause?: () => void;
  pauseRequested?: boolean;
  running: boolean;
  currentModel: ModelChoice | null;
  modelChoices: ModelChoices | null;
  onModelChange?: (ref: ModelRef) => void;
  effort?: ReasoningEffort | null;
  acceptedEfforts?: ReasoningEffort[];
  onEffortChange?: (effort: ReasoningEffort | null) => void;
  error: string | null;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  placeholder?: string;
  commands?: SlashCommand[];
  agents?: Agent[];
  mode?: string | null;
  onModeChange?: (id: string) => void;
  attachments?: AttachmentInput[];
  onAttach?: (files: File[]) => void;
  onRemoveAttachment?: (index: number) => void;
  workflows?: WorkflowSummary[];
  workflowSelection?: WorkflowSelection;
  onWorkflowSelectionChange?: (selection: WorkflowSelection) => void;
  mcpView?: McpSessionView | null;
  onMcpToggle?: (name: string, active: boolean) => void;
}

export function Composer({
  text,
  onTextChange,
  onSend,
  onStop,
  onPause,
  pauseRequested,
  running,
  currentModel,
  modelChoices,
  onModelChange,
  effort,
  acceptedEfforts,
  onEffortChange,
  error,
  textareaRef,
  placeholder,
  commands,
  agents,
  mode,
  onModeChange,
  attachments,
  onAttach,
  onRemoveAttachment,
  workflows,
  workflowSelection,
  onWorkflowSelectionChange,
  mcpView,
  onMcpToggle,
}: ComposerProps) {
  const [selected, setSelected] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const t = useT();
  const matches = matchCommands(text, commands ?? []);
  const menuOpen = matches.length > 0 && !dismissed;
  const matchKey = matches.map((command) => command.name).join("\n");

  useEffect(() => {
    setDismissed(false);
  }, [text]);

  // The box grows with the text, up to its max height, then scrolls.
  useEffect(() => {
    const element = textareaRef.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [text, textareaRef]);

  useEffect(() => {
    setSelected(0);
  }, [matchKey]);

  function selectCommand(index: number) {
    const command = matches[index];
    if (command) {
      onTextChange(`/${command.name} `);
    }
  }

  const canSend = running
    ? text.trim().length > 0 && !(attachments?.length)
    : text.trim().length > 0 || (attachments?.length ?? 0) > 0;

  function handleFileChange(event: React.ChangeEvent<HTMLInputElement>) {
    const files = event.target.files;
    if (files && files.length > 0) {
      onAttach?.([...files]);
    }
    event.target.value = "";
  }

  function handlePaste(event: React.ClipboardEvent<HTMLTextAreaElement>) {
    if (!onAttach) return;
    const files = event.clipboardData?.files;
    if (files && files.length > 0) {
      event.preventDefault();
      onAttach([...files]);
    }
  }

  function handleDrop(event: React.DragEvent<HTMLDivElement>) {
    if (!onAttach) return;
    event.preventDefault();
    const files = event.dataTransfer?.files;
    if (files && files.length > 0) {
      onAttach([...files]);
    }
  }

  function handleDragOver(event: React.DragEvent<HTMLDivElement>) {
    if (!onAttach) return;
    event.preventDefault();
  }

  return (
    <div className="px-6 pt-2 pb-4">
      <div
        className="mx-auto max-w-page rounded-lg border border-border-strong bg-panel shadow-float transition-[border-color,box-shadow] focus-within:border-accent focus-within:ring-4 focus-within:ring-accent-soft"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        {menuOpen && (
          <SlashMenu
            commands={matches}
            selected={selected}
            onSelect={selectCommand}
          />
        )}
        <label
          htmlFor="prompt"
          className="absolute h-px w-px overflow-hidden clip-[rect(0_0_0_0)]"
        >
          {t("composerLabel")}
        </label>
        {attachments && attachments.length > 0 && (
          <div className="flex flex-wrap gap-2 px-3.5 pt-3">
            {attachments.map((a, i) => (
              <div key={i} className="relative h-14 w-14">
                <img
                  src={a.data}
                  alt={a.filename}
                  className="h-14 w-14 rounded-lg border border-border object-cover"
                />
                <button
                  type="button"
                  aria-label={t("removeAttachment", { name: a.filename })}
                  onClick={() => onRemoveAttachment?.(i)}
                  className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full border border-border-strong bg-bg text-muted hover:text-text"
                >
                  <CloseIcon className="h-2.5 w-2.5" />
                </button>
              </div>
            ))}
          </div>
        )}
        {/* The text is the primary surface; send sits beside it so the options below never move it. */}
        <div className="flex items-start gap-3 py-2.5 pr-2.5 pl-4">
          <textarea
            id="prompt"
            ref={textareaRef}
            rows={2}
            value={text}
            onChange={(event) => onTextChange(event.target.value)}
            onPaste={handlePaste}
            onKeyDown={(event) => {
              if (menuOpen) {
                if (event.key === "ArrowDown") {
                  event.preventDefault();
                  setSelected((current) =>
                    Math.min(current + 1, matches.length - 1),
                  );
                  return;
                }
                if (event.key === "ArrowUp") {
                  event.preventDefault();
                  setSelected((current) => Math.max(current - 1, 0));
                  return;
                }
                if (event.key === "Enter" || event.key === "Tab") {
                  event.preventDefault();
                  selectCommand(selected);
                  return;
                }
                if (event.key === "Escape") {
                  event.preventDefault();
                  setDismissed(true);
                  return;
                }
              }
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                onSend();
              }
            }}
            placeholder={
              placeholder ??
              t("composerPlaceholder")
            }
            className="max-h-60 min-w-0 grow resize-none bg-transparent py-1.5 leading-relaxed outline-none placeholder:text-faint"
          />
          {running && onPause && (
            <Button
              variant="secondary"
              size="lg"
              iconOnly
              aria-label={t("pause")}
              title={
                pauseRequested
                  ? t("pauseRequested")
                  : t("pauseAfterStep")
              }
              disabled={pauseRequested}
              onClick={onPause}
            >
              <PauseIcon className="h-4 w-4" />
            </Button>
          )}
          {running && (
            <Button
              variant="danger"
              size="lg"
              iconOnly
              aria-label={t("stop")}
              title={t("stopTurn")}
              onClick={onStop}
            >
              <StopIcon className="h-4 w-4" />
            </Button>
          )}
          <Button
            variant={running ? "secondary" : "primary"}
            size="lg"
            iconOnly
            aria-label={running ? t("queue") : t("send")}
            title={
              running
                ? t("queueTitle")
                : t("sendTitle")
            }
            onClick={onSend}
            disabled={!canSend}
          >
            <SendIcon className="h-[18px] w-[18px]" />
          </Button>
        </div>
        {/* Left: what will run. Right: what it runs on. */}
        <div className="flex flex-wrap items-center gap-1 rounded-b-2xl border-t border-border bg-bg/60 px-2 py-1.5">
          {onAttach && (
            <>
              <Button
                variant="ghost"
                iconOnly
                aria-label={t("attachImage")}
                title={t("attachImageTitle")}
                onClick={() => fileInputRef.current?.click()}
              >
                <PaperclipIcon className="h-3.5 w-3.5" />
              </Button>
              <input
                ref={fileInputRef}
                type="file"
                multiple
                accept={IMAGE_TYPES.join(",")}
                className="hidden"
                onChange={handleFileChange}
              />
            </>
          )}
          {agents && onModeChange && (
            <ModePicker
              agents={agents}
              mode={mode ?? null}
              onChange={onModeChange}
            />
          )}
          {workflows && onWorkflowSelectionChange && workflowSelection && (
            <WorkflowPicker
              workflows={workflows}
              selection={workflowSelection}
              onChange={onWorkflowSelectionChange}
            />
          )}
          {mcpView && onMcpToggle && (
            <McpPicker view={mcpView} onToggle={onMcpToggle} />
          )}
          <div className="ml-auto flex items-center gap-1.5 px-1.5 text-xs text-muted">
            <span className="h-1.5 w-1.5 rounded-full bg-ok" />
            {onEffortChange && (
              <EffortPicker
                effort={effort ?? null}
                accepted={acceptedEfforts ?? []}
                onChange={onEffortChange}
              />
            )}
            {onModelChange ? (
              <ModelPicker
                current={currentModel}
                choices={modelChoices}
                onChange={onModelChange}
              />
            ) : (
              <span>
                {currentModel ? modelShortName(currentModel.model) : "…"}
              </span>
            )}
          </div>
        </div>
        {error && <div className="px-3 pb-2 text-xs text-err">{error}</div>}
      </div>
    </div>
  );
}
