import { create } from "zustand";

/**
 * POS cart. The backend recalculates every amount at checkout. The totals here only
 * preview what the cashier will be asked to pay, using the same rounding as the server.
 */

export interface CartLine {
  productId: string;
  name: string;
  sku: string | null;
  unit: string;
  unitPriceIdr: number;
  quantity: number;
  stock: number;
}

export interface CartProductInput {
  id: string;
  name: string;
  sku: string | null;
  unit: string;
  selling_price_idr: number;
  stock_quantity: number;
}

export interface CartTotals {
  subtotalIdr: number;
  saleDiscountIdr: number;
  taxableIdr: number;
  taxIdr: number;
  totalIdr: number;
  itemCount: number;
}

export function computeTotals(lines: CartLine[], saleDiscountIdr: number, taxRateBps: number): CartTotals {
  const subtotal = lines.reduce((sum, line) => sum + line.unitPriceIdr * line.quantity, 0);
  const discount = Math.min(Math.max(0, Math.trunc(saleDiscountIdr) || 0), subtotal);
  const taxable = subtotal - discount;
  // Half-up rounding in integers, identical to backend/src/money.rs `tax_on`.
  const tax = Math.floor((taxable * taxRateBps + 5000) / 10000);
  return {
    subtotalIdr: subtotal,
    saleDiscountIdr: discount,
    taxableIdr: taxable,
    taxIdr: tax,
    totalIdr: taxable + tax,
    itemCount: lines.reduce((sum, line) => sum + line.quantity, 0),
  };
}

type AddResult = { ok: true } | { ok: false; reason: string };

interface CartState {
  lines: CartLine[];
  saleDiscountIdr: number;
  addProduct: (product: CartProductInput, quantity?: number) => AddResult;
  setQuantity: (productId: string, quantity: number) => AddResult;
  removeLine: (productId: string) => void;
  setSaleDiscount: (amount: number) => void;
  clear: () => void;
}

export const useCartStore = create<CartState>()((set, get) => ({
  lines: [],
  saleDiscountIdr: 0,

  addProduct(product, quantity = 1) {
    const existing = get().lines.find((line) => line.productId === product.id);
    const next = (existing?.quantity ?? 0) + quantity;
    if (product.stock_quantity < next) {
      return {
        ok: false,
        reason: `Only ${product.stock_quantity} ${product.unit} of ${product.name} in stock.`,
      };
    }
    if (existing) {
      set({
        lines: get().lines.map((line) =>
          line.productId === product.id ? { ...line, quantity: next, stock: product.stock_quantity } : line,
        ),
      });
    } else {
      set({
        lines: [
          ...get().lines,
          {
            productId: product.id,
            name: product.name,
            sku: product.sku,
            unit: product.unit,
            unitPriceIdr: product.selling_price_idr,
            quantity: next,
            stock: product.stock_quantity,
          },
        ],
      });
    }
    return { ok: true };
  },

  setQuantity(productId, quantity) {
    const line = get().lines.find((l) => l.productId === productId);
    if (!line) return { ok: false, reason: "Item is not in the cart." };
    if (!Number.isInteger(quantity) || quantity < 1) {
      get().removeLine(productId);
      return { ok: true };
    }
    if (quantity > line.stock) {
      return { ok: false, reason: `Only ${line.stock} ${line.unit} of ${line.name} in stock.` };
    }
    set({
      lines: get().lines.map((l) => (l.productId === productId ? { ...l, quantity } : l)),
    });
    return { ok: true };
  },

  removeLine(productId) {
    set({ lines: get().lines.filter((line) => line.productId !== productId) });
  },

  setSaleDiscount(amount) {
    set({ saleDiscountIdr: Math.max(0, Math.trunc(amount) || 0) });
  },

  clear() {
    set({ lines: [], saleDiscountIdr: 0 });
  },
}));
