import { describe, expect, it } from 'vitest';
import type { LaunchEntry } from './allowList';
import { collectGameGenres, filterGameLibrary, matchesGameQuery } from './gameLibrary';

const games: LaunchEntry[] = [
  {
    id: '1',
    name: 'Counter-Strike 2',
    executablePath: 'C:\\cs2.exe',
    genre: 'FPS',
    sortOrder: 2,
  },
  {
    id: '2',
    name: 'Valorant',
    executablePath: 'C:\\valorant.exe',
    genre: 'Tactical Shooter',
    sortOrder: 1,
  },
  {
    id: '3',
    name: 'Rocket League',
    executablePath: 'C:\\rl.exe',
    sortOrder: 3,
  },
];

describe('matchesGameQuery', () => {
  it('matches substring and word tokens', () => {
    expect(matchesGameQuery('Counter-Strike 2', 'counter')).toBe(true);
    expect(matchesGameQuery('Counter-Strike 2', 'strike 2')).toBe(true);
    expect(matchesGameQuery('Counter-Strike 2', 'dota')).toBe(false);
  });

  it('matches empty query', () => {
    expect(matchesGameQuery('Valorant', '')).toBe(true);
  });
});

describe('collectGameGenres', () => {
  it('returns sorted unique genres', () => {
    expect(collectGameGenres(games)).toEqual(['FPS', 'Tactical Shooter']);
  });
});

describe('filterGameLibrary', () => {
  it('filters by name', () => {
    const result = filterGameLibrary(games, { query: 'valor', genre: null, sort: 'name' });
    expect(result.map((g) => g.name)).toEqual(['Valorant']);
  });

  it('filters by genre', () => {
    const result = filterGameLibrary(games, { query: '', genre: 'FPS', sort: 'name' });
    expect(result.map((g) => g.name)).toEqual(['Counter-Strike 2']);
  });

  it('sorts by sortOrder', () => {
    const result = filterGameLibrary(games, { query: '', genre: null, sort: 'sortOrder' });
    expect(result.map((g) => g.name)).toEqual(['Valorant', 'Counter-Strike 2', 'Rocket League']);
  });

  it('sorts by name', () => {
    const result = filterGameLibrary(games, { query: '', genre: null, sort: 'name' });
    expect(result[0]?.name).toBe('Counter-Strike 2');
  });
});
