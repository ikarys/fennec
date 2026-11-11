/**
 * Attention cues: short synthesized sounds for new notices.
 *
 * They are generated with the Web Audio API, so there is no asset to load and
 * nothing to fetch from a server that may be gone.
 */
export type Cue = 'yourTurn' | 'done' | 'problem';

let context: AudioContext | null = null;

const CUES: Record<
  Cue,
  Array<{ frequency: number; offset: number; duration: number; type: OscillatorType }>
> = {
  yourTurn: [
    { frequency: 660, offset: 0, duration: 0.12, type: 'sine' },
    { frequency: 880, offset: 0.13, duration: 0.18, type: 'sine' },
  ],
  done: [
    { frequency: 784, offset: 0, duration: 0.35, type: 'triangle' },
    { frequency: 1175, offset: 0.05, duration: 0.4, type: 'triangle' },
  ],
  problem: [
    { frequency: 330, offset: 0, duration: 0.16, type: 'sine' },
    { frequency: 247, offset: 0.18, duration: 0.24, type: 'sine' },
  ],
};

export function unlockSounds(): void {
  try {
    if (context === null) {
      context = new AudioContext();
    }
    if (context.state === 'suspended') {
      void context.resume().catch(() => {});
    }
  } catch {
    context = null;
  }
}

function playNote(
  context: AudioContext,
  note: { frequency: number; offset: number; duration: number; type: OscillatorType },
): void {
  const start = context.currentTime + note.offset;
  const end = start + note.duration;
  const oscillator = context.createOscillator();
  oscillator.type = note.type;
  oscillator.frequency.value = note.frequency;
  const gain = context.createGain();
  gain.gain.setValueAtTime(0, start);
  gain.gain.linearRampToValueAtTime(0.08, start + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.0001, end);
  oscillator.connect(gain);
  gain.connect(context.destination);
  oscillator.start(start);
  oscillator.stop(end);
}

export function playCue(cue: Cue): void {
  if (context === null || context.state !== 'running') return;
  for (const note of CUES[cue]) {
    playNote(context, note);
  }
}
