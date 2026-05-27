import { http } from '@gaming-cafe/utils';

export interface ExpenseCategory {
  id: string;
  name: string;
  description?: string | null;
  parentId?: string | null;
  isActive: boolean;
  budgetAmount?: number | null;
  budgetPeriod?: string | null;
  createdAt: string;
  updatedAt: string;
}

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

export interface Expense {
  id: string;
  categoryId: string;
  vendorId?: string | null;
  amount: number;
  paymentMethod: string;
  paymentAccount?: string | null;
  description?: string | null;
  receiptUrl?: string | null;
  expenseDate: string;
  isRecurring: boolean;
  recurrencePattern?: string | null;
  nextRecurrenceDate?: string | null;
  approvalStatus: string;
  approvedBy?: string | null;
  approvedAt?: string | null;
  rejectionReason?: string | null;
  shiftId?: string | null;
  createdBy?: string | null;
  updatedBy?: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface ExpenseSummary {
  categoryName: string;
  budgetAmount?: number | null;
  budgetPeriod?: string | null;
  totalSpent: number;
  remainingBudget?: number | null;
  expenseCount: number;
}

export interface ListResponse<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

// Expense categories
export const getExpenseCategories = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<ExpenseCategory>>('/expense-categories', { params: filters });

export const getExpenseCategory = async (id: string) =>
  http.get<ExpenseCategory>(`/expense-categories/${id}`);

export const createExpenseCategory = async (data: Partial<ExpenseCategory>) =>
  http.post<ExpenseCategory>('/expense-categories', data);

export const updateExpenseCategory = async (id: string, data: Partial<ExpenseCategory>) =>
  http.patch<ExpenseCategory>(`/expense-categories/${id}`, data);

export const deleteExpenseCategory = async (id: string) => http.delete(`/expense-categories/${id}`);

// Vendors
export const getVendors = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<Vendor>>('/vendors', { params: filters });

export const getVendor = async (id: string) => http.get<Vendor>(`/vendors/${id}`);

export const createVendor = async (data: Partial<Vendor>) => http.post<Vendor>('/vendors', data);

export const updateVendor = async (id: string, data: Partial<Vendor>) =>
  http.patch<Vendor>(`/vendors/${id}`, data);

export const deleteVendor = async (id: string) => http.delete(`/vendors/${id}`);

// Expenses
export const getExpenses = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<Expense>>('/expenses', {
    params: {
      ...filters,
      limit: filters.limit || 20,
      page: filters.page || 1,
    },
  });

export const getExpense = async (id: string) => http.get<Expense>(`/expenses/${id}`);

export const createExpense = async (data: Partial<Expense>) =>
  http.post<Expense>('/expenses', data);

export const updateExpense = async (id: string, data: Partial<Expense>) =>
  http.patch<Expense>(`/expenses/${id}`, data);

export const approveExpense = async (id: string) =>
  http.patch<Expense>(`/expenses/${id}/approve`, {});

export const rejectExpense = async (id: string, rejectionReason: string) =>
  http.patch<Expense>(`/expenses/${id}/reject`, { rejectionReason });

export const getExpenseSummary = async (filters: Record<string, unknown> = {}) =>
  http.get<ExpenseSummary[]>('/expenses/summary', { params: filters });
