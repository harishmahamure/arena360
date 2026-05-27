import { http } from '@gaming-cafe/utils';

export interface TotpSetupResponse {
  secret: string;
  otpauthUri: string;
  totpEnabled: boolean;
}

export const setupTotp = async (userId: string) =>
  http.post<TotpSetupResponse>(`/users/${userId}/totp/setup`, {});

export const verifyTotpSetup = async (userId: string, code: string) =>
  http.post<TotpSetupResponse>(`/users/${userId}/totp/verify`, { code });

export const disableTotp = async (userId: string) =>
  http.delete<{ disabled: boolean }>(`/users/${userId}/totp`);
