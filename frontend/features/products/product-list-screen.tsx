"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { Plus } from "lucide-react";
import { useCurrentStore } from "@/features/auth/session";
import { useCategories, useProducts } from "@/lib/api/queries";
import { formatIdr, formatNumber } from "@/lib/format";
import { useDebouncedValue } from "@/lib/hooks";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { EmptyState, Spinner } from "@/components/ui/empty-state";
import { PageHeader } from "@/components/ui/card";
import { SelectField, TextField } from "@/components/ui/field";
import { canManage } from "@/features/auth/session";

const PAGE_SIZE = 25;

export function ProductListScreen() {
  const router = useRouter();
  const store = useCurrentStore();
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<"active" | "inactive" | "all">("active");
  const [lowStock, setLowStock] = useState(false);
  const [categoryId, setCategoryId] = useState("");
  const [offset, setOffset] = useState(0);
  const debouncedSearch = useDebouncedValue(search.trim(), 300);

  const categories = useCategories();
  const products = useProducts({
    q: debouncedSearch || undefined,
    status,
    low_stock: lowStock || undefined,
    category_id: categoryId || undefined,
    limit: PAGE_SIZE,
    offset,
  });

  const total = products.data?.pagination.total ?? 0;
  const from = total === 0 ? 0 : offset + 1;
  const to = Math.min(offset + PAGE_SIZE, total);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Products"
        description="Search by name, brand, SKU or barcode."
        actions={
          store && canManage(store.role) ? (
            <Link href="/products/new" className="inline-flex h-10 items-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary-hover">
              <Plus className="size-4" aria-hidden="true" />
              Add product
            </Link>
          ) : null
        }
      />

      <Card className="p-4">
        <div className="grid gap-4 md:grid-cols-[minmax(0,2fr)_repeat(3,minmax(0,1fr))] md:items-end">
          <TextField
            label="Search"
            placeholder="Name, brand, SKU or barcode"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setOffset(0);
            }}
            type="search"
            autoComplete="off"
          />
          <SelectField
            label="Status"
            value={status}
            onChange={(e) => {
              setStatus(e.target.value as typeof status);
              setOffset(0);
            }}
          >
            <option value="active">Active</option>
            <option value="inactive">Inactive</option>
            <option value="all">All</option>
          </SelectField>
          <SelectField
            label="Category"
            value={categoryId}
            onChange={(e) => {
              setCategoryId(e.target.value);
              setOffset(0);
            }}
          >
            <option value="">All categories</option>
            {(categories.data ?? []).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </SelectField>
          <label className="flex h-10 items-center gap-2 text-sm font-medium">
            <input
              type="checkbox"
              checked={lowStock}
              onChange={(e) => {
                setLowStock(e.target.checked);
                setOffset(0);
              }}
              className="size-4 accent-primary"
            />
            Low stock only
          </label>
        </div>
      </Card>

      {products.isError ? <Alert tone="danger">The product list could not be loaded. Refresh and try again.</Alert> : null}

      <Card className="overflow-hidden">
        {products.isPending ? (
          <Spinner label="Loading products" />
        ) : products.data && products.data.data.length === 0 ? (
          <EmptyState
            title="No products match these filters"
            description={store && canManage(store.role) ? "Scan a barcode to add your first product." : "Try a different search."}
            action={
              store && canManage(store.role) ? (
                <Button onClick={() => router.push("/products/new")}>Add product</Button>
              ) : null
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[640px] text-left text-sm">
              <thead className="bg-surface-muted text-xs uppercase tracking-wide text-muted">
                <tr>
                  <th className="px-4 py-3 font-medium">Product</th>
                  <th className="px-4 py-3 font-medium">Category</th>
                  <th className="px-4 py-3 text-right font-medium">Stock</th>
                  <th className="px-4 py-3 text-right font-medium">Selling price</th>
                  <th className="px-4 py-3 font-medium">Source</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {products.data?.data.map((product) => (
                  <tr key={product.id} className="cursor-pointer hover:bg-surface-muted/60" onClick={() => router.push(`/products/${product.id}`)}>
                    <td className="px-4 py-3">
                      <Link href={`/products/${product.id}`} className="font-medium text-foreground hover:text-primary">
                        {product.name}
                      </Link>
                      <div className="mt-0.5 flex flex-wrap gap-x-3 text-xs text-muted">
                        {product.brand ? <span>{product.brand}</span> : null}
                        {product.primary_barcode ? <span className="font-mono">{product.primary_barcode}</span> : null}
                        {product.sku ? <span className="font-mono">SKU {product.sku}</span> : null}
                        {!product.is_active ? <Badge tone="neutral">Inactive</Badge> : null}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-muted">{product.category?.name ?? "—"}</td>
                    <td className="px-4 py-3 text-right">
                      <span className="font-medium">{formatNumber(product.stock_quantity)}</span>{" "}
                      <span className="text-muted">{product.unit}</span>
                      {product.is_low_stock ? (
                        <div className="mt-1">
                          <Badge tone="warning">Low stock</Badge>
                        </div>
                      ) : null}
                    </td>
                    <td className="px-4 py-3 text-right font-medium">{formatIdr(product.selling_price_idr)}</td>
                    <td className="px-4 py-3">
                      <Badge tone={product.data_source === "manual" ? "neutral" : "info"}>{product.data_source}</Badge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {total > 0 ? (
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-4 py-3 text-sm">
            <span className="text-muted">
              Showing {formatNumber(from)}–{formatNumber(to)} of {formatNumber(total)}
            </span>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}>
                Previous
              </Button>
              <Button variant="outline" size="sm" disabled={offset + PAGE_SIZE >= total} onClick={() => setOffset(offset + PAGE_SIZE)}>
                Next
              </Button>
            </div>
          </div>
        ) : null}
      </Card>
    </div>
  );
}
