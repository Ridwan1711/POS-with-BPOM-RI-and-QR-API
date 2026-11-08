"use client";

import { useMemo, useState } from "react";
import { ApiError } from "@/lib/api/client";
import { errorMessage } from "@/lib/api/errors";
import type { CheckoutBody } from "@/lib/api/queries";
import { formatIdr, PAYMENT_LABELS, sanitizeDigits } from "@/lib/format";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { SelectField, TextField } from "@/components/ui/field";
import type { CartTotals } from "./cart";

const CASH = "cash";
const NON_CASH_METHODS = ["qris", "debit_card", "credit_card", "bank_transfer", "ewallet"];
const QUICK_CASH = [50_000, 100_000, 200_000];

export interface PaymentDialogProps {
  open: boolean;
  onClose: () => void;
  totals: CartTotals;
  /** Items and discount to send. The server recalculates everything from its own prices. */
  buildBody: (clientRequestId: string, method: string, amount: number) => CheckoutBody;
  submit: (body: CheckoutBody) => Promise<unknown>;
  pending: boolean;
  onSucceeded: (saleId: string) => void;
  onStockProblem: () => void;
}

/**
 * Tender and confirm. The client request ID is created once per open dialog, so pressing
 * Pay twice (or retrying after a dropped connection) cannot charge the customer twice.
 */
export function PaymentDialog({ open, onClose, totals, buildBody, submit, pending, onSucceeded, onStockProblem }: PaymentDialogProps) {
  const [method, setMethod] = useState<string>(CASH);
  const [tendered, setTendered] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [requestId, setRequestId] = useState(() => crypto.randomUUID());

  const isCash = method === CASH;
  const amount = isCash ? Number(tendered || 0) : totals.totalIdr;
  const change = Math.max(0, amount - totals.totalIdr);
  const shortBy = Math.max(0, totals.totalIdr - amount);
  const canPay = totals.itemCount > 0 && (isCash ? amount >= totals.totalIdr : true);

  const quickButtons = useMemo(() => {
    const exact = totals.totalIdr;
    return [exact, ...QUICK_CASH.filter((v) => v > exact).slice(0, 2)];
  }, [totals.totalIdr]);

  function handleOpenChange(next: boolean) {
    if (!next) onClose();
  }

  // Reset tender each time the dialog opens, with a new request ID for the new sale.
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    if (open) {
      setMethod(CASH);
      setTendered(String(totals.totalIdr));
      setError(null);
      setRequestId(crypto.randomUUID());
    }
  }

  async function pay() {
    setError(null);
    if (!canPay) {
      setError(isCash ? `Cash received is short by ${formatIdr(shortBy)}.` : "Payment does not match the total.");
      return;
    }
    try {
      const sale = (await submit(buildBody(requestId, method, amount))) as { id: string };
      onSucceeded(sale.id);
    } catch (err) {
      if (err instanceof ApiError && err.code === "insufficient_stock") {
        setError(err.message);
        onStockProblem();
        return;
      }
      setError(errorMessage(err, "Payment could not be completed. Nothing was charged. Try again."));
    }
  }

  return (
    <Dialog
      open={open}
      onClose={() => handleOpenChange(false)}
      title="Payment"
      description={`${totals.itemCount} item${totals.itemCount === 1 ? "" : "s"} · Total ${formatIdr(totals.totalIdr)}`}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={() => handleOpenChange(false)} disabled={pending}>
            Back to cart
          </Button>
          <Button size="lg" onClick={pay} disabled={pending || !canPay}>
            {pending ? "Processing…" : `Complete sale · ${formatIdr(totals.totalIdr)}`}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-5">
        {error ? <Alert tone="danger">{error}</Alert> : null}

        <SelectField
          label="Payment method"
          value={method}
          onChange={(event) => {
            const next = event.target.value;
            setMethod(next);
            setTendered(next === CASH ? String(totals.totalIdr) : String(totals.totalIdr));
          }}
        >
          <option value={CASH}>{PAYMENT_LABELS.cash}</option>
          {NON_CASH_METHODS.map((m) => (
            <option key={m} value={m}>
              {PAYMENT_LABELS[m]}
            </option>
          ))}
        </SelectField>

        {isCash ? (
          <>
            <TextField
              label="Cash received (IDR)"
              inputMode="numeric"
              autoFocus
              value={tendered}
              onChange={(event) => setTendered(sanitizeDigits(event.target.value))}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void pay();
                }
              }}
            />
            <div className="flex flex-wrap gap-2" role="group" aria-label="Quick amounts">
              {quickButtons.map((value) => (
                <Button key={value} variant="outline" size="sm" onClick={() => setTendered(String(value))}>
                  {value === totals.totalIdr ? "Exact" : formatIdr(value)}
                </Button>
              ))}
            </div>
          </>
        ) : (
          <p className="rounded-md bg-surface-muted px-4 py-3 text-sm text-muted">
            The {PAYMENT_LABELS[method]} amount must equal the total: {formatIdr(totals.totalIdr)}.
          </p>
        )}

        <div className="grid grid-cols-2 gap-3 rounded-lg border border-border bg-surface-muted p-4 text-sm">
          <span className="text-muted">Total</span>
          <span className="text-right font-medium">{formatIdr(totals.totalIdr)}</span>
          {isCash ? (
            <>
              <span className="text-muted">Change</span>
              <span className="text-right text-lg font-semibold text-primary">{formatIdr(change)}</span>
            </>
          ) : null}
        </div>
      </div>
    </Dialog>
  );
}
