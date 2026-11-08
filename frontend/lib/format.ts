const idrFormatter = new Intl.NumberFormat("id-ID", {
  style: "currency",
  currency: "IDR",
  maximumFractionDigits: 0,
});

const numberFormatter = new Intl.NumberFormat("id-ID");

/** "Rp 12.500" — whole rupiah only, never floating point. */
export function formatIdr(amount: number): string {
  return idrFormatter.format(Math.trunc(amount));
}

export function formatNumber(value: number): string {
  return numberFormatter.format(value);
}

/** Basis points to a percentage string: 1100 -> "11%", 1050 -> "10.5%". */
export function formatPercentBps(bps: number): string {
  const percent = bps / 100;
  return `${Number.isInteger(percent) ? percent : percent.toFixed(2)}%`;
}

export function formatDateTime(iso: string, timeZone?: string): string {
  return new Intl.DateTimeFormat("en-GB", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    timeZone,
  }).format(new Date(iso));
}

export function formatShortDate(isoDate: string): string {
  // Dates from the API are store-local "YYYY-MM-DD". Parse as UTC to avoid timezone shifts.
  const [y, m, d] = isoDate.split("-").map(Number);
  return new Intl.DateTimeFormat("en-GB", { day: "2-digit", month: "short", timeZone: "UTC" }).format(
    new Date(Date.UTC(y, m - 1, d)),
  );
}

/** Keeps only digits from user input and returns the integer, or null when empty. */
export function parseIdrInput(text: string): number | null {
  const digits = text.replace(/[^\d]/g, "");
  if (digits === "") return null;
  return Number(digits);
}

/** Digits-only text for money inputs (no separators, no decimals). */
export function sanitizeDigits(text: string): string {
  return text.replace(/[^\d]/g, "");
}

export const REASON_LABELS: Record<string, string> = {
  initial_stock: "Initial stock",
  restock: "Restock",
  sale: "Sale",
  return: "Customer return",
  damage: "Damaged",
  loss: "Lost / missing",
  correction: "Correction",
  count_adjustment: "Stock count",
  other: "Other",
};

export const PAYMENT_LABELS: Record<string, string> = {
  cash: "Cash",
  qris: "QRIS",
  debit_card: "Debit card",
  credit_card: "Credit card",
  bank_transfer: "Bank transfer",
  ewallet: "E-wallet",
};
