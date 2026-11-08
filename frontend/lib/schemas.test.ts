import { describe, expect, it } from "vitest";
import { emptyProductForm, productFormSchema, toCreatePayload } from "./schemas";

describe("productFormSchema", () => {
  it("requires a name and a positive selling price", () => {
    const result = productFormSchema.safeParse({ ...emptyProductForm, sellingPrice: "0" });
    expect(result.success).toBe(false);
    const messages = result.success ? [] : result.error.issues.map((i) => i.message);
    expect(messages).toContain("Product name is required.");
    expect(messages).toContain("Selling price must be more than zero.");
  });

  it("rejects fractional or formatted money", () => {
    const result = productFormSchema.safeParse({ ...emptyProductForm, name: "A", sellingPrice: "12.500" });
    expect(result.success).toBe(false);
  });

  it("rejects non-https image links", () => {
    const result = productFormSchema.safeParse({ ...emptyProductForm, name: "A", sellingPrice: "1000", imageUrl: "http://x" });
    expect(result.success).toBe(false);
  });
});

describe("toCreatePayload", () => {
  it("converts digit strings to numbers and blanks to null", () => {
    const payload = toCreatePayload(
      { ...emptyProductForm, name: "  Gula  ", sellingPrice: "16500", purchasePrice: "13000", initialStock: "24", brand: "  " },
      { identifiers: [{ code: "8996001600016", is_primary: true }], externalSourceCode: null },
    );
    expect(payload.name).toBe("Gula");
    expect(payload.selling_price_idr).toBe(16500);
    expect(payload.purchase_price_idr).toBe(13000);
    expect(payload.initial_stock).toBe(24);
    expect(payload.brand).toBeNull();
    expect(payload.low_stock_threshold).toBeNull();
    expect(payload.identifiers).toHaveLength(1);
  });
});
