import { http } from '@gaming-cafe/utils';
import type { CreditSummary } from '../credit';

export const setCreditLimit = async (playerId: string, creditLimit: number) =>
  http.patch<CreditSummary>(`/users/${playerId}/credit-limit`, { creditLimit });
