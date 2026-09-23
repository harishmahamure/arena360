import { http, toastUtils } from '@gaming-cafe/utils';
import type { PanelLoginResponse, VerifyOtpResponseData } from './types';

export const loginPanelAPI = async (username: string, password: string) =>
  http.post<PanelLoginResponse>('/auth/login/panel', { username, password });

export const verifyPanelMfaAPI = async (challengeToken: string, code: string) =>
  http.post<PanelLoginResponse>('/auth/login/panel/mfa', { challengeToken, code });

export const loginAPI = async (username: string, password: string, totp?: string) => {
  try {
    return await http.post<VerifyOtpResponseData>('/auth/login/admin', {
      username,
      password,
      totp,
    });
  } catch (error: unknown) {
    toastUtils.error('Login failed. Please check your credentials.', {
      position: 'bottom-center',
      autoClose: 3000,
    });
    throw error;
  }
};

export const loginStaffAPI = async (username: string, password: string, totp?: string) => {
  const response = await http.post<VerifyOtpResponseData>('/auth/login/staff', {
    username,
    password,
    totp,
  });
  return response;
};
