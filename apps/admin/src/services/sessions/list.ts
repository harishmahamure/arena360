import { http } from '@gaming-cafe/utils';

export interface GetSessionsResponse {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: GetSessionsResponseData;
}

export interface GetSessionsResponseData {
  data: SessionResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface SessionResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  playerPlanId: string;
  deviceId: string;
  gameId: null;
  startTime: string;
  endTime: string;
  durationMinutes: number;
  timeCreditsConsumed: number;
  playerPlan: PlayerPlan;
  device: Device;
  game: null;
}

interface Device {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  name: string;
  serialNumber: string;
  localIpAddress: null;
  deviceType: string;
  location: string;
  status: string;
}

interface PlayerPlan {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  playerId: string;
  planId: string;
  purchaseDate: string;
  activationDate: string;
  expiryDate: string;
  remainingUsageCount: null;
  remainingTimeCredits: number;
  status: string;
  player: Player;
  plan: Plan;
}

interface Plan {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  name: string;
  description: string;
  price: string;
  planType: string;
  durationMinutes: number;
  validityDays: number;
  timeWindowStart: null;
  timeWindowEnd: null;
  timeCredits: number;
  perMinuteRate: string;
  maxSessions: null;
  isActive: boolean;
}

interface Player {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  email: string;
  username: string;
  isActive: boolean;
  firstName: string;
  lastName: string;
  role: string;
  sessionOtpId: null;
  sessionOtp: string;
}

export const getSessions = async (filters: any = {}) => {
  return http.get<GetSessionsResponseData>('/sessions', {
    params: {
      ...filters,
      sortBy: filters.sortBy || 'startTime',
      sortOrder: filters.sortOrder || 'DESC',
      limit: filters.limit || 20,
      page: filters.page || 1,
    },
  });
};
