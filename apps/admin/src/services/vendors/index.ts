import { http } from '@gaming-cafe/utils';

export interface Vendor {
  id: string;
  name: string;
  contactPerson?: string | null;
  phone?: string | null;
  email?: string | null;
  address?: string | null;
  gstNumber?: string | null;
  isActive: boolean;
  notes?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ListResponse<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export const getVendors = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<Vendor>>('/vendors', { params: filters });

export const getVendor = async (id: string) => http.get<Vendor>(`/vendors/${id}`);

export const createVendor = async (data: Partial<Vendor>) => http.post<Vendor>('/vendors', data);

export const updateVendor = async (id: string, data: Partial<Vendor>) =>
  http.patch<Vendor>(`/vendors/${id}`, data);

export const deleteVendor = async (id: string) => http.delete(`/vendors/${id}`);
