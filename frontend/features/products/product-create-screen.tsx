"use client";

import Link from "next/link";
import { useCallback, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { useCurrentStore, canManage as roleCanManage } from "@/features/auth/session";
import { ApiError } from "@/lib/api/client";
import { errorMessage } from "@/lib/api/errors";
import { useCategories, useCreateProduct, useIdentify } from "@/lib/api/queries";
import type { IdentifyResponse } from "@/lib/api/types";
import { emptyProductForm, productFormSchema, toCreatePayload, type ProductFormValues } from "@/lib/schemas";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, PageHeader } from "@/components/ui/card";
import { CameraScannerDialog, isCameraScanSupported } from "@/components/camera-scanner";
import { Spinner } from "@/components/ui/empty-state";
import { IdentifyPanel } from "./identify-panel";
import { ProductFields } from "./product-fields";
import { suggestionToFormPatch } from "./suggestion-form";

const NOT_SAVEABLE_CODE_STATUSES = new Set(["invalid_code", "unsupported_format"]);

/**
 * Product creation: SCAN -> IDENTIFY -> REVIEW -> SET PRICE -> SAVE.
 * Lookup results only pre-fill the form. Nothing is saved until the merchant presses Save.
 */
export function ProductCreateScreen() {
  const router = useRouter();
  const store = useCurrentStore();
  const allowed = store ? roleCanManage(store.role) : false;
  const categories = useCategories();
  const identify = useIdentify();
  const createProduct = useCreateProduct();
  const scanRef = useRef<HTMLInputElement>(null);

  const form = useForm<ProductFormValues>({
    resolver: zodResolver(productFormSchema),
    defaultValues: emptyProductForm,
  });

  const [lookup, setLookup] = useState<{ code: string; result: IdentifyResponse } | null>(null);
  const [lookupError, setLookupError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<{ message: string; existingProductId?: string } | null>(null);
  const [cameraOpen, setCameraOpen] = useState(false);
  const [cameraSupported] = useState(isCameraScanSupported);

  const runLookup = useCallback(
    async (code: string) => {
      setLookupError(null);
      setSaveError(null);
      try {
        const result = await identify.mutateAsync(code);
        setLookup({ code, result });
        if (result.suggestion) {
          // Merge, so anything the merchant typed in other fields is kept.
          form.reset({ ...form.getValues(), ...suggestionToFormPatch(result.suggestion) });
        }
      } catch (error) {
        setLookupError(errorMessage(error, "The lookup failed. Check the code and try again."));
      }
    },
    [form, identify],
  );

  const onCameraDetected = useCallback(
    (code: string) => {
      setCameraOpen(false);
      void runLookup(code);
    },
    [runLookup],
  );

  async function onSubmit(values: ProductFormValues) {
    setSaveError(null);
    const result = lookup?.result;
    const isExisting = result?.status === "existing_product";
    if (isExisting) return;

    const identifiers =
      lookup && result && !NOT_SAVEABLE_CODE_STATUSES.has(result.status)
        ? [{ code: lookup.code, is_primary: true }]
        : [];
    const externalSourceCode = result?.suggestion?.primary_source || null;

    try {
      const product = await createProduct.mutateAsync(
        toCreatePayload(values, { identifiers, externalSourceCode }),
      );
      router.push(`/products/${product.id}?created=1`);
    } catch (error) {
      if (error instanceof ApiError && error.code === "duplicate_identifier") {
        const existing = (error.details as { existing_product_id?: string } | null)?.existing_product_id;
        setSaveError({ message: error.message, existingProductId: existing });
        return;
      }
      setSaveError({ message: errorMessage(error) });
    }
  }

  if (store && !allowed) {
    return (
      <div className="flex flex-col gap-6">
        <PageHeader title="Add product" />
        <Alert tone="warning" title="Managers only">
          Ask a store manager or owner to add products. You can still find products in the cashier screen.
        </Alert>
      </div>
    );
  }

  const isExisting = lookup?.result.status === "existing_product";
  const scanInvalid = lookup && NOT_SAVEABLE_CODE_STATUSES.has(lookup.result.status);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Add product"
        description="Scan the barcode first. Review the details that were found, set your prices, then save."
        actions={
          <Link href="/products" className="text-sm font-medium text-primary hover:underline">
            Back to products
          </Link>
        }
      />

      <div className="grid gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
        <Card className="h-fit p-5 lg:sticky lg:top-6">
          <p className="mb-4 text-xs font-semibold uppercase tracking-wide text-muted">1 · Identify</p>
          <IdentifyPanel
            ref={scanRef}
            result={lookup?.result ?? null}
            pending={identify.isPending}
            error={lookupError}
            onLookup={(code) => void runLookup(code)}
            onCameraOpen={() => setCameraOpen(true)}
            cameraSupported={cameraSupported}
            onRetry={() => lookup && void runLookup(lookup.code)}
          />
        </Card>

        <Card>
          <CardHeader
            title="2 · Review and set prices"
            description={
              lookup && !scanInvalid
                ? `Barcode ${lookup.code} will be saved with this product.`
                : "Without a barcode the product is saved as a manual entry."
            }
          />
          <form onSubmit={form.handleSubmit(onSubmit)} noValidate className="flex flex-col gap-6 p-5">
            {categories.isPending ? <Spinner label="Loading categories" /> : null}
            <ProductFields form={form} categories={categories.data ?? []} mode="create" />

            {isExisting ? (
              <Alert tone="warning">This barcode already belongs to a product in your store, so it cannot be saved again.</Alert>
            ) : null}
            {saveError ? (
              <Alert tone="danger">
                {saveError.message}
                {saveError.existingProductId ? (
                  <>
                    {" "}
                    <Link className="font-medium underline" href={`/products/${saveError.existingProductId}`}>
                      Open the existing product
                    </Link>
                    .
                  </>
                ) : null}
              </Alert>
            ) : null}

            <div className="flex flex-wrap items-center justify-end gap-3 border-t border-border pt-5">
              <Button variant="ghost" onClick={() => router.push("/products")}>
                Cancel
              </Button>
              <Button type="submit" disabled={createProduct.isPending || isExisting} size="lg">
                {createProduct.isPending ? "Saving…" : "Save product"}
              </Button>
            </div>
          </form>
        </Card>
      </div>

      <CameraScannerDialog open={cameraOpen} onClose={() => setCameraOpen(false)} onDetected={onCameraDetected} />
    </div>
  );
}
