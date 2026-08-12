/**
 * DTO for querying dashboard stats with optional date filters
 */
export interface StatsQueryDto {
  startDate?: string;
  endDate?: string;
  /** When false, previous-period metrics are omitted. Defaults to true. */
  compare?: boolean;
}

export interface StaffStatsQueryDto extends StatsQueryDto {
  shiftStart?: string;
}

/**
 * Current vs previous period pair (revenue, usage stats endpoints)
 */
export interface PeriodPair<T> {
  current: T;
  previous?: T | null;
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
  creditRevenue: number;
  planTransactionCount: number;
  productTransactionCount: number;
  planCashRevenue: number;
  planOnlineRevenue: number;
  planCreditRevenue: number;
  productCashRevenue: number;
  productOnlineRevenue: number;
  productCreditRevenue: number;
  planCashCount: number;
  planOnlineCount: number;
  planCreditCount: number;
  productCashCount: number;
  productOnlineCount: number;
  productCreditCount: number;
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
  revenue: PeriodPair<RevenueByPaymentMethodDto>;
  transactions: PeriodPair<TransactionStatsDto>;
  usage: PeriodPair<UsageStatsDto>;
  users: UserStatsDto;
  plans: PlanStatsDto;
  devices: DeviceStatsDto;
  topPerformers: TopPerformersDto;
  revenueTrend: RevenueTrendDto[];
}

/** Cash register reconciliation aggregates for a period */
export interface FinanceReconciliationStatsDto {
  period: {
    startDate: string;
    endDate: string;
    label: string;
    previousLabel: string;
  };
  metrics: PeriodPair<{
    openCount: number;
    closedCount: number;
    reconciledCount: number;
    pendingReconcileCount: number;
    totalDeposited: number;
  }>;
}

/** Cash deposit pipeline aggregates for a period */
export interface FinanceDepositStatsDto {
  period: {
    startDate: string;
    endDate: string;
    label: string;
    previousLabel: string;
  };
  metrics: PeriodPair<{
    pendingCount: number;
    pendingAmount: number;
    approvedCount: number;
    approvedAmount: number;
    rejectedCount: number;
    rejectedAmount: number;
    bankAmount: number;
    homeAmount: number;
  }>;
}

/** Cash register variance aggregates for a period */
export interface FinanceVarianceStatsDto {
  period: {
    startDate: string;
    endDate: string;
    label: string;
    previousLabel: string;
  };
  metrics: PeriodPair<{
    totalVariance: number;
    averageVariance: number;
    overCount: number;
    shortCount: number;
    evenCount: number;
    registerCount: number;
  }>;
  registers: {
    id: string;
    shiftId: string;
    status: string;
    variance: number;
    closingBalance?: number | null;
    expectedClosing?: number | null;
    updatedAt: string;
  }[];
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
