import { describe, expect, it } from "vitest";
import { suggestionToFormPatch } from "./suggestion-form";
import type { ProductSuggestion } from "@/lib/api/types";

const base: ProductSuggestion = {
  fields: [],
  sources: [{ code: "mock", name: "Mock", confidence: "low" }],
  primary_source: "mock",
  confidence: "low",
  fetched_at: "2026-11-08T10:00:00Z",
};

describe("suggestionToFormPatch", () => {
  it("writes only the fields the provider supplied", () => {
    const patch = suggestionToFormPatch({ ...base, name: "Air Mineral", brand: "Acme" });
    expect(patch).toEqual({ name: "Air Mineral", brand: "Acme" });
  });

  it("never sets merchant-owned pricing, stock, SKU or category", () => {
    const patch = suggestionToFormPatch({
      ...base,
      name: "Kopi",
      // A provider has no business sending these, but if it does they must be ignored.
      ...({ selling_price_idr: 1, stock_quantity: 999, sku: "HACK", category_id: "x" } as object),
    });
    expect(Object.keys(patch).sort()).toEqual(["name"]);
  });

  it("downgrades verified registrations to reported", () => {
    const patch = suggestionToFormPatch({
      ...base,
      registration: { number: "REG-1", status: "verified", valid_until: null, verified_at: null },
    });
    expect(patch.registrationNumber).toBe("REG-1");
    expect(patch.registrationStatus).toBe("reported");
  });

  it("ignores unit hints that are too long to fit the unit field", () => {
    const patch = suggestionToFormPatch({ ...base, unit_hint: "a-very-long-unit-name-indeed" });
    expect(patch.unit).toBeUndefined();
  });
});
