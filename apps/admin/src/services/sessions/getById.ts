import { http } from '@gaming-cafe/utils';

interface SessionResponse {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: SessionData;
}

interface SessionData {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null | string;
  playerPlanId: string;
  deviceId: string;
  gameId: string;
  startTime: string;
  endTime: null | string;
  durationMinutes: null | number;
  timeCreditsConsumed: null | number;
  playerPlan: PlayerPlanResponse;
  device: DeviceResponse;
  game: GameResponse;
}

interface PlayerPlanResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null | string;
  playerId: string;
  planId: string;
  status: string;
  remainingTimeCredits: number;
  player: {
    id: string;
    email: string;
    username: string;
    firstName: string;
    lastName: string;
  };
  plan: {
    id: string;
    name: string;
    description: string;
    planType: string;
    price: string;
    timeCredits: number;
  };
}

interface DeviceResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null | string;
  name: string;
  deviceType: string;
  status: string;
  serialNumber: string;
  isActive: boolean;
}

interface GameResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null | string;
  title: string;
  genre: string;
  description: string;
  isActive: boolean;
}

export const getSessionById = async (id: string) => {
  return http.get<SessionResponse>(`/sessions/${id}`);
};
