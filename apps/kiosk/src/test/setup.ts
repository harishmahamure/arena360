import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// jsdom has no media decoder. Playback behavior is asserted with these spies.
HTMLMediaElement.prototype.play = vi.fn().mockResolvedValue(undefined);
HTMLMediaElement.prototype.pause = vi.fn();
