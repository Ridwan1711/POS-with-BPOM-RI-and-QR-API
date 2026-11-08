import { z } from "zod";

const digits = /^\d*$/;
const orNull = (value: string) => (value.trim() === "" ? null : value.trim());

export const loginSchema = z.object({
  email: z.string().trim().email("Enter a valid email address."),
  password: z.string().min(1, "Enter your password."),
});
export type LoginValues = z.infer<typeof loginSchema>;

/** Numeric fields are kept as digit strings in the form and converted on submit. */
export const productFormSchema = z.object({
  name: z.string().trim().min(1, "Product name is required.").max(200, "Keep the name under 200 characters."),
  brand: z.string().trim().max(120, "Brand must be 120 characters or fewer."),
  manufacturer: z.string().trim().max(120, "Manufacturer must be 120 characters or fewer."),
  description: z.string().trim().max(2000, "Description must be 2000 characters or fewer."),
  categoryId: z.string(),
  sku: z.string().trim().max(64, "SKU must be 64 characters or fewer."),
  unit: z.string().trim().min(1, "Unit is required.").max(20, "Unit must be 20 characters or fewer."),
  purchasePrice: z.string().regex(digits, "Use whole rupiah only, without separators."),
  sellingPrice: z
    .string()
    .regex(/^\d+$/, "Enter the selling price in whole rupiah.")
    .refine((value) => Number(value) > 0, "Selling price must be more than zero."),
  initialStock: z.string().regex(digits, "Use a whole number."),
  lowStockThreshold: z.string().regex(digits, "Use a whole number."),
  registrationNumber: z.string().trim().max(100, "Registration number must be 100 characters or fewer."),
  registrationStatus: z.enum(["unknown", "reported"]),
  imageUrl: z
    .string()
    .trim()
    .max(2048)
    .refine((value) => value === "" || value.startsWith("https://"), "The image link must start with https://."),
});
export type ProductFormValues = z.infer<typeof productFormSchema>;

export const emptyProductForm: ProductFormValues = {
  name: "",
  brand: "",
  manufacturer: "",
  description: "",
  categoryId: "",
  sku: "",
  unit: "pcs",
  purchasePrice: "",
  sellingPrice: "",
  initialStock: "0",
  lowStockThreshold: "",
  registrationNumber: "",
  registrationStatus: "unknown",
  imageUrl: "",
};

export interface CreateProductPayload {
  name: string;
  description: string | null;
  brand: string | null;
  manufacturer: string | null;
  category_id: string | null;
  sku: string | null;
  unit: string;
  purchase_price_idr: number;
  selling_price_idr: number;
  initial_stock: number;
  low_stock_threshold: number | null;
  image_url: string | null;
  registration_number: string | null;
  registration_status: "unknown" | "reported";
  identifiers: { code: string; is_primary: boolean }[];
  external_source_code: string | null;
}

export function toCreatePayload(
  values: ProductFormValues,
  extras: { identifiers: { code: string; is_primary: boolean }[]; externalSourceCode: string | null },
): CreateProductPayload {
  return {
    name: values.name.trim(),
    description: orNull(values.description),
    brand: orNull(values.brand),
    manufacturer: orNull(values.manufacturer),
    category_id: values.categoryId || null,
    sku: orNull(values.sku),
    unit: values.unit.trim(),
    purchase_price_idr: Number(values.purchasePrice || "0"),
    selling_price_idr: Number(values.sellingPrice),
    initial_stock: Number(values.initialStock || "0"),
    low_stock_threshold: values.lowStockThreshold === "" ? null : Number(values.lowStockThreshold),
    image_url: orNull(values.imageUrl),
    registration_number: orNull(values.registrationNumber),
    registration_status: values.registrationStatus,
    identifiers: extras.identifiers,
    external_source_code: extras.externalSourceCode,
  };
}

export const adjustmentReasons = ["restock", "return", "damage", "loss", "correction", "count_adjustment", "other"] as const;

export const adjustmentSchema = z.object({
  direction: z.enum(["add", "remove"]),
  quantity: z.string().regex(/^[1-9]\d*$/, "Enter a whole number greater than zero."),
  reason: z.enum(adjustmentReasons),
  note: z.string().trim().max(500, "Note must be 500 characters or fewer."),
});
export type AdjustmentValues = z.infer<typeof adjustmentSchema>;

export const categorySchema = z.object({
  name: z.string().trim().min(1, "Category name is required.").max(100, "Keep the name under 100 characters."),
});
export type CategoryValues = z.infer<typeof categorySchema>;

export const settingsSchema = z.object({
  name: z.string().trim().min(1, "Store name is required.").max(120),
  address: z.string().trim().max(500),
  phone: z.string().trim().max(40),
  timezone: z.string().trim().min(1, "Choose a time zone."),
  taxPercent: z
    .string()
    .regex(/^\d{1,2}(\.\d{1,2})?$/, "Enter a percentage such as 0 or 11.")
    .refine((value) => Number(value) <= 30, "Tax above 30% is not allowed."),
  allowNegativeStock: z.boolean(),
  defaultLowStockThreshold: z.string().regex(/^\d+$/, "Use a whole number."),
  receiptFooter: z.string().trim().max(300),
});
export type SettingsValues = z.infer<typeof settingsSchema>;
