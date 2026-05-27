import { http } from '@gaming-cafe/utils';
import type { SessionResponse } from './list';

/**
 * Payload for ending a session matching EndSessionDto
 */
export interface EndSessionPayload {
  endTime?: string;
}

export const endSession = async (id: string, payload: EndSessionPayload = {}) => {
  return http.patch<SessionResponse>(`/sessions/${id}/end`, payload);
};
