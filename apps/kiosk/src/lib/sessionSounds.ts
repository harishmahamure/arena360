const SOUND_BY_THRESHOLD: Record<number, string> = {
  10: '/10 minutes.mp3',
  5: '/5 minutes.mp3',
  2: '/2 minutes.mp3',
};

const preloaded = new Map<number, HTMLAudioElement>();
let unlocked = false;

/** Call after a user gesture (login / start session) so Tauri can play reminders. */
export function prepareSessionSounds(): void {
  for (const [minutes, src] of Object.entries(SOUND_BY_THRESHOLD)) {
    const key = Number(minutes);
    if (preloaded.has(key)) continue;
    const audio = new Audio(src);
    audio.preload = 'auto';
    preloaded.set(key, audio);
  }

  if (unlocked) return;
  const first = preloaded.values().next().value;
  if (!first) return;

  const previousVolume = first.volume;
  first.volume = 0;
  void first
    .play()
    .then(() => {
      first.pause();
      first.currentTime = 0;
      first.volume = previousVolume;
      unlocked = true;
    })
    .catch(() => {
      first.volume = previousVolume;
    });
}

export function playRemainingTimeSound(minutes: number): void {
  const src = SOUND_BY_THRESHOLD[minutes];
  if (!src) return;

  try {
    const audio = preloaded.get(minutes) ?? new Audio(src);
    audio.currentTime = 0;
    void audio.play().catch(() => {
      // Best-effort; reminders must never block session logic.
    });
  } catch {
    // Audio is best-effort and must never block session logic.
  }
}
