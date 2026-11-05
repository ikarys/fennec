import { useEffect, useRef, useState } from 'react';
import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { RefreshIcon, SpinnerIcon } from '../Icons';
import { useT } from '../../i18n';
import { getServer, setServerPort, restartServer, type ServerSettings } from '../../lib/api';

export function ServerSection() {
  const t = useT();
  const [view, setView] = useState<ServerSettings | null>(null);
  const [draft, setDraft] = useState('');
  const [loadError, setLoadError] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);
  const intervalRef = useRef<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    getServer()
      .then((result) => {
        if (!cancelled) {
          setView(result);
          setDraft(String(result.port));
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLoadError(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    return () => {
      if (intervalRef.current !== null) {
        window.clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, []);

  if (loadError) {
    return <p className="text-xs text-danger">{loadError}</p>;
  }

  if (!view) {
    return null;
  }

  const dirty = draft !== String(view.port);
  const save: SectionSave = {
    dirty,
    onSave: async () => {
      const next = await setServerPort(Number(draft));
      setView(next);
    },
  };

  const stopPolling = () => {
    if (intervalRef.current !== null) {
      window.clearInterval(intervalRef.current);
      intervalRef.current = null;
    }
  };

  const onRestart = async () => {
    if (!window.confirm(t('serverRestartConfirm'))) {
      return;
    }

    setRestarting(true);
    try {
      await restartServer();
    } catch (error) {
      setRestarting(false);
      window.alert(t('serverRestartFailed', { error: error instanceof Error ? error.message : String(error) }));
      return;
    }

    // The flag pins the process to its running port when present.
    const target = view.fromFlag ? view.runningPort : view.port;
    const servedByFennec = window.location.port === String(view.runningPort);
    let tries = 0;

    intervalRef.current = window.setInterval(async () => {
      tries += 1;
      try {
        if (servedByFennec && target !== view.runningPort) {
          await fetch(`${window.location.protocol}//${window.location.hostname}:${target}/api/version`, {
            mode: 'no-cors',
          });
          stopPolling();
          setRestarting(false);
          window.location.href = `${window.location.protocol}//${window.location.hostname}:${target}/${window.location.hash}`;
          return;
        }

        const response = await fetch('/api/version');
        // The old process may answer briefly after re-exec.
        if (response.ok && tries >= 2) {
          stopPolling();
          setRestarting(false);
          window.location.reload();
          return;
        }
      } catch {
        // The new server is not reachable yet.
      }

      if (tries >= 60) {
        stopPolling();
        setRestarting(false);
        window.alert(t('serverRestartTimeout'));
      }
    }, 1000);
  };

  return (
    <Section
      title={t('serverTitle')}
      description={t('serverDescription')}
      actions={
        <Button
          variant="ghost"
          size="sm"
          iconOnly
          title={t('serverRestartTitle')}
          aria-label={t('serverRestartTitle')}
          onClick={onRestart}
          disabled={restarting}
        >
          {restarting ? <SpinnerIcon className="h-4 w-4 animate-spin" /> : <RefreshIcon className="h-4 w-4" />}
        </Button>
      }
      save={save}
    >
      <FieldRow label={t('serverPortLabel')} hint={t('serverPortHint', { path: view.path })}>
        <TextInput
          type="number"
          min={1024}
          max={65535}
          className="w-32"
          value={draft}
          onChange={(event) => setDraft(event.currentTarget.value)}
        />
      </FieldRow>
      <div className="flex items-center gap-2 text-xs text-muted">
        {t('serverRunningOn')}
        <Badge tone="accent">{view.runningPort}</Badge>
        {view.port !== view.runningPort ? <Badge tone="attention">{t('serverRestartNeeded')}</Badge> : null}
      </div>
      {view.fromFlag ? <p className="m-0 text-xs text-faint">{t('serverFromFlag')}</p> : null}
    </Section>
  );
}
