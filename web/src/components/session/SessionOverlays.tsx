// The panes and dialogs a session opens over its page: integration review, preview and canvas.
import type { ThreadView } from '../../lib/fold';
import { CanvasPane } from '../canvas/CanvasPane';
import { PreviewPane } from '../preview/PreviewPane';
import type { PreviewTarget } from '../preview/PreviewContext';
import { IntegrationDialog } from './IntegrationDialog';

interface SessionOverlaysProps {
  sessionId: string | null;
  projectId: string | null;
  view: ThreadView;
  integrationOpen: boolean;
  onCloseIntegration: () => void;
  preview: PreviewTarget | null;
  onClosePreview: () => void;
  canvasId: number | null;
  onCloseCanvas: () => void;
  /** A note written on the canvas goes to the composer or the run's comment. */
  onAnnotate: (message: string) => void;
}

export function SessionOverlays({
  sessionId,
  projectId,
  view,
  integrationOpen,
  onCloseIntegration,
  preview,
  onClosePreview,
  canvasId,
  onCloseCanvas,
  onAnnotate,
}: SessionOverlaysProps) {
  return (
    <>
      {sessionId && integrationOpen && (
        <IntegrationDialog
          sessionId={sessionId}
          projectId={projectId}
          open
          onClose={onCloseIntegration}
        />
      )}
      {preview && sessionId && (
        <PreviewPane
          sessionId={sessionId}
          target={preview}
          onClose={onClosePreview}
        />
      )}
      {canvasId != null && (
        <CanvasPane
          artifactId={canvasId}
          refreshKey={view}
          onClose={onCloseCanvas}
          onAnnotate={onAnnotate}
        />
      )}
    </>
  );
}
