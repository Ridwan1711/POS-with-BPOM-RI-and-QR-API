"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { canManage, useCurrentStore } from "@/features/auth/session";
import { errorMessage } from "@/lib/api/errors";
import { useAdjustStock, useInventory, useMovements, useProduct } from "@/lib/api/queries";
import type { ProductListItem } from "@/lib/api/types";
import { formatDateTime, formatNumber, REASON_LABELS } from "@/lib/format";
import { adjustmentReasons, adjustmentSchema, type AdjustmentValues } from "@/lib/schemas";
import { useDebouncedValue } from "@/lib/hooks";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, PageHeader } from "@/components/ui/card";
import { Dialog } from "@/components/ui/dialog";
import { EmptyState, Spinner } from "@/components/ui/empty-state";
import { SelectField, TextAreaField, TextField } from "@/components/ui/field";
import { useRouter } from "next/navigation";

export function InventoryScreen() {
  const router = useRouter();
  const store = useCurrentStore();
  const editable = store ? canManage(store.role) : false;
  const searchParams = useSearchParams();
  const adjustId = searchParams.get("adjust");

  const [search, setSearch] = useState("");
  const [lowOnly, setLowOnly] = useState(false);
  const debounced = useDebouncedValue(search.trim(), 300);
  const inventory = useInventory({ q: debounced || undefined, low_stock: lowOnly || undefined, limit: 50 });
  const movements = useMovements({ limit: 15 });
  const [target, setTarget] = useState<ProductListItem | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!adjustId || !editable) return;
    setTarget(null);
  }, [adjustId, editable]);

  const targetFromUrl = useProduct(adjustId && editable ? adjustId : "");
  const dialogProduct = target ?? (targetFromUrl.data
    ? ({
        id: targetFromUrl.data.id,
        name: targetFromUrl.data.name,
        sku: targetFromUrl.data.sku,
        unit: targetFromUrl.data.unit,
        stock_quantity: targetFromUrl.data.stock_quantity,
      } as ProductListItem)
    : null);

  function closeDialog() {
    setTarget(null);
    if (adjustId) router.replace("/inventory");
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Inventory"
        description="Stock on hand, low-stock items and the history of every change."
      />

      {notice ? <Alert tone="success">{notice}</Alert> : null}

      <Card className="p-4">
        <div className="flex flex-wrap items-end gap-4">
          <div className="min-w-[220px] flex-1">
            <TextField label="Search" placeholder="Name, SKU or barcode" value={search} onChange={(e) => setSearch(e.target.value)} type="search" autoComplete="off" />
          </div>
          <label className="flex h-10 items-center gap-2 text-sm font-medium">
            <input type="checkbox" className="size-4 accent-primary" checked={lowOnly} onChange={(e) => setLowOnly(e.target.checked)} />
            Low stock only
          </label>
        </div>
      </Card>

      <Card className="overflow-hidden">
        {inventory.isPending ? (
          <Spinner label="Loading stock" />
        ) : inventory.data?.data.length === 0 ? (
          <EmptyState title="No products to show" description="Products you add appear here with their stock levels." />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[560px] text-left text-sm">
              <thead className="bg-surface-muted text-xs uppercase tracking-wide text-muted">
                <tr>
                  <th className="px-4 py-3 font-medium">Product</th>
                  <th className="px-4 py-3 text-right font-medium">On hand</th>
                  <th className="px-4 py-3 text-right font-medium">Alert at</th>
                  <th className="px-4 py-3 font-medium">Status</th>
                  <th className="px-4 py-3 text-right font-medium"><span className="sr-only">Actions</span></th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {inventory.data?.data.map((item) => (
                  <tr key={item.id}>
                    <td className="px-4 py-3">
                      <Link href={`/products/${item.id}`} className="font-medium hover:text-primary">
                        {item.name}
                      </Link>
                      <div className="text-xs text-muted">{item.sku ?? item.primary_barcode ?? "No code"}</div>
                    </td>
                    <td className="px-4 py-3 text-right font-semibold">
                      {formatNumber(item.stock_quantity)} <span className="font-normal text-muted">{item.unit}</span>
                    </td>
                    <td className="px-4 py-3 text-right text-muted">{formatNumber(item.low_stock_threshold)}</td>
                    <td className="px-4 py-3">
                      {item.stock_quantity <= 0 ? (
                        <Badge tone="danger">Out of stock</Badge>
                      ) : item.is_low_stock ? (
                        <Badge tone="warning">Low stock</Badge>
                      ) : (
                        <Badge tone="success">In stock</Badge>
                      )}
                    </td>
                    <td className="px-4 py-3 text-right">
                      {editable ? (
                        <Button variant="outline" size="sm" onClick={() => setTarget(item)}>
                          Adjust
                        </Button>
                      ) : null}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <Card>
        <CardHeader title="Recent stock changes" description="Every change has a reason and a person attached." />
        {movements.isPending ? (
          <Spinner />
        ) : movements.data?.data.length === 0 ? (
          <EmptyState title="No stock changes yet" />
        ) : (
          <ul className="divide-y divide-border">
            {movements.data?.data.map((m) => (
              <li key={m.id} className="flex flex-wrap items-center justify-between gap-3 px-5 py-3 text-sm">
                <div className="min-w-0">
                  <p className="truncate font-medium">{m.product_name}</p>
                  <p className="text-xs text-muted">
                    {REASON_LABELS[m.reason] ?? m.reason}
                    {m.note ? ` · ${m.note}` : ""} · {m.created_by_name ?? "System"} · {formatDateTime(m.created_at)}
                  </p>
                </div>
                <div className="text-right">
                  <span className={m.quantity_delta > 0 ? "font-semibold text-success" : "font-semibold text-danger"}>
                    {m.quantity_delta > 0 ? "+" : ""}
                    {formatNumber(m.quantity_delta)}
                  </span>
                  <p className="text-xs text-muted">Balance {formatNumber(m.quantity_after)}</p>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      {editable && dialogProduct ? (
        <AdjustDialog
          product={dialogProduct}
          open
          onClose={closeDialog}
          onDone={(message) => {
            setNotice(message);
            closeDialog();
          }}
        />
      ) : null}
    </div>
  );
}

function AdjustDialog({
  product,
  open,
  onClose,
  onDone,
}: {
  product: ProductListItem;
  open: boolean;
  onClose: () => void;
  onDone: (message: string) => void;
}) {
  const adjust = useAdjustStock();
  const [error, setError] = useState<string | null>(null);
  const form = useForm<AdjustmentValues>({
    resolver: zodResolver(adjustmentSchema),
    defaultValues: { direction: "add", quantity: "", reason: "restock", note: "" },
  });

  async function onSubmit(values: AdjustmentValues) {
    setError(null);
    const magnitude = Number(values.quantity);
    try {
      const response = await adjust.mutateAsync({
        product_id: product.id,
        quantity_delta: values.direction === "add" ? magnitude : -magnitude,
        reason: values.reason,
        note: values.note.trim() || undefined,
      });
      form.reset();
      onDone(`${product.name}: now ${formatNumber(response.stock_quantity)} ${product.unit} on hand.`);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={`Adjust stock: ${product.name}`}
      description={`${formatNumber(product.stock_quantity)} ${product.unit} on hand now.`}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" form="adjust-form" disabled={adjust.isPending}>
            {adjust.isPending ? "Saving…" : "Save adjustment"}
          </Button>
        </>
      }
    >
      <form id="adjust-form" onSubmit={form.handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <div className="grid grid-cols-2 gap-3">
          <SelectField label="Change" {...form.register("direction")}>
            <option value="add">Add stock</option>
            <option value="remove">Remove stock</option>
          </SelectField>
          <TextField label="Quantity" inputMode="numeric" {...form.register("quantity")} error={form.formState.errors.quantity?.message} />
        </div>
        <SelectField label="Reason" {...form.register("reason")} error={form.formState.errors.reason?.message}>
          {adjustmentReasons.map((reason) => (
            <option key={reason} value={reason}>
              {REASON_LABELS[reason]}
            </option>
          ))}
        </SelectField>
        <TextAreaField label="Note (optional)" {...form.register("note")} error={form.formState.errors.note?.message} />
      </form>
    </Dialog>
  );
}
