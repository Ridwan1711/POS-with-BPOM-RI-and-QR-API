"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { canManage, useCurrentStore } from "@/features/auth/session";
import { ApiError } from "@/lib/api/client";
import { errorMessage } from "@/lib/api/errors";
import { useAddIdentifier, useCategories, useProduct, useRemoveIdentifier, useUpdateProduct, type ProductUpdate } from "@/lib/api/queries";
import type { ProductDetail } from "@/lib/api/types";
import { formatDateTime, formatIdr, formatNumber, formatShortDate } from "@/lib/format";
import { emptyProductForm, productFormSchema, type ProductFormValues } from "@/lib/schemas";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, PageHeader } from "@/components/ui/card";
import { Spinner } from "@/components/ui/empty-state";
import { TextField } from "@/components/ui/field";
import { ProductFields } from "./product-fields";

function toFormValues(product: ProductDetail): ProductFormValues {
  return {
    ...emptyProductForm,
    name: product.name,
    brand: product.brand ?? "",
    manufacturer: product.manufacturer ?? "",
    description: product.description ?? "",
    categoryId: product.category?.id ?? "",
    sku: product.sku ?? "",
    unit: product.unit,
    purchasePrice: String(product.purchase_price_idr),
    sellingPrice: String(product.selling_price_idr),
    lowStockThreshold: product.low_stock_threshold_override === null ? "" : String(product.low_stock_threshold_override),
    registrationNumber: product.bpom_registration_number ?? "",
    registrationStatus: product.registration_status === "reported" ? "reported" : "unknown",
    imageUrl: product.image_url ?? "",
  };
}

export function ProductDetailScreen({ productId }: { productId: string }) {
  const store = useCurrentStore();
  const editable = store ? canManage(store.role) : false;
  const searchParams = useSearchParams();
  const created = searchParams.get("created") === "1";
  const product = useProduct(productId);
  const categories = useCategories();
  const update = useUpdateProduct(productId);
  const addIdentifier = useAddIdentifier(productId);
  const removeIdentifier = useRemoveIdentifier(productId);
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [newCode, setNewCode] = useState("");
  const [identifierError, setIdentifierError] = useState<string | null>(null);

  const form = useForm<ProductFormValues>({
    resolver: zodResolver(productFormSchema),
    defaultValues: emptyProductForm,
  });

  useEffect(() => {
    if (product.data) form.reset(toFormValues(product.data));
  }, [product.data, form]);

  if (product.isPending) return <Spinner label="Loading product" />;
  if (product.isError || !product.data) {
    return <Alert tone="danger">This product could not be found in your store.</Alert>;
  }
  const item = product.data;

  async function onSubmit(values: ProductFormValues) {
    setSaved(false);
    setSaveError(null);
    const body: ProductUpdate = {
      name: values.name.trim(),
      brand: values.brand.trim() || null,
      manufacturer: values.manufacturer.trim() || null,
      description: values.description.trim() || null,
      sku: values.sku.trim() || null,
      category_id: values.categoryId || null,
      unit: values.unit.trim(),
      purchase_price_idr: Number(values.purchasePrice || "0"),
      selling_price_idr: Number(values.sellingPrice),
      low_stock_threshold: values.lowStockThreshold === "" ? null : Number(values.lowStockThreshold),
      image_url: values.imageUrl.trim() || null,
      bpom_registration_number: values.registrationNumber.trim() || null,
      registration_status: values.registrationStatus,
    };
    try {
      await update.mutateAsync(body);
      setSaved(true);
    } catch (error) {
      setSaveError(errorMessage(error));
    }
  }

  async function toggleActive() {
    setSaveError(null);
    try {
      await update.mutateAsync({ is_active: !item.is_active });
    } catch (error) {
      setSaveError(errorMessage(error));
    }
  }

  async function onAddIdentifier() {
    const code = newCode.trim();
    if (!code) return;
    setIdentifierError(null);
    try {
      await addIdentifier.mutateAsync(code);
      setNewCode("");
    } catch (error) {
      setIdentifierError(
        error instanceof ApiError ? error.message : errorMessage(error),
      );
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={item.name}
        description={[item.brand, item.category?.name].filter(Boolean).join(" · ") || "Product details"}
        actions={
          <>
            <Link
              href={`/inventory?adjust=${item.id}`}
              className="inline-flex h-9 items-center rounded-md border border-border-strong px-3 text-sm font-medium hover:bg-surface-muted"
            >
              Adjust stock
            </Link>
            <Link href="/products" className="inline-flex h-9 items-center px-3 text-sm font-medium text-primary hover:underline">
              All products
            </Link>
          </>
        }
      />

      {created ? <Alert tone="success" title="Product saved">It is now available in the cashier screen.</Alert> : null}
      {saved ? <Alert tone="success">Changes saved.</Alert> : null}
      {saveError ? <Alert tone="danger">{saveError}</Alert> : null}

      <div className="grid gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <Card>
          <CardHeader
            title="Details"
            description={`Last updated ${formatDateTime(item.updated_at)}`}
            actions={
              <>
                <Badge tone={item.is_active ? "success" : "neutral"}>{item.is_active ? "Active" : "Inactive"}</Badge>
                <Badge tone="neutral">Source: {item.data_source}</Badge>
              </>
            }
          />
          <form onSubmit={form.handleSubmit(onSubmit)} noValidate className="flex flex-col gap-6 p-5">
            <ProductFields form={form} categories={categories.data ?? []} mode="edit" />
            {editable ? (
              <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-5">
                <Button variant="ghost" className="text-danger" onClick={toggleActive} disabled={update.isPending}>
                  {item.is_active ? "Deactivate product" : "Reactivate product"}
                </Button>
                <Button type="submit" disabled={update.isPending} size="lg">
                  {update.isPending ? "Saving…" : "Save changes"}
                </Button>
              </div>
            ) : (
              <Alert tone="info">Only managers and owners can change product details.</Alert>
            )}
          </form>
        </Card>

        <div className="flex flex-col gap-6">
          <Card>
            <CardHeader title="Stock" description="Changes are recorded with a reason in the stock history." />
            <dl className="grid grid-cols-2 gap-4 p-5 text-sm">
              <div>
                <dt className="text-muted">On hand</dt>
                <dd className="mt-0.5 text-xl font-semibold">
                  {formatNumber(item.stock_quantity)} <span className="text-sm font-normal text-muted">{item.unit}</span>
                </dd>
              </div>
              <div>
                <dt className="text-muted">Low stock alert at</dt>
                <dd className="mt-0.5 text-xl font-semibold">{formatNumber(item.low_stock_threshold)}</dd>
              </div>
              <div>
                <dt className="text-muted">Purchase price</dt>
                <dd className="mt-0.5 font-medium">{formatIdr(item.purchase_price_idr)}</dd>
              </div>
              <div>
                <dt className="text-muted">Selling price</dt>
                <dd className="mt-0.5 font-medium">{formatIdr(item.selling_price_idr)}</dd>
              </div>
            </dl>
          </Card>

          <Card>
            <CardHeader title="Barcodes and codes" description="Each code identifies this product in the cashier screen." />
            <div className="flex flex-col gap-4 p-5">
              {item.identifiers.length === 0 ? <p className="text-sm text-muted">No barcode yet. This product is a manual entry.</p> : null}
              <ul className="flex flex-col divide-y divide-border">
                {item.identifiers.map((identifier) => (
                  <li key={identifier.id} className="flex items-center justify-between gap-3 py-2.5">
                    <div className="min-w-0">
                      <p className="font-mono text-sm">{identifier.raw_value}</p>
                      <p className="text-xs text-muted">
                        {identifier.kind.toUpperCase()}
                        {identifier.is_primary ? " · primary" : ""}
                      </p>
                    </div>
                    {editable ? (
                      <Button variant="ghost" size="sm" onClick={() => removeIdentifier.mutate(identifier.id)} disabled={removeIdentifier.isPending}>
                        Remove
                      </Button>
                    ) : null}
                  </li>
                ))}
              </ul>
              {editable ? (
                <form
                  className="flex gap-2"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void onAddIdentifier();
                  }}
                >
                  <div className="min-w-0 flex-1">
                    <TextField label="Add another code" value={newCode} onChange={(e) => setNewCode(e.target.value)} error={identifierError ?? undefined} autoComplete="off" />
                  </div>
                  <Button type="submit" variant="secondary" className="mt-6" disabled={addIdentifier.isPending || newCode.trim() === ""}>
                    Add
                  </Button>
                </form>
              ) : null}
            </div>
          </Card>

          <ProvenanceCard product={item} />
        </div>
      </div>
    </div>
  );
}

function ProvenanceCard({ product }: { product: ProductDetail }) {
  const hasRegistration = Boolean(product.bpom_registration_number);
  return (
    <Card>
      <CardHeader title="Source and registration" description="Where this product's details came from." />
      <div className="flex flex-col gap-4 p-5 text-sm">
        {product.external_records.length === 0 ? (
          <p className="text-muted">Entered manually. No external source was used to create this product.</p>
        ) : (
          product.external_records.map((record) => (
            <div key={record.source_code} className="rounded-md border border-border p-3">
              <p className="font-medium">
                Confirmed from <span className="font-mono">{record.source_code}</span>{" "}
                <Badge tone={record.confidence === "high" ? "success" : "warning"}>{record.confidence} confidence</Badge>
              </p>
              <p className="mt-1 text-xs text-muted">Fetched {formatDateTime(record.fetched_at)}</p>
              <p className="mt-2 text-xs text-muted">
                Stored separately from your prices and stock. Later lookups never overwrite the product.
              </p>
            </div>
          ))
        )}
        <div className="rounded-md border border-border p-3">
          <p className="font-medium">BPOM RI registration</p>
          {hasRegistration ? (
            <p className="mt-1">
              <span className="font-mono">{product.bpom_registration_number}</span> ·{" "}
              <Badge tone={product.registration_status === "verified" ? "success" : "warning"}>
                {product.registration_status === "verified"
                  ? "Verified"
                  : product.registration_status === "expired"
                    ? "Expired"
                    : "Reported, not verified"}
              </Badge>
            </p>
          ) : (
            <p className="mt-1 text-muted">No registration number recorded.</p>
          )}
          {product.registration_valid_until ? (
            <p className="mt-1 text-xs text-muted">Valid until {formatShortDate(product.registration_valid_until)}</p>
          ) : null}
          <p className="mt-2 text-xs text-muted">
            This app does not verify BPOM registrations. A barcode match is not proof of registration.
          </p>
        </div>
      </div>
    </Card>
  );
}
