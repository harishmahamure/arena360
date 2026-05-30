import type { components } from '@gaming-cafe/api-types';
import { getHttpClient } from './http';

export type Game = components['schemas']['Game'];

interface GamePage {
  data: Game[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

/** Fetch the active game catalog (DRAFT-0022). Display-only; launching stays
 * client-side via the allow-list (ADR-0019). */
export async function fetchActiveGames(): Promise<Game[]> {
  const http = getHttpClient();
  const page = await http.get<GamePage>('/games?isActive=true&limit=200&sortBy=sortOrder');
  return page.data ?? [];
}

/** First active game that carries a background asset (video preferred). */
export function pickBackgroundGame(games: Game[]): Game | undefined {
  return games.find((g) => g.videoUrl) ?? games.find((g) => g.thumbnailUrl);
}
