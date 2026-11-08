// Mirrors the Rust DTOs in backend/src. Keep in sync when the API changes.

export type Role = "cashier" | "manager" | "owner";

export interface Paginated<T> {
  data: T[];
  pagination: { limit: number; offset: number; total: number };
}

export interface DataList<T> {
  data: T[];
}

export interface SessionResponse {
  user: { id: string; email: string; full_name: string };
  stores: { store_id: string; name: string; role: Role }[];
}

export interface StoreSettings {
  id: string;
  name: string;
  address: string | null;
  phone: string | null;
  timezone: string;
  currency_code: string;
  tax_rate_bps: number;
  allow_negative_stock: boolean;
  default_low_stock_threshold: number;
  receipt_footer: string | null;
  updated_at: string;
}

export interface CategoryRef {
  id: string;
  name: string;
}

export interface ProductListItem {
  id: string;
  name: string;
  brand: string | null;
  sku: string | null;
  unit: string;
  category: CategoryRef | null;
  primary_barcode: string | null;
  purchase_price_idr: number;
  selling_price_idr: number;
  stock_quantity: number;
  low_stock_threshold: number;
  low_stock_threshold_override: number | null;
  is_low_stock: boolean;
  registration_status: "unknown" | "reported" | "verified" | "expired";
  data_source: string;
  is_active: boolean;
  updated_at: string;
}

export interface IdentifierView {
  id: string;
  scheme: "gtin" | "qr" | "internal";
  kind: string;
  raw_value: string;
  value: string;
  is_primary: boolean;
}

export interface ExternalRecordView {
  source_code: string;
  confidence: "low" | "medium" | "high";
  payload: Record<string, unknown>;
  fetched_at: string;
}

export interface ProductDetail extends ProductListItem {
  description: string | null;
  manufacturer: string | null;
  image_url: string | null;
  bpom_registration_number: string | null;
  registration_valid_until: string | null;
  registration_verified_at: string | null;
  created_at: string;
  identifiers: IdentifierView[];
  external_records: ExternalRecordView[];
}

export type IdentificationStatus =
  | "existing_product"
  | "identified"
  | "partially_identified"
  | "not_found"
  | "provider_unavailable"
  | "invalid_code"
  | "unsupported_format";

export interface RegistrationInfo {
  number: string | null;
  status: "unknown" | "reported" | "verified" | "expired";
  valid_until: string | null;
  verified_at: string | null;
}

export interface ProductSuggestion {
  name?: string | null;
  brand?: string | null;
  manufacturer?: string | null;
  description?: string | null;
  unit_hint?: string | null;
  image_url?: string | null;
  registration?: RegistrationInfo | null;
  fields: string[];
  sources: { code: string; name: string; confidence: "low" | "medium" | "high" }[];
  primary_source: string;
  confidence: "low" | "medium" | "high";
  fetched_at: string;
}

export interface ProviderRecord {
  code: string;
  name: string;
  confidence: "low" | "medium" | "high";
  outcome: "found" | "not_found" | "unavailable" | "not_configured";
  message: string | null;
  latency_ms: number;
}

export interface CodeInput {
  raw: string;
  scheme: string | null;
  kind: string | null;
  canonical: string | null;
}

export interface IdentifyResponse {
  status: IdentificationStatus;
  message: string;
  input: CodeInput;
  existing_product: ProductListItem | null;
  suggestion: ProductSuggestion | null;
  providers: ProviderRecord[];
  from_cache: boolean;
  looked_up_at: string;
}

export interface ScanResponse {
  status: "found" | "not_found" | "invalid_code" | "unsupported_format";
  message: string;
  input: CodeInput;
  product: ProductListItem | null;
}

export interface CategoryView {
  id: string;
  name: string;
  is_active: boolean;
  product_count: number;
  updated_at: string;
}

export interface MovementView {
  id: string;
  product_id: string;
  product_name: string;
  product_sku: string | null;
  quantity_delta: number;
  quantity_after: number;
  reason: string;
  note: string | null;
  created_by_name: string | null;
  created_at: string;
}

export interface AdjustmentResponse {
  movement: MovementView;
  product_id: string;
  stock_quantity: number;
}

export interface SaleSummary {
  id: string;
  receipt_number: string;
  total_idr: number;
  paid_idr: number;
  change_idr: number;
  item_quantity: number;
  payment_methods: string[];
  cashier_name: string | null;
  created_at: string;
}

export interface SalesPage {
  data: SaleSummary[];
  pagination: { limit: number; offset: number; total: number };
  summary: { transaction_count: number; revenue_idr: number };
}

export interface SaleItemView {
  product_id: string | null;
  product_name: string;
  sku: string | null;
  unit: string;
  unit_price_idr: number;
  quantity: number;
  discount_idr: number;
  line_total_idr: number;
}

export interface SaleView {
  id: string;
  receipt_number: string;
  cashier_name: string | null;
  subtotal_idr: number;
  item_discount_idr: number;
  sale_discount_idr: number;
  tax_rate_bps: number;
  tax_idr: number;
  total_idr: number;
  paid_idr: number;
  change_idr: number;
  created_at: string;
  items: SaleItemView[];
  payments: { method: string; amount_idr: number; reference: string | null }[];
  store: { name: string; address: string | null; phone: string | null; receipt_footer: string | null };
}

export interface DashboardSummary {
  timezone: string;
  date: string;
  today: {
    transaction_count: number;
    revenue_idr: number;
    average_ticket_idr: number;
    items_sold: number;
  };
  last_7_days: { date: string; transaction_count: number; revenue_idr: number }[];
  low_stock_count: number;
  low_stock_items: ProductListItem[];
  recent_sales: SaleSummary[];
}

export interface ExternalSource {
  code: string;
  name: string;
  description: string | null;
  enabled: boolean;
  priority: number;
  requires_credentials: boolean;
  rate_limit_per_minute: number;
}
