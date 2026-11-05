import { useEffect, useRef, useState } from 'react';
import { useT } from '../i18n';
import type { ModelChoice, ModelChoices, ModelRef } from '../lib/api';
import { getModelChoices, setSessionModel, setSessionMode } from '../lib/api';

interface UseModeAndModelArgs {
  sessionId: string | null;
  /** The last `mode.changed` value from the event stream, or null. */
  viewMode: string | null;
  /** The server's resolved mode from the session detail, or null. */
  detailMode: string | null;
  /** A new-session tab has no detail yet: the server lists its default agent first. */
  fallbackMode: string | null;
  /** The session's model resolved by the server, or null. */
  detailModel: ModelChoice | null;
  /** The page's send-error state: selection failures are shown there. */
  onError: (message: string | null) => void;
  /** A model change was applied: the page re-reads the session detail. */
  onModelApplied: () => void;
}

/**
 * The session's mode and model: optimistic while the request is in flight,
 * reconciled against the server's state once it catches up.
 */
export function useModeAndModel({
  sessionId,
  viewMode,
  detailMode,
  fallbackMode,
  detailModel,
  onError,
  onModelApplied,
}: UseModeAndModelArgs) {
  const t = useT();
  // Optimistic mode while `setSessionMode` is in flight; cleared once the
  // server's mode (event stream or detail) matches it.
  const [pendingMode, setPendingMode] = useState<string | null>(null);
  const [modelChoices, setModelChoices] = useState<ModelChoices | null>(null);
  const [pendingModel, setPendingModel] = useState<ModelRef | null>(null);
  const modelRequestRef = useRef(0);
  const modeRequestRef = useRef(0);

  useEffect(() => {
    let active = true;
    getModelChoices()
      .then((data) => {
        if (active) setModelChoices(data);
      })
      .catch(() => {
        // The composer falls back to the session's model or '…'.
      });
    return () => {
      active = false;
    };
  }, []);

  // A pending choice belongs to one session; do not leak it across tabs.
  useEffect(() => {
    setPendingMode(null);
    setPendingModel(null);
  }, [sessionId]);

  const serverMode = viewMode ?? detailMode ?? fallbackMode ?? null;
  const mode = pendingMode ?? serverMode;

  useEffect(() => {
    // The pending value is only for the request gap; the server's mode is the
    // source of truth once it catches up.
    if (pendingMode && serverMode === pendingMode) {
      setPendingMode(null);
    }
  }, [pendingMode, serverMode]);

  const serverModel = detailModel ?? modelChoices?.default ?? null;
  const currentModel: ModelChoice | null = pendingModel
    ? {
        provider: pendingModel.provider,
        model: pendingModel.model,
        providerName:
          modelChoices?.favorites.find((favorite) =>
            sameModelRef(favorite, pendingModel),
          )?.providerName ??
          (modelChoices && sameModelRef(modelChoices.default, pendingModel)
            ? modelChoices.default.providerName
            : pendingModel.provider),
      }
    : serverModel;

  useEffect(() => {
    if (
      pendingModel &&
      serverModel &&
      serverModel.provider === pendingModel.provider &&
      serverModel.model === pendingModel.model
    ) {
      setPendingModel(null);
    }
  }, [pendingModel, serverModel]);

  async function handleModeChange(id: string): Promise<boolean> {
    // A not-yet-created session keeps the choice locally; `handleSend` passes
    // it to `createSession`.
    if (!sessionId) {
      setPendingMode(id);
      return true;
    }

    const request = ++modeRequestRef.current;
    setPendingMode(id);
    onError(null);

    try {
      await setSessionMode(sessionId, id);
      return true;
    } catch (error) {
      // A newer request supersedes this one; only the latest may revert.
      if (request !== modeRequestRef.current) return true;
      setPendingMode(null);
      onError(
        error instanceof Error
          ? error.message
          : t('modeChangeError'),
      );
      return false;
    }
  }

  async function handleModelChange(ref: ModelRef): Promise<boolean> {
    // A not-yet-created session keeps the choice locally; `handleSend` passes
    // it to `createSession`.
    if (!sessionId) {
      setPendingModel(ref);
      return true;
    }

    const request = ++modelRequestRef.current;
    setPendingModel(ref);
    onError(null);

    try {
      await setSessionModel(sessionId, ref);
      onModelApplied();
      setPendingModel(null);
      return true;
    } catch (error) {
      // A newer request supersedes this one; only the latest may revert.
      if (request !== modelRequestRef.current) return true;
      setPendingModel(null);
      onError(
        error instanceof Error
          ? error.message
          : t('modelChangeError'),
      );
      return false;
    }
  }

  return {
    modelChoices,
    mode,
    currentModel,
    pendingModel,
    handleModeChange,
    handleModelChange,
  };
}

function sameModelRef(a: ModelRef, b: ModelRef): boolean {
  return a.provider === b.provider && a.model === b.model;
}
