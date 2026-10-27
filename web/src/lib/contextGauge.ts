export type GaugeTone = 'ok' | 'warn' | 'err';

/** The gauge's tone: red from 5 points before the compaction threshold, amber 15 points before; with compaction off, against a full window (amber 85 %, red 95 %). */
export function gaugeTone(percent: number, compactAt: number): GaugeTone {
  const threshold = compactAt > 0 ? compactAt * 100 : 100;
  if (percent >= threshold - 5) return 'err';
  if (percent >= threshold - 15) return 'warn';
  return 'ok';
}
