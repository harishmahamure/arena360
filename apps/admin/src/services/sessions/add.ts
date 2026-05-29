import { http } from '@gaming-cafe/utils';

export interface StartSessionPayload {
  balanceId: string;
  deviceId: string;
  startTime?: string;
}

export const startSession = async (payload: StartSessionPayload) => {
  return http.post('/sessions', payload);
};
