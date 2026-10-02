import { http } from '@gaming-cafe/utils';
import type { SessionResponse } from './list';

/**
 * Payload for ending a session matching EndSessionDto
 */
export interface EndSessionPayload {
  endTime?: string;
  staffTotp?: string;
  /** One of voluntary | auto | force | offline_reconcile (echoed to realtime). */
  reason?: string;
}

export const endSession = async (id: string, payload: EndSessionPayload = {}) => {
  return http.patch<SessionResponse>(`/sessions/${id}/end`, payload);
};

/**
 * Force-end a session from the admin SPA. Emits `session.ended` with
 * `reason: "force"`, which the kiosk renders as a grace overlay before
 * cleaning up (D14).
 */
export const forceEndSession = async (id: string, staffTotp?: string) => {
  return endSession(id, { reason: 'force', staffTotp });
};
