import { http } from '@gaming-cafe/utils';

export interface ListResponse<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface InventoryLocation {
  id: string;
  name: string;
  kind: 'warehouse' | 'store';
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface LocationStockRow {
  locationId: string;
  productId: string;
  quantityPieces: number;
  productName?: string | null;
  productSku?: string | null;
  updatedAt: string;
}

export interface StockReceiptLine {
  id: string;
  receiptId: string;
  productId: string;
  boxQuantity: number;
  piecesAdded: number;
}

export interface StockReceipt {
  id: string;
  locationId: string;
  vendorId?: string | null;
  notes?: string | null;
  createdAt: string;
  lines?: StockReceiptLine[];
}

export interface StockAdjustmentLine {
  id: string;
  adjustmentId: string;
  productId: string;
  previousPieces: number;
  countedPieces: number;
  deltaPieces: number;
}

export interface StockAdjustment {
  id: string;
  locationId: string;
  notes: string;
  createdBy?: string | null;
  createdAt: string;
  lines?: StockAdjustmentLine[];
}

export interface StockAdjustmentWithLines extends StockAdjustment {
  lines: StockAdjustmentLine[];
}

export interface StockTransferLine {
  id: string;
  transferRequestId: string;
  productId: string;
  quantityPieces: number;
}

export interface StockTransferRequest {
  id: string;
  fromLocationId: string;
  toLocationId: string;
  status: string;
  requestedBy?: string | null;
  approvedBy?: string | null;
  approvedAt?: string | null;
  rejectionReason?: string | null;
  fulfilledBy?: string | null;
  fulfilledAt?: string | null;
  createdAt: string;
  updatedAt: string;
  lines?: StockTransferLine[];
}

export interface StockTransferRequestWithLines extends StockTransferRequest {
  lines: StockTransferLine[];
}

export interface StockWasteLine {
  id: string;
  wasteEventId: string;
  productId: string;
  quantityPieces: number;
  reasonCode: string;
  note?: string | null;
}

export interface StockWasteEvent {
  id: string;
  locationId: string;
  status: string;
  notes?: string | null;
  createdAt: string;
  lines?: StockWasteLine[];
}

export interface WasteSummaryRow {
  reasonCode: string;
  productId: string;
  productName: string;
  locationId: string;
  locationName: string;
  totalPieces: number;
  estimatedCost: number;
}

export const getInventoryLocations = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<InventoryLocation>>('/inventory/locations', { params: filters });

export const createInventoryLocation = async (data: {
  name: string;
  kind: string;
  isActive?: boolean;
}) => http.post<InventoryLocation>('/inventory/locations', data);

export const updateInventoryLocation = async (
  id: string,
  data: Partial<{ name: string; kind: string; isActive: boolean }>,
) => http.patch<InventoryLocation>(`/inventory/locations/${id}`, data);

export const getLocationStock = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<LocationStockRow>>('/inventory/stock', { params: filters });

export const createStockAdjustment = async (data: {
  locationId: string;
  notes: string;
  lines: { productId: string; countedPieces: number }[];
}) => http.post<StockAdjustmentWithLines>('/inventory/adjustments', data);

export const getStockAdjustments = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<StockAdjustment>>('/inventory/adjustments', { params: filters });

export const getStockAdjustmentById = async (id: string) =>
  http.get<StockAdjustmentWithLines>(`/inventory/adjustments/${id}`);

export const createStockReceipt = async (data: {
  locationId: string;
  vendorId?: string;
  notes?: string;
  lines: { productId: string; boxQuantity: number }[];
}) => http.post<StockReceipt>('/inventory/receipts', data);

export const getStockReceipts = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<StockReceipt>>('/inventory/receipts', { params: filters });

export const createTransferRequest = async (data: {
  fromLocationId?: string;
  toLocationId?: string;
  lines: { productId: string; quantityPieces: number }[];
}) => http.post<StockTransferRequest>('/inventory/transfer-requests', data);

export const getTransferRequests = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<StockTransferRequest>>('/inventory/transfer-requests', { params: filters });

export const getTransferRequestById = async (id: string) =>
  http.get<StockTransferRequestWithLines>(`/inventory/transfer-requests/${id}`);

export const approveTransferRequest = async (id: string) =>
  http.patch<StockTransferRequest>(`/inventory/transfer-requests/${id}/approve`, {});

export const rejectTransferRequest = async (id: string, rejectionReason: string) =>
  http.patch<StockTransferRequest>(`/inventory/transfer-requests/${id}/reject`, {
    rejectionReason,
  });

export const fulfillTransferRequest = async (id: string) =>
  http.patch<StockTransferRequest>(`/inventory/transfer-requests/${id}/fulfill`, {});

export const createWasteEvent = async (data: {
  locationId: string;
  notes?: string;
  lines: { productId: string; quantityPieces: number; reasonCode: string; note?: string }[];
}) => http.post<StockWasteEvent>('/inventory/waste-events', data);

export const getWasteEvents = async (filters: Record<string, unknown> = {}) =>
  http.get<ListResponse<StockWasteEvent>>('/inventory/waste-events', { params: filters });

export const approveWasteEvent = async (id: string) =>
  http.patch<StockWasteEvent>(`/inventory/waste-events/${id}/approve`, {});

export const rejectWasteEvent = async (id: string, rejectionReason: string) =>
  http.patch<StockWasteEvent>(`/inventory/waste-events/${id}/reject`, { rejectionReason });

export const getWasteSummary = async (filters: Record<string, unknown> = {}) =>
  http.get<WasteSummaryRow[]>('/inventory/waste/summary', { params: filters });
