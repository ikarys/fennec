// Tells the tabs when the session starts or stops running and when it gets a name.
import { useEffect, useRef } from 'react';

export function useSessionTabSync(
  running: boolean,
  name: string | undefined,
  onRunningChanged: () => void,
  onRenamed: () => void
): void {
  useEffect(() => {
    onRunningChanged();
  }, [running, onRunningChanged]);

  const previousName = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (name && name !== previousName.current) {
      onRenamed();
    }
    previousName.current = name;
  }, [name, onRenamed]);
}
