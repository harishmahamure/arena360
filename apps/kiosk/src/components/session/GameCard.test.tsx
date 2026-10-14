import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LaunchEntry } from '../../lib/allowList';
import { GameCard } from './GameCard';

const entry: LaunchEntry = {
  id: 'game',
  name: 'Test Game',
  executablePath: 'C:/Games/game.exe',
  thumbnailUrl: '/poster.jpg',
  videoUrl: '/preview.webm',
};

describe('GameCard previews', () => {
  beforeEach(() => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
  });

  it('loads only the hovered preview and releases it when the pointer leaves', () => {
    const { container } = render(
      <>
        <GameCard entry={entry} launchable launching={false} onLaunch={vi.fn()} />
        <GameCard
          entry={{ ...entry, id: 'other', name: 'Other' }}
          launchable
          launching={false}
          onLaunch={vi.fn()}
        />
      </>,
    );
    expect(container.querySelectorAll('video')).toHaveLength(0);
    const card = screen.getByRole('button', { name: /test game/i });
    fireEvent.mouseEnter(card);
    expect(container.querySelectorAll('video')).toHaveLength(1);
    fireEvent.mouseLeave(card);
    expect(container.querySelectorAll('video')).toHaveLength(0);
  });

  it('keeps a keyboard-focused preview after hover ends, then removes it on blur', () => {
    const { container } = render(
      <GameCard entry={entry} launchable launching={false} onLaunch={vi.fn()} />,
    );
    const card = screen.getByRole('button');
    fireEvent.focus(card);
    fireEvent.mouseEnter(card);
    fireEvent.mouseLeave(card);
    expect(container.querySelector('video')).toBeInTheDocument();
    fireEvent.blur(card);
    expect(container.querySelector('video')).toBeNull();
  });
});
