export interface VerifyOtpResponseData {
  accessToken: string;
  user: VerifyOtpResponseUser;
}

export interface VerifyOtpResponseUser {
  id: string;
  email?: string | null;
  username: string;
  firstName: string;
  lastName: string;
  role: string;
  isActive: boolean;
}

export type PanelLoginResponse =
  | {
      status: 'authenticated';
      accessToken: string;
      user: VerifyOtpResponseUser;
      nextStep: 'dashboard' | 'shift_setup';
    }
  | {
      status: 'mfa_required';
      challengeToken: string;
      expiresAt: string;
    };
