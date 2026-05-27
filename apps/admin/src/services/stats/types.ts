/**
 * DTO for querying dashboard stats with optional date filters
 */
export interface StatsQueryDto {
  startDate?: string;
  endDate?: string;
}

export interface StaffStatsQueryDto extends StatsQueryDto {
  shiftStart?: string;
}

/**
 * Current vs previous period pair (revenue, usage stats endpoints)
 */
export interface PeriodPair<T> {
  current: T;
  previous: T;
}

/**
 * Revenue breakdown by payment method
 */
export interface RevenueByPaymentMethodDto {
  plan: number;
  merchandise: number;
  total: number;
  cashRevenue: number;
  onlineRevenue: number;
}

/**
 * Transaction statistics
 */
export interface TransactionStatsDto {
  totalTransactions: number;
  completedTransactions: number;
  pendingTransactions: number;
  failedTransactions: number;
  averageTransactionAmount: number;
}

/**
 * Usage statistics
 */
export interface UsageStatsDto {
  totalSessions: number;
  activeSessions: number;
  completedSessions: number;
  totalHours: number;
  totalMinutes: number;
  averageSessionDuration: number;
}

/**
 * User statistics
 */
export interface UserStatsDto {
  totalUsers: number;
  activeUsers: number;
  totalPlayers: number;
  activePlayers: number;
  newUsersThisPeriod: number;
}

/**
 * Plan statistics
 */
export interface PlanStatsDto {
  totalActivePlans: number;
  totalExpiredPlans: number;
  plansByType: {
    type: string;
    count: number;
    revenue: number;
  }[];
}

/**
 * Device statistics
 */
export interface DeviceStatsDto {
  totalDevices: number;
  activeDevices: number;
  deviceUtilization: {
    deviceId: string;
    deviceName: string;
    totalSessions: number;
    totalHours: number;
    utilizationPercentage: number;
  }[];
}

/**
 * Top performing items
 */
export interface TopPerformersDto {
  topPlans: {
    planId: string;
    planName: string;
    revenue: number;
    purchaseCount: number;
  }[];
  topGames: {
    gameId: string;
    gameName: string;
    sessionCount: number;
    totalHours: number;
  }[];
  topPlayers: {
    playerId: string;
    playerName: string;
    totalSpent: number;
    totalSessions: number;
  }[];
}

/**
 * Revenue trends over time
 */
export interface RevenueTrendDto {
  date: string;
  cashRevenue: number;
  onlineRevenue: number;
  totalRevenue: number;
  transactionCount: number;
}

/**
 * Main dashboard stats response
 */
export interface DashboardStatsDto {
  period: {
    startDate: string;
    endDate: string;
    label: string;
    previousLabel: string;
  };
  revenue: {
    previous: RevenueByPaymentMethodDto;
    current: RevenueByPaymentMethodDto;
  };
  transactions: {
    previous: TransactionStatsDto;
    current: TransactionStatsDto;
  };
  usage: {
    previous: UsageStatsDto;
    current: UsageStatsDto;
  };
  users: UserStatsDto;
  plans: PlanStatsDto;
  devices: DeviceStatsDto;
  topPerformers: TopPerformersDto;
  revenueTrend: RevenueTrendDto[];
}

export interface StaffPlayerStatsDto {
  activePlayers: number;
  newPlayersInPeriod: number;
}

export interface StaffDeviceStatsDto {
  total: number;
  available: number;
  inUse: number;
}

export interface StaffDashboardStatsDto {
  period: {
    startDate: string;
    endDate: string;
    label: string;
    previousLabel: string;
  };
  shift?: {
    startDate: string;
    endDate: string;
    label: string;
    previousLabel: string;
  } | null;
  sessions: UsageStatsDto;
  transactions: TransactionStatsDto;
  revenue: RevenueByPaymentMethodDto;
  shiftRevenue?: RevenueByPaymentMethodDto | null;
  players: StaffPlayerStatsDto;
  devices: StaffDeviceStatsDto;
}
