import type { ProductSuggestion } from "@/lib/api/types";
import type { ProductFormValues } from "@/lib/schemas";

/**
 * Maps an identification suggestion onto the product form.
 *
 * Only fields the provider actually supplied are written. Prices, stock, SKU, category and
 * the low-stock threshold are merchant-owned and are never touched by external data.
 * A provider's "verified" status is shown as "reported" here: merchants cannot set verified,
 * and only an official source can confirm it.
 */
export function suggestionToFormPatch(suggestion: ProductSuggestion): Partial<ProductFormValues> {
  const patch: Partial<ProductFormValues> = {};
  if (suggestion.name) patch.name = suggestion.name;
  if (suggestion.brand) patch.brand = suggestion.brand;
  if (suggestion.manufacturer) patch.manufacturer = suggestion.manufacturer;
  if (suggestion.description) patch.description = suggestion.description;
  if (suggestion.unit_hint && suggestion.unit_hint.length <= 20) patch.unit = suggestion.unit_hint;
  if (suggestion.image_url) patch.imageUrl = suggestion.image_url;
  if (suggestion.registration) {
    if (suggestion.registration.number) patch.registrationNumber = suggestion.registration.number;
    patch.registrationStatus = suggestion.registration.number ? "reported" : "unknown";
  }
  return patch;
}

/** The field labels a suggestion supplied, for display ("Brand, Manufacturer, ..."). */
export const SUGGESTION_FIELD_LABELS: Record<string, string> = {
  name: "Name",
  brand: "Brand",
  manufacturer: "Manufacturer",
  description: "Description",
  unit_hint: "Unit",
  image_url: "Image",
  registration: "Registration",
};
