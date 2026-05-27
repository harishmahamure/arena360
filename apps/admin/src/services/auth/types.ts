export interface VerifyOtpResponse {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: VerifyOtpResponseData;
}

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

export interface LoginResponse {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: LoginResponseData;
}

export interface LoginResponseData {
  message: string;
  transactionId: string;
}
