import { http } from '@gaming-cafe/utils';

export interface RegistrationCodeResponse {
  registrationCode: string;
  expiresAt: string;
}

export const issueRegistrationCode = async (deviceId: string) => {
  return http.post<RegistrationCodeResponse>(`/devices/${deviceId}/registration-code`);
};
