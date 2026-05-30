const SOUND_BY_THRESHOLD: Record<number, string> = {
  10: '/10 minutes.mp3',
  5: '/5 minutes.mp3',
  2: '/2 minutes.mp3',
};

export function playRemainingTimeSound(minutes: number): void {
  const src = SOUND_BY_THRESHOLD[minutes];
  if (!src) return;

  try {
    const audio = new Audio(src);
    audio.play().catch(() => {
      // Browsers may block audio if the session has not had user interaction.
    });
  } catch {
    // Audio is best-effort and must never block session logic.
  }
}
