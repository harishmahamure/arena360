import { http } from '@gaming-cafe/utils';

export interface CreateSsoTokenInput {
  purpose: 'tv_provision' | 'tv_login';
  deviceId: string;
}

export interface CreateSsoTokenResponse {
  token: string;
  expiresAt: string;
  deviceId?: string;
}

export const createSsoToken = async (input: CreateSsoTokenInput) => {
  return http.post<CreateSsoTokenResponse>('/auth/sso/tokens', input);
};
