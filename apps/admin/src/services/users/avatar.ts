import { http } from '@gaming-cafe/utils';
import type { VerifyOtpResponseUser } from '../auth/types';

/** Set (or clear with null) the signed-in user's own profile photo. */
export const updateOwnAvatar = (avatarUrl: string | null) =>
  http.put<VerifyOtpResponseUser>('/users/me/avatar', { avatarUrl });
