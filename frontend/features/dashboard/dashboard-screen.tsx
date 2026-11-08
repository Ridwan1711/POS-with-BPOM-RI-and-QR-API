"use client";

import Link from "next/link";
import { useDashboard } from "@/lib/api/queries";
import { formatDateTime, formatIdr, formatNumber, formatShortDate } from "@/lib/format";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardHeader, PageHeader } from "@/components/ui/card";
import { EmptyState, Spinner } from "@/components/ui/empty-state";

export function DashboardScreen() {
  const dashboard = useDashboard();

  if (dashboard.isPending) return <Spinner label="Loading today's numbers" />;
  if (dashboard.isError || !dashboard.data) {
    return <Alert tone="danger">The dashboard could not be loaded. Refresh and try again.</Alert>;
  }
  const d = dashboard.data;
  const maxRevenue = Math.max(1, ...d.last_7_days.map((day) => day.revenue_idr));

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Dashboard" description={`Store day ${formatShortDate(d.date)} · ${d.timezone}`} />

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <Stat label="Sales today" value={formatIdr(d.today.revenue_idr)} />
        <Stat label="Transactions" value={formatNumber(d.today.transaction_count)} />
        <Stat label="Average sale" value={formatIdr(d.today.average_ticket_idr)} />
        <Stat label="Items sold" value={formatNumber(d.today.items_sold)} />
      </div>

      <div className="grid gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <Card>
          <CardHeader title="Last 7 days" description="Revenue per store day." />
          <div className="p-5">
            <div className="flex h-44 items-end gap-2" role="img" aria-label="Revenue for the last seven days">
              {d.last_7_days.map((day) => (
                <div key={day.date} className="flex h-full flex-1 flex-col justify-end gap-1">
                  <span className="text-center text-[11px] text-muted">{day.revenue_idr > 0 ? formatCompact(day.revenue_idr) : ""}</span>
                  <div
                    className="w-full rounded-t-md bg-primary/80 transition-[height]"
                    style={{ height: `${Math.max(2, (day.revenue_idr / maxRevenue) * 100)}%` }}
                    title={`${formatShortDate(day.date)}: ${formatIdr(day.revenue_idr)} from ${day.transaction_count} sales`}
                  />
                </div>
              ))}
            </div>
            <div className="mt-2 flex gap-2">
              {d.last_7_days.map((day) => (
                <span key={day.date} className="flex-1 text-center text-[11px] text-muted">
                  {formatShortDate(day.date)}
                </span>
              ))}
            </div>
          </div>
        </Card>

        <Card>
          <CardHeader
            title="Low stock"
            description={`${formatNumber(d.low_stock_count)} product${d.low_stock_count === 1 ? "" : "s"} at or below the alert level.`}
            actions={<Link href="/inventory?low=1" className="text-sm font-medium text-primary hover:underline">View all</Link>}
          />
          {d.low_stock_items.length === 0 ? (
            <EmptyState title="Nothing is running low" description="Stock levels are healthy." />
          ) : (
            <ul className="divide-y divide-border">
              {d.low_stock_items.map((item) => (
                <li key={item.id} className="flex items-center justify-between gap-3 px-5 py-3 text-sm">
                  <Link href={`/products/${item.id}`} className="min-w-0 truncate font-medium hover:text-primary">
                    {item.name}
                  </Link>
                  <Badge tone={item.stock_quantity <= 0 ? "danger" : "warning"}>
                    {formatNumber(item.stock_quantity)} {item.unit}
                  </Badge>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>

      <Card>
        <CardHeader
          title="Recent transactions"
          actions={<Link href="/transactions" className="text-sm font-medium text-primary hover:underline">All transactions</Link>}
        />
        {d.recent_sales.length === 0 ? (
          <EmptyState title="No sales yet" description="Completed sales appear here as soon as they are paid." />
        ) : (
          <ul className="divide-y divide-border">
            {d.recent_sales.map((sale) => (
              <li key={sale.id}>
                <Link href={`/transactions/${sale.id}`} className="flex items-center justify-between gap-3 px-5 py-3 text-sm hover:bg-surface-muted/60">
                  <span className="min-w-0">
                    <span className="block font-mono font-medium">{sale.receipt_number}</span>
                    <span className="text-xs text-muted">
                      {formatDateTime(sale.created_at)} · {sale.item_quantity} items
                    </span>
                  </span>
                  <span className="font-semibold">{formatIdr(sale.total_idr)}</span>
                </Link>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <Card className="p-5">
      <p className="text-sm text-muted">{label}</p>
      <p className="mt-1 text-2xl font-semibold tracking-tight">{value}</p>
    </Card>
  );
}

function formatCompact(amount: number): string {
  return new Intl.NumberFormat("id-ID", { notation: "compact", maximumFractionDigits: 1 }).format(amount);
}
