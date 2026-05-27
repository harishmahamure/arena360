import { http } from '@gaming-cafe/utils';

interface TransactionResponse {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: Data;
}

interface Data {
  id: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: null;
  playerId: string;
  transactionType: string;
  planId: string;
  amount: string;
  paymentMethod: string;
  paymentStatus: string;
  notes: null;
  transactionDate: string;
  player: PlayerResponse;
  plan: PlanResponse;
  transactionProducts: [];
}

interface PlanResponse {
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

interface PlayerResponse {
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

export const getTransactionById = async (id: string) => {
  return http.get<TransactionResponse>(`/transactions/${id}`);
};
