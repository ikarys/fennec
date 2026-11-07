import { useEffect, useRef, useState } from 'react';
import { Section, type SectionSave } from '../ui/Section';
import { FieldRow, TextInput } from '../ui/Field';
import { Button } from '../ui/Button';
import { Badge } from '../ui/Badge';
import { Switch } from '../ui/Switch';
import { CheckIcon, RefreshIcon, SpinnerIcon } from '../ui/Icons';
import { useT } from '../../i18n';
import { getServer, setServer, setServerPassword, restartServer, type ServerSettings } from '../../lib/api';

export function NetworkSection() {
  const t = useT();
  const [view, setView] = useState<ServerSettings | null>(null);
  const [portDraft, setPortDraft] = useState('');
  const [remoteDraft, setRemoteDraft] = useState(false);
  const [passwordDraft, setPasswordDraft] = useState('');
  const [passwordError, setPasswordError] = useState<string | null>(null);
  const [savingPassword, setSavingPassword] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);
  const intervalRef = useRef<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    getServer()
      .then((result) => {
        if (!cancelled) {
          setView(result);
          setPortDraft(String(result.port));
          setRemoteDraft(result.remote);
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

  const dirty = portDraft !== String(view.port) || remoteDraft !== view.remote;
  const save: SectionSave = {
    dirty,
    onSave: async () => {
      const next = await setServer(Number(portDraft), remoteDraft);
      setView(next);
      setPortDraft(String(next.port));
      setRemoteDraft(next.remote);
    },
  };

  const remoteDisabled = !view.passwordSet && !remoteDraft;

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

  const onSavePassword = async () => {
    if (!passwordDraft || savingPassword) {
      return;
    }

    setSavingPassword(true);
    setPasswordError(null);
    try {
      const next = await setServerPassword(passwordDraft);
      setView(next);
      setPasswordDraft('');
    } catch (error) {
      setPasswordError(error instanceof Error ? error.message : String(error));
    } finally {
      setSavingPassword(false);
    }
  };

  return (
    <Section
      title={t('networkTitle')}
      description={t('networkDescription')}
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
          value={portDraft}
          onChange={(event) => setPortDraft(event.currentTarget.value)}
        />
      </FieldRow>
      <FieldRow
        label={t('networkRemoteLabel')}
        hint={remoteDisabled ? t('networkRemoteNeedsPassword') : t('networkRemoteHint')}
      >
        <Switch
          checked={remoteDraft}
          onChange={setRemoteDraft}
          label={t('networkRemoteLabel')}
          disabled={remoteDisabled}
        />
      </FieldRow>
      <FieldRow
        label={t('networkPasswordLabel')}
        hint={
          view.passwordSet ? (
            <Badge tone="ok">{t('networkPasswordSet')}</Badge>
          ) : (
            <Badge tone="attention">{t('networkPasswordNone')}</Badge>
          )
        }
      >
        <div className="flex items-center gap-2">
          <TextInput
            type="password"
            className="w-48"
            value={passwordDraft}
            onChange={(event) => setPasswordDraft(event.currentTarget.value)}
          />
          <Button
            variant="ghost"
            size="sm"
            iconOnly
            title={t('networkPasswordSave')}
            aria-label={t('networkPasswordSave')}
            onClick={onSavePassword}
            disabled={savingPassword || passwordDraft.length === 0}
          >
            {savingPassword ? (
              <SpinnerIcon className="h-4 w-4 animate-spin" />
            ) : (
              <CheckIcon className="h-4 w-4" />
            )}
          </Button>
        </div>
      </FieldRow>
      {passwordError ? <p className="m-0 text-xs text-danger">{passwordError}</p> : null}
      <div className="flex items-center gap-2 text-xs text-muted">
        {t('serverRunningOn')}
        <Badge tone="accent">{view.runningPort}</Badge>
        {view.runningRemote ? <Badge tone="info">{t('networkRemoteOn')}</Badge> : null}
        {view.restartNeeded ? <Badge tone="attention">{t('serverRestartNeeded')}</Badge> : null}
      </div>
      {view.fromFlag ? <p className="m-0 text-xs text-faint">{t('serverFromFlag')}</p> : null}
    </Section>
  );
}
