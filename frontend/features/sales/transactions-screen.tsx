"use client";

import Link from "next/link";
import { useState } from "react";
import { useDashboard, useSales } from "@/lib/api/queries";
import { formatDateTime, formatIdr, formatNumber, PAYMENT_LABELS } from "@/lib/format";
import { useDebouncedValue } from "@/lib/hooks";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, PageHeader } from "@/components/ui/card";
import { EmptyState, Spinner } from "@/components/ui/empty-state";
import { TextField } from "@/components/ui/field";

const PAGE_SIZE = 25;
type Range = "today" | "week" | "all";

export function TransactionsScreen() {
  const dashboard = useDashboard();
  const [range, setRange] = useState<Range>("week");
  const [receipt, setReceipt] = useState("");
  const [offset, setOffset] = useState(0);
  const debouncedReceipt = useDebouncedValue(receipt.trim(), 300);

  const today = dashboard.data?.date;
  const from = range === "today" ? today : range === "week" && today ? shiftDate(today, -6) : undefined;
  const to = range === "all" ? undefined : today;

  const sales = useSales({
    from,
    to,
    q: debouncedReceipt || undefined,
    limit: PAGE_SIZE,
    offset,
  });

  const total = sales.data?.pagination.total ?? 0;

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Transactions" description="Every completed sale, with its receipt." />

      <Card className="flex flex-col gap-4 p-4">
        <div className="flex flex-wrap gap-2" role="group" aria-label="Date range">
          {(
            [
              ["today", "Today"],
              ["week", "Last 7 days"],
              ["all", "All time"],
            ] as const
          ).map(([value, label]) => (
            <Button
              key={value}
              variant={range === value ? "primary" : "outline"}
              size="sm"
              aria-pressed={range === value}
              onClick={() => {
                setRange(value);
                setOffset(0);
              }}
            >
              {label}
            </Button>
          ))}
        </div>
        <div className="max-w-sm">
          <TextField
            label="Find by receipt number"
            placeholder="for example 20261108-000042"
            value={receipt}
            onChange={(e) => {
              setReceipt(e.target.value);
              setOffset(0);
            }}
            autoComplete="off"
          />
        </div>
      </Card>

      {sales.data ? (
        <div className="grid gap-4 sm:grid-cols-2">
          <Card className="p-5">
            <p className="text-sm text-muted">Transactions</p>
            <p className="mt-1 text-2xl font-semibold">{formatNumber(sales.data.summary.transaction_count)}</p>
          </Card>
          <Card className="p-5">
            <p className="text-sm text-muted">Revenue in this list</p>
            <p className="mt-1 text-2xl font-semibold">{formatIdr(sales.data.summary.revenue_idr)}</p>
          </Card>
        </div>
      ) : null}

      {sales.isError ? <Alert tone="danger">Transactions could not be loaded. Refresh and try again.</Alert> : null}

      <Card className="overflow-hidden">
        {sales.isPending ? (
          <Spinner label="Loading transactions" />
        ) : sales.data?.data.length === 0 ? (
          <EmptyState title="No transactions in this range" description="Completed sales in the cashier screen appear here." />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[640px] text-left text-sm">
              <thead className="bg-surface-muted text-xs uppercase tracking-wide text-muted">
                <tr>
                  <th className="px-4 py-3 font-medium">Receipt</th>
                  <th className="px-4 py-3 font-medium">Time</th>
                  <th className="px-4 py-3 text-right font-medium">Items</th>
                  <th className="px-4 py-3 font-medium">Payment</th>
                  <th className="px-4 py-3 font-medium">Cashier</th>
                  <th className="px-4 py-3 text-right font-medium">Total</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {sales.data?.data.map((sale) => (
                  <tr key={sale.id} className="hover:bg-surface-muted/60">
                    <td className="px-4 py-3">
                      <Link href={`/transactions/${sale.id}`} className="font-mono font-medium hover:text-primary">
                        {sale.receipt_number}
                      </Link>
                    </td>
                    <td className="px-4 py-3 text-muted">{formatDateTime(sale.created_at)}</td>
                    <td className="px-4 py-3 text-right">{formatNumber(sale.item_quantity)}</td>
                    <td className="px-4 py-3">
                      <div className="flex flex-wrap gap-1">
                        {sale.payment_methods.map((m) => (
                          <Badge key={m}>{PAYMENT_LABELS[m] ?? m}</Badge>
                        ))}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-muted">{sale.cashier_name ?? "—"}</td>
                    <td className="px-4 py-3 text-right font-semibold">{formatIdr(sale.total_idr)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {total > PAGE_SIZE ? (
          <div className="flex items-center justify-between border-t border-border px-4 py-3 text-sm">
            <span className="text-muted">
              {formatNumber(offset + 1)}–{formatNumber(Math.min(offset + PAGE_SIZE, total))} of {formatNumber(total)}
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

/** Shifts a "YYYY-MM-DD" date by whole days without timezone drift. */
export function shiftDate(isoDate: string, days: number): string {
  const [y, m, d] = isoDate.split("-").map(Number);
  const date = new Date(Date.UTC(y, m - 1, d + days));
  return date.toISOString().slice(0, 10);
}
