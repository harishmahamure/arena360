export interface VerifyOtpResponseData {
  accessToken: string;
  user: VerifyOtpResponseUser;
}

export interface VerifyOtpResponseUser {
  id: string;
  email: string;
  username: string;
  firstName: string;
  lastName: string;
  role: string;
  isActive: boolean;
}
