"use client";

import type { UseFormReturn } from "react-hook-form";
import type { CategoryView } from "@/lib/api/types";
import type { ProductFormValues } from "@/lib/schemas";
import { SelectField, TextAreaField, TextField } from "@/components/ui/field";

/** Product form fields, shared by the create and edit screens. Stock is only entered at creation. */
export function ProductFields({
  form,
  categories,
  mode,
}: {
  form: UseFormReturn<ProductFormValues>;
  categories: CategoryView[];
  mode: "create" | "edit";
}) {
  const { register, formState } = form;
  const errors = formState.errors;
  const activeCategories = categories.filter((c) => c.is_active);

  return (
    <div className="grid gap-x-4 gap-y-5 md:grid-cols-2">
      <TextField
        containerClassName="md:col-span-2"
        label="Product name"
        autoComplete="off"
        {...register("name")}
        error={errors.name?.message}
      />
      <TextField label="Brand" autoComplete="off" {...register("brand")} error={errors.brand?.message} />
      <TextField label="Manufacturer" autoComplete="off" {...register("manufacturer")} error={errors.manufacturer?.message} />
      <SelectField label="Category" {...register("categoryId")} error={errors.categoryId?.message}>
        <option value="">No category</option>
        {activeCategories.map((category) => (
          <option key={category.id} value={category.id}>
            {category.name}
          </option>
        ))}
      </SelectField>
      <TextField
        label="SKU (internal code)"
        hint="Optional. Stored in upper case and unique within your store."
        autoComplete="off"
        {...register("sku")}
        error={errors.sku?.message}
      />
      <TextField label="Unit" hint="For example pcs, box, bottle, kg" autoComplete="off" {...register("unit")} error={errors.unit?.message} />
      <TextField
        label="Purchase price (IDR)"
        hint="What you pay the supplier. Only you can change this."
        inputMode="numeric"
        {...register("purchasePrice")}
        error={errors.purchasePrice?.message}
      />
      <TextField
        label="Selling price (IDR)"
        inputMode="numeric"
        {...register("sellingPrice")}
        error={errors.sellingPrice?.message}
      />
      {mode === "create" ? (
        <TextField
          label="Initial stock"
          hint="Units on hand today. Recorded in the stock history."
          inputMode="numeric"
          {...register("initialStock")}
          error={errors.initialStock?.message}
        />
      ) : null}
      <TextField
        label="Low stock alert at"
        hint="Leave empty to use the store default."
        inputMode="numeric"
        {...register("lowStockThreshold")}
        error={errors.lowStockThreshold?.message}
      />
      <TextField
        containerClassName="md:col-span-2"
        label="Image link (https)"
        hint="Optional. Only https:// links are accepted. The image is not downloaded automatically."
        type="url"
        {...register("imageUrl")}
        error={errors.imageUrl?.message}
      />
      <TextAreaField containerClassName="md:col-span-2" label="Description" {...register("description")} error={errors.description?.message} />
      <div className="grid gap-x-4 gap-y-5 md:col-span-2 md:grid-cols-2">
        <TextField
          label="BPOM registration number"
          hint="Enter only what is printed on the pack, or what a source reported."
          autoComplete="off"
          {...register("registrationNumber")}
          error={errors.registrationNumber?.message}
        />
        <SelectField label="Registration status" {...register("registrationStatus")} error={errors.registrationStatus?.message}>
          <option value="unknown">Unknown or not checked</option>
          <option value="reported">Reported by source (not verified)</option>
        </SelectField>
      </div>
    </div>
  );
}
