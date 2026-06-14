import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LaunchEntry } from '../../lib/allowList';
import { HomeView } from './HomeView';

const launch = vi.fn();
const isLaunchable = vi.fn((entry: LaunchEntry) => entry.present !== false);

const games: LaunchEntry[] = [
  {
    id: 'game-1',
    name: 'Cyber Odyssey',
    executablePath: 'C:\\Games\\cyber.exe',
    present: true,
    genre: 'RPG',
    description: 'Explore the neon frontier.',
    thumbnailUrl: 'https://cdn.example/cyber.jpg',
  },
  {
    id: 'game-2',
    name: 'Star Arena',
    executablePath: 'C:\\Games\\star.exe',
    present: true,
    genre: 'Shooter',
    thumbnailUrl: 'https://cdn.example/star.jpg',
  },
  {
    id: 'game-3',
    name: 'Missing Game',
    executablePath: 'C:\\Games\\missing.exe',
    present: false,
    genre: 'Puzzle',
  },
];

vi.mock('../../lib/allowList', () => ({
  fetchGames: () => games,
}));

vi.mock('./useAllowList', () => ({
  useAllowList: (loader: () => LaunchEntry[]) => ({ items: loader() }),
}));

vi.mock('./useLauncher', () => ({
  useLauncher: () => ({
    launchingKey: null,
    isLaunchable,
    launch,
  }),
}));

describe('HomeView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isLaunchable.mockImplementation((entry: LaunchEntry) => entry.present !== false);
    Element.prototype.scrollIntoView = vi.fn();
  });

  it('renders HeroGameCarousel tiles for allow-list games', () => {
    render(<HomeView deviceName="PC-01" onNavigate={vi.fn()} onSearchLibrary={vi.fn()} />);

    const carousel = screen.getByRole('listbox', { name: /featured games/i });
    expect(within(carousel).getByRole('option', { name: /cyber odyssey/i })).toBeInTheDocument();
    expect(within(carousel).getByRole('option', { name: /star arena/i })).toBeInTheDocument();
    expect(within(carousel).getByRole('button', { name: /all games/i })).toBeInTheDocument();
  });

  it('updates featured title when a carousel tile is selected', () => {
    render(<HomeView deviceName="PC-01" onNavigate={vi.fn()} onSearchLibrary={vi.fn()} />);

    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Cyber Odyssey');

    fireEvent.click(screen.getByRole('option', { name: /star arena/i }));
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Star Arena');
  });

  it('disables Play Now when the featured game is not launchable', () => {
    render(<HomeView deviceName="PC-01" onNavigate={vi.fn()} onSearchLibrary={vi.fn()} />);

    fireEvent.click(screen.getByRole('option', { name: /missing game/i }));
    expect(screen.getByRole('button', { name: /play now/i })).toBeDisabled();
  });

  it('launches the featured game from Play Now', () => {
    render(<HomeView deviceName="PC-01" onNavigate={vi.fn()} onSearchLibrary={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /play now/i }));
    expect(launch).toHaveBeenCalledWith(games[0]);
  });

  it('renders Quick Launch GameCards separately from the hero carousel', () => {
    render(<HomeView deviceName="PC-01" onNavigate={vi.fn()} onSearchLibrary={vi.fn()} />);

    expect(screen.getByRole('heading', { name: /quick launch/i })).toBeInTheDocument();
    expect(screen.getByText('PC-01')).toBeInTheDocument();

    const quickCards = screen.getAllByRole('button', { name: /cyber odyssey/i });
    expect(quickCards.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole('listbox', { name: /featured games/i })).toBeInTheDocument();
  });

  it('navigates to library with featured query from Game details', () => {
    const onNavigate = vi.fn();
    const onSearchLibrary = vi.fn();

    render(
      <HomeView deviceName="PC-01" onNavigate={onNavigate} onSearchLibrary={onSearchLibrary} />,
    );

    fireEvent.click(screen.getByRole('button', { name: /game details/i }));
    expect(onSearchLibrary).toHaveBeenCalledWith('Cyber Odyssey');
    expect(onNavigate).toHaveBeenCalledWith('library');
  });
});
