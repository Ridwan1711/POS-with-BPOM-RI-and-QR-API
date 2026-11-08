"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";
import { Minus, Plus, Trash2 } from "lucide-react";
import { useCurrentStore } from "@/features/auth/session";
import { api } from "@/lib/api/client";
import { errorMessage } from "@/lib/api/errors";
import { useCheckout, useLocalScan, useProducts, useStoreSettings } from "@/lib/api/queries";
import type { ProductListItem, SaleView, ScanResponse } from "@/lib/api/types";
import { formatIdr, formatNumber, sanitizeDigits } from "@/lib/format";
import { useDebouncedValue } from "@/lib/hooks";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Dialog } from "@/components/ui/dialog";
import { ScanField } from "@/components/scan-field";
import { Spinner } from "@/components/ui/empty-state";
import { TextField } from "@/components/ui/field";
import { useCartStore, computeTotals, type CartProductInput } from "./cart";
import { PaymentDialog } from "./payment-dialog";
import { Receipt } from "./receipt";

type Feedback = { tone: "success" | "warning" | "danger" | "info"; text: string; productId?: string } | null;

/**
 * Cashier screen: scan or search, build the cart, take payment, print a receipt.
 * Optimized for keyboard-wedge scanners: the scan field keeps focus and clears after each scan.
 */
export function PosScreen() {
  const store = useCurrentStore();
  const settings = useStoreSettings();
  const scan = useLocalScan();
  const checkout = useCheckout();

  const lines = useCartStore((s) => s.lines);
  const saleDiscount = useCartStore((s) => s.saleDiscountIdr);
  const addProduct = useCartStore((s) => s.addProduct);
  const setQuantity = useCartStore((s) => s.setQuantity);
  const removeLine = useCartStore((s) => s.removeLine);
  const setSaleDiscount = useCartStore((s) => s.setSaleDiscount);
  const clearCart = useCartStore((s) => s.clear);

  const taxRateBps = settings.data?.tax_rate_bps ?? 0;
  const totals = computeTotals(lines, saleDiscount, taxRateBps);

  const [feedback, setFeedback] = useState<Feedback>(null);
  const [search, setSearch] = useState("");
  const debouncedSearch = useDebouncedValue(search.trim(), 250);
  const products = useProducts({ q: debouncedSearch || undefined, status: "active", limit: 24 });

  const [paymentOpen, setPaymentOpen] = useState(false);
  const [receipt, setReceipt] = useState<SaleView | null>(null);
  const scanRef = useRef<HTMLInputElement>(null);

  const focusScan = useCallback(() => {
    scanRef.current?.focus({ preventScroll: true });
  }, []);

  useEffect(() => {
    if (!paymentOpen && !receipt) focusScan();
  }, [paymentOpen, receipt, focusScan]);

  function addFromScan(result: ScanResponse) {
    if (result.status === "found" && result.product) {
      const product = result.product;
      if (!product.is_active) {
        setFeedback({ tone: "warning", text: `${product.name} is inactive and cannot be sold.`, productId: product.id });
        return;
      }
      const outcome = addProduct(toCartInput(product));
      setFeedback(
        outcome.ok
          ? { tone: "success", text: `Added ${product.name}`, productId: product.id }
          : { tone: "warning", text: outcome.reason, productId: product.id },
      );
      return;
    }
    if (result.status === "not_found") {
      setFeedback({ tone: "warning", text: `No product uses ${result.input.raw}. Add it in Products.` });
      return;
    }
    setFeedback({ tone: "danger", text: result.message });
  }

  async function onScan(code: string) {
    setFeedback(null);
    try {
      addFromScan(await scan.mutateAsync(code));
    } catch (error) {
      setFeedback({ tone: "danger", text: errorMessage(error, "The scan failed. Try again.") });
    } finally {
      focusScan();
    }
  }

  function addTile(product: ProductListItem) {
    const outcome = addProduct(toCartInput(product));
    setFeedback(
      outcome.ok
        ? { tone: "success", text: `Added ${product.name}`, productId: product.id }
        : { tone: "warning", text: outcome.reason, productId: product.id },
    );
    focusScan();
  }

  function startNewSale() {
    setReceipt(null);
    clearCart();
    setFeedback(null);
    focusScan();
  }

  if (!store) return <Spinner label="Loading cashier" />;

  return (
    <div className="grid gap-6 pb-24 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)] lg:pb-0">
      <section aria-label="Find products" className="flex min-w-0 flex-col gap-4">
        <Card className="p-4">
          <ScanField
            ref={scanRef}
            label="Scan barcode"
            placeholder="Scan the product, or type its code and press Enter"
            onSubmit={onScan}
            pending={scan.isPending}
            clearOnSubmit
            submitLabel="Add"
          />
          <div className="mt-3">
            <TextField
              label="Search products"
              type="search"
              placeholder="Name, brand or SKU"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              autoComplete="off"
            />
          </div>
          {feedback ? (
            <p
              role="status"
              aria-live="polite"
              className={
                "mt-3 rounded-md px-3 py-2 text-sm " +
                (feedback.tone === "success"
                  ? "bg-success-soft text-success"
                  : feedback.tone === "danger"
                    ? "bg-danger-soft text-danger"
                    : "bg-warning-soft text-warning")
              }
            >
              {feedback.text}
              {feedback.tone !== "success" && feedback.text.includes("Add it in Products") ? (
                <>
                  {" "}
                  <Link href="/products/new" className="font-medium underline">
                    Add product
                  </Link>
                </>
              ) : null}
            </p>
          ) : null}
        </Card>

        {products.isPending ? (
          <Spinner label="Loading products" />
        ) : products.data && products.data.data.length === 0 ? (
          <p className="px-1 text-sm text-muted">No active products match. Scan a barcode or try another search.</p>
        ) : (
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-4" aria-label="Products">
            {products.data?.data.map((product) => {
              const out = product.stock_quantity <= 0;
              return (
                <button
                  key={product.id}
                  type="button"
                  onClick={() => addTile(product)}
                  disabled={out}
                  className="flex min-h-28 flex-col justify-between rounded-lg border border-border bg-surface p-3 text-left transition-colors hover:border-primary hover:bg-primary-soft/40 focus-visible:outline-2 focus-visible:outline-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <span className="line-clamp-2 text-sm font-medium leading-snug">{product.name}</span>
                  <span className="mt-2 text-base font-semibold">{formatIdr(product.selling_price_idr)}</span>
                  <span className="text-xs text-muted">
                    {out ? "Out of stock" : `${formatNumber(product.stock_quantity)} ${product.unit} left`}
                  </span>
                  {product.is_low_stock && !out ? <Badge tone="warning" className="mt-2 self-start">Low stock</Badge> : null}
                </button>
              );
            })}
          </div>
        )}
      </section>

      <aside aria-label="Cart" className="lg:sticky lg:top-6 lg:h-fit">
        <Card className="flex flex-col">
          <div className="flex items-center justify-between border-b border-border px-5 py-4">
            <h2 className="text-base font-semibold">Cart</h2>
            <span className="text-sm text-muted">{totals.itemCount} item{totals.itemCount === 1 ? "" : "s"}</span>
          </div>

          {lines.length === 0 ? (
            <p className="px-5 py-10 text-center text-sm text-muted">Scan a product or tap one to start the sale.</p>
          ) : (
            <ul className="max-h-[45vh] divide-y divide-border overflow-y-auto">
              {lines.map((line) => (
                <li key={line.productId} className="flex items-start gap-3 px-5 py-3">
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">{line.name}</p>
                    <p className="text-xs text-muted">
                      {formatIdr(line.unitPriceIdr)} / {line.unit}
                    </p>
                    <div className="mt-2 flex items-center gap-1">
                      <Button variant="outline" size="sm" className="size-8 px-0" aria-label={`Decrease ${line.name}`} onClick={() => setQuantity(line.productId, line.quantity - 1)}>
                        <Minus className="size-3.5" aria-hidden="true" />
                      </Button>
                      <input
                        aria-label={`Quantity of ${line.name}`}
                        inputMode="numeric"
                        value={line.quantity}
                        onChange={(e) => setQuantity(line.productId, Number(sanitizeDigits(e.target.value) || 0))}
                        className="h-8 w-14 rounded-md border border-border-strong bg-surface text-center text-sm"
                      />
                      <Button variant="outline" size="sm" className="size-8 px-0" aria-label={`Increase ${line.name}`} onClick={() => setQuantity(line.productId, line.quantity + 1)}>
                        <Plus className="size-3.5" aria-hidden="true" />
                      </Button>
                      <Button variant="ghost" size="sm" className="ml-auto size-8 px-0 text-danger" aria-label={`Remove ${line.name}`} onClick={() => removeLine(line.productId)}>
                        <Trash2 className="size-4" aria-hidden="true" />
                      </Button>
                    </div>
                  </div>
                  <span className="shrink-0 text-sm font-semibold">{formatIdr(line.unitPriceIdr * line.quantity)}</span>
                </li>
              ))}
            </ul>
          )}

          <div className="flex flex-col gap-3 border-t border-border px-5 py-4 text-sm">
            <div className="flex items-center justify-between gap-3">
              <label htmlFor="sale-discount" className="text-muted">
                Discount (IDR)
              </label>
              <input
                id="sale-discount"
                inputMode="numeric"
                value={saleDiscount === 0 ? "" : String(saleDiscount)}
                placeholder="0"
                onChange={(e) => setSaleDiscount(Number(sanitizeDigits(e.target.value) || 0))}
                className="h-9 w-32 rounded-md border border-border-strong bg-surface px-3 text-right"
              />
            </div>
            <dl className="flex flex-col gap-1.5">
              <div className="flex justify-between">
                <dt className="text-muted">Subtotal</dt>
                <dd>{formatIdr(totals.subtotalIdr)}</dd>
              </div>
              {totals.saleDiscountIdr > 0 ? (
                <div className="flex justify-between">
                  <dt className="text-muted">Discount</dt>
                  <dd>-{formatIdr(totals.saleDiscountIdr)}</dd>
                </div>
              ) : null}
              {taxRateBps > 0 ? (
                <div className="flex justify-between">
                  <dt className="text-muted">Tax</dt>
                  <dd>{formatIdr(totals.taxIdr)}</dd>
                </div>
              ) : null}
              <div className="flex items-baseline justify-between border-t border-border pt-2">
                <dt className="font-semibold">Total</dt>
                <dd className="text-2xl font-semibold tracking-tight">{formatIdr(totals.totalIdr)}</dd>
              </div>
            </dl>
          </div>

          <div className="flex gap-2 border-t border-border p-4">
            <Button variant="ghost" onClick={clearCart} disabled={lines.length === 0} className="shrink-0">
              Clear
            </Button>
            <Button size="lg" className="flex-1" disabled={lines.length === 0} onClick={() => setPaymentOpen(true)}>
              Pay {formatIdr(totals.totalIdr)}
            </Button>
          </div>
        </Card>
      </aside>

      {lines.length > 0 ? (
        <div className="no-print fixed inset-x-0 bottom-0 z-20 flex items-center gap-3 border-t border-border bg-surface px-4 py-3 shadow-lg lg:hidden">
          <div className="min-w-0 flex-1">
            <p className="text-xs text-muted">
              {totals.itemCount} item{totals.itemCount === 1 ? "" : "s"}
            </p>
            <p className="truncate text-lg font-semibold">{formatIdr(totals.totalIdr)}</p>
          </div>
          <Button size="lg" onClick={() => setPaymentOpen(true)}>
            Pay
          </Button>
        </div>
      ) : null}

      <PaymentDialog
        open={paymentOpen}
        onClose={() => setPaymentOpen(false)}
        totals={totals}
        pending={checkout.isPending}
        submit={(body) => checkout.mutateAsync(body)}
        buildBody={(clientRequestId, method, amount) => ({
          client_request_id: clientRequestId,
          items: lines.map((line) => ({ product_id: line.productId, quantity: line.quantity })),
          discount_idr: saleDiscount,
          payments: [{ method, amount_idr: method === "cash" ? amount : totals.totalIdr }],
        })}
        onSucceeded={async (saleId) => {
          setPaymentOpen(false);
          const sale = await api.get<SaleView>(`/sales/${saleId}`);
          setReceipt(sale);
        }}
        onStockProblem={() => {
          void settings.refetch();
          void products.refetch();
        }}
      />

      <Dialog
        open={receipt !== null}
        onClose={startNewSale}
        title="Sale complete"
        description={receipt ? `Receipt ${receipt.receipt_number}` : undefined}
        size="sm"
        footer={
          <>
            <Button variant="outline" onClick={() => window.print()} className="no-print">
              Print receipt
            </Button>
            <Button onClick={startNewSale}>New sale</Button>
          </>
        }
      >
        {receipt ? (
          <>
            <Alert tone="success" title={`Change ${formatIdr(receipt.change_idr)}`}>
              Paid {formatIdr(receipt.paid_idr)} for {formatIdr(receipt.total_idr)}.
            </Alert>
            <div className="mt-4 print:mt-0">
              <Receipt sale={receipt} />
            </div>
          </>
        ) : null}
      </Dialog>
    </div>
  );
}

function toCartInput(product: ProductListItem): CartProductInput {
  return {
    id: product.id,
    name: product.name,
    sku: product.sku,
    unit: product.unit,
    selling_price_idr: product.selling_price_idr,
    stock_quantity: product.stock_quantity,
  };
}
