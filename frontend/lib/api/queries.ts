"use client";

import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, withQuery } from "./client";
import type {
  AdjustmentResponse,
  CategoryView,
  DashboardSummary,
  DataList,
  ExternalSource,
  IdentifyResponse,
  MovementView,
  Paginated,
  ProductDetail,
  ProductListItem,
  SaleView,
  SalesPage,
  ScanResponse,
  StoreSettings,
  IdentifierView,
} from "./types";
import type { CreateProductPayload } from "@/lib/schemas";

/** Cache keys. Mutations invalidate by prefix, e.g. ["products"]. */
export const keys = {
  categories: ["categories"] as const,
  products: (params: object = {}) => ["products", params] as const,
  product: (id: string) => ["product", id] as const,
  inventory: (params: object = {}) => ["inventory", params] as const,
  movements: (params: object = {}) => ["movements", params] as const,
  sales: (params: object = {}) => ["sales", params] as const,
  sale: (id: string) => ["sale", id] as const,
  dashboard: (date?: string) => ["dashboard", date ?? "today"] as const,
  settings: ["settings"] as const,
  sources: ["external-sources"] as const,
};

// ---------- Categories ----------

export function useCategories() {
  return useQuery({
    queryKey: keys.categories,
    queryFn: () => api.get<DataList<CategoryView>>("/categories"),
    select: (res) => res.data,
  });
}

export function useSaveCategory() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name, isActive }: { id?: string; name?: string; isActive?: boolean }) =>
      id
        ? api.patch<CategoryView>(`/categories/${id}`, { name, is_active: isActive })
        : api.post<CategoryView>("/categories", { name }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.categories }),
  });
}

// ---------- Products ----------

export interface ProductListParams {
  q?: string;
  status?: "active" | "inactive" | "all";
  low_stock?: boolean;
  category_id?: string;
  limit?: number;
  offset?: number;
}

export function useProducts(params: ProductListParams) {
  return useQuery({
    queryKey: keys.products(params),
    queryFn: ({ signal }) => api.get<Paginated<ProductListItem>>(withQuery("/products", { ...params }), signal),
    placeholderData: keepPreviousData,
  });
}

export function useProduct(id: string) {
  return useQuery({
    queryKey: keys.product(id),
    queryFn: () => api.get<ProductDetail>(`/products/${id}`),
    enabled: Boolean(id),
  });
}

export function useIdentify() {
  return useMutation({
    mutationFn: (code: string) => api.post<IdentifyResponse>("/products/identify", { code }),
  });
}

export function useLocalScan() {
  return useMutation({
    mutationFn: (code: string) => api.post<ScanResponse>("/products/scan", { code }),
  });
}

export function useCreateProduct() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (payload: CreateProductPayload) => api.post<ProductDetail>("/products", payload),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["products"] }),
  });
}

export interface ProductUpdate {
  name?: string;
  description?: string | null;
  brand?: string | null;
  manufacturer?: string | null;
  sku?: string | null;
  category_id?: string | null;
  unit?: string;
  purchase_price_idr?: number;
  selling_price_idr?: number;
  low_stock_threshold?: number | null;
  image_url?: string | null;
  bpom_registration_number?: string | null;
  registration_status?: "unknown" | "reported";
  is_active?: boolean;
}

export function useUpdateProduct(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: ProductUpdate) => api.patch<ProductDetail>(`/products/${id}`, body),
    onSuccess: (product) => {
      queryClient.setQueryData(keys.product(id), product);
      queryClient.invalidateQueries({ queryKey: ["products"] });
      queryClient.invalidateQueries({ queryKey: ["inventory"] });
    },
  });
}

export function useAddIdentifier(productId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => api.post<IdentifierView>(`/products/${productId}/identifiers`, { code }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: keys.product(productId) });
      queryClient.invalidateQueries({ queryKey: ["products"] });
    },
  });
}

export function useRemoveIdentifier(productId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (identifierId: string) => api.delete<void>(`/products/${productId}/identifiers/${identifierId}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: keys.product(productId) });
      queryClient.invalidateQueries({ queryKey: ["products"] });
    },
  });
}

export function useExternalSources(enabled = true) {
  return useQuery({
    queryKey: keys.sources,
    queryFn: () => api.get<DataList<ExternalSource>>("/external-sources"),
    select: (res) => res.data,
    enabled,
  });
}

// ---------- Inventory ----------

export interface InventoryParams {
  q?: string;
  low_stock?: boolean;
  limit?: number;
  offset?: number;
}

export function useInventory(params: InventoryParams) {
  return useQuery({
    queryKey: keys.inventory(params),
    queryFn: () => api.get<Paginated<ProductListItem>>(withQuery("/inventory", { ...params })),
    placeholderData: keepPreviousData,
  });
}

export interface MovementParams {
  product_id?: string;
  reason?: string;
  limit?: number;
  offset?: number;
}

export function useMovements(params: MovementParams) {
  return useQuery({
    queryKey: keys.movements(params),
    queryFn: () => api.get<Paginated<MovementView>>(withQuery("/inventory/movements", { ...params })),
    placeholderData: keepPreviousData,
  });
}

export function useAdjustStock() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: { product_id: string; quantity_delta: number; reason: string; note?: string }) =>
      api.post<AdjustmentResponse>("/inventory/adjustments", body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["inventory"] });
      queryClient.invalidateQueries({ queryKey: ["movements"] });
      queryClient.invalidateQueries({ queryKey: ["products"] });
      queryClient.invalidateQueries({ queryKey: ["product"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
}

// ---------- Sales ----------

export interface SalesParams {
  from?: string;
  to?: string;
  q?: string;
  limit?: number;
  offset?: number;
}

export function useSales(params: SalesParams) {
  return useQuery({
    queryKey: keys.sales(params),
    queryFn: () => api.get<SalesPage>(withQuery("/sales", { ...params })),
    placeholderData: keepPreviousData,
  });
}

export function useSale(id: string) {
  return useQuery({
    queryKey: keys.sale(id),
    queryFn: () => api.get<SaleView>(`/sales/${id}`),
    enabled: Boolean(id),
  });
}

export interface CheckoutBody {
  client_request_id: string;
  items: { product_id: string; quantity: number }[];
  discount_idr: number;
  payments: { method: string; amount_idr: number }[];
}

export function useCheckout() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CheckoutBody) => api.post<SaleView>("/sales", body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["products"] });
      queryClient.invalidateQueries({ queryKey: ["inventory"] });
      queryClient.invalidateQueries({ queryKey: ["sales"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
}

// ---------- Dashboard, settings ----------

export function useDashboard(date?: string) {
  return useQuery({
    queryKey: keys.dashboard(date),
    queryFn: () => api.get<DashboardSummary>(withQuery("/dashboard/summary", { date })),
    refetchInterval: 60_000,
  });
}

export function useStoreSettings() {
  return useQuery({
    queryKey: keys.settings,
    queryFn: () => api.get<StoreSettings>("/settings"),
  });
}

export function useSaveSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: Record<string, unknown>) => api.patch<StoreSettings>("/settings", body),
    onSuccess: (settings) => {
      queryClient.setQueryData(keys.settings, settings);
      queryClient.invalidateQueries({ queryKey: ["session"] });
    },
  });
}
