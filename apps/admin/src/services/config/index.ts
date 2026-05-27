import { http } from '@gaming-cafe/utils';

export interface Configuration {
  id: string;
  key: string;
  value: unknown;
  category: string;
  description?: string | null;
  createdAt: string;
  updatedAt: string;
}

export const getConfigs = async (category?: string) =>
  http.get<Configuration[]>('/config', { params: category ? { category } : {} });

export const getConfig = async (key: string) => http.get<Configuration>(`/config/${key}`);

export const upsertConfig = async (key: string, value: unknown, description?: string) =>
  http.put<Configuration>(`/config/${key}`, { value, description });
