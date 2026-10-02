import { http } from '@gaming-cafe/utils';

export interface NotificationItem {
  id: string;
  activityId: string;
  kind: string;
  title: string;
  summary?: string | null;
  payload?: Record<string, unknown>;
  actorUserId?: string | null;
  entityType?: string | null;
  entityId?: string | null;
  readAt?: string | null;
  createdAt: string;
}

export interface ActivityLogEntry {
  id: string;
  kind: string;
  title: string;
  summary?: string | null;
  payload?: Record<string, unknown>;
  actorUserId?: string | null;
  entityType?: string | null;
  entityId?: string | null;
  createdAt: string;
}

export interface PaginatedNotifications {
  data: NotificationItem[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface PaginatedActivityLog {
  data: ActivityLogEntry[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface NotificationFilters {
  page?: number;
  limit?: number;
  unreadOnly?: boolean;
  importantOnly?: boolean;
}

export interface ActivityLogFilters {
  page?: number;
  limit?: number;
  kind?: string;
  actorUserId?: string;
  from?: string;
  to?: string;
}

export const MAX_INBOX_NOTIFICATIONS = 10;

export const getNotifications = (filters: NotificationFilters = {}) =>
  http.get<PaginatedNotifications>('/notifications', {
    params: {
      page: filters.page ?? 1,
      limit: Math.min(filters.limit ?? MAX_INBOX_NOTIFICATIONS, MAX_INBOX_NOTIFICATIONS),
      unreadOnly: filters.unreadOnly,
      importantOnly: filters.importantOnly,
    },
  });

export const getUnreadCount = (importantOnly = true) =>
  http.get<{ count: number }>('/notifications/unread-count', {
    params: importantOnly ? { importantOnly: true } : undefined,
  });

export const markNotificationRead = (id: string) =>
  http.patch<{ count: number }>(`/notifications/${id}/read`);

export const markAllNotificationsRead = () =>
  http.post<{ count: number }>('/notifications/read-all');

export const getActivityLog = (filters: ActivityLogFilters = {}) =>
  http.get<PaginatedActivityLog>('/activity-log', {
    params: {
      page: filters.page ?? 1,
      limit: filters.limit ?? 20,
      kind: filters.kind,
      actorUserId: filters.actorUserId,
      from: filters.from,
      to: filters.to,
    },
  });
