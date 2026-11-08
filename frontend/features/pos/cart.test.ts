import { beforeEach, describe, expect, it } from "vitest";
import { computeTotals, useCartStore, type CartLine } from "./cart";

const coffee = { id: "p1", name: "Kopi", sku: null, unit: "cup", selling_price_idr: 6000, stock_quantity: 3 };

function line(overrides: Partial<CartLine> = {}): CartLine {
  return { productId: "p", name: "X", sku: null, unit: "pcs", unitPriceIdr: 10_000, quantity: 1, stock: 99, ...overrides };
}

describe("computeTotals", () => {
  it("sums quantities without tax", () => {
    const totals = computeTotals([line({ unitPriceIdr: 12_500, quantity: 2 }), line({ unitPriceIdr: 3_000 })], 0, 0);
    expect(totals.subtotalIdr).toBe(28_000);
    expect(totals.totalIdr).toBe(28_000);
    expect(totals.itemCount).toBe(3);
  });

  it("applies the sale discount before tax and rounds tax half-up", () => {
    // 15,000 after discount; 11% = 1,650 exactly.
    const totals = computeTotals([line({ unitPriceIdr: 20_000 })], 5_000, 1_100);
    expect(totals.taxableIdr).toBe(15_000);
    expect(totals.taxIdr).toBe(1_650);
    expect(totals.totalIdr).toBe(16_650);
  });

  it("never discounts more than the subtotal", () => {
    expect(computeTotals([line({ unitPriceIdr: 5_000 })], 999_999, 0).totalIdr).toBe(0);
  });
});

describe("cart store", () => {
  beforeEach(() => useCartStore.getState().clear());

  it("increments an existing line instead of duplicating it", () => {
    const { addProduct } = useCartStore.getState();
    expect(addProduct(coffee).ok).toBe(true);
    expect(addProduct(coffee).ok).toBe(true);
    const { lines } = useCartStore.getState();
    expect(lines).toHaveLength(1);
    expect(lines[0].quantity).toBe(2);
  });

  it("refuses to exceed the stock seen at scan time", () => {
    const { addProduct } = useCartStore.getState();
    addProduct(coffee, 3);
    const result = addProduct(coffee);
    expect(result.ok).toBe(false);
    expect(useCartStore.getState().lines[0].quantity).toBe(3);
  });

  it("removes a line when quantity is set below one", () => {
    useCartStore.getState().addProduct(coffee);
    useCartStore.getState().setQuantity("p1", 0);
    expect(useCartStore.getState().lines).toHaveLength(0);
  });

  it("clamps the sale discount to a non-negative whole number", () => {
    useCartStore.getState().setSaleDiscount(-500);
    expect(useCartStore.getState().saleDiscountIdr).toBe(0);
    useCartStore.getState().setSaleDiscount(1500.9);
    expect(useCartStore.getState().saleDiscountIdr).toBe(1500);
  });
});
