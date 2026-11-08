"use client";

import type { SaleView } from "@/lib/api/types";
import { formatDateTime, formatIdr, formatPercentBps, PAYMENT_LABELS } from "@/lib/format";

/** Printable receipt. Everything shown comes from the server's record of the sale. */
export function Receipt({ sale }: { sale: SaleView }) {
  return (
    <article aria-label={`Receipt ${sale.receipt_number}`} className="mx-auto w-full max-w-sm bg-white p-4 font-mono text-[13px] leading-relaxed text-black">
      <header className="text-center">
        <p className="text-base font-bold">{sale.store.name}</p>
        {sale.store.address ? <p>{sale.store.address}</p> : null}
        {sale.store.phone ? <p>{sale.store.phone}</p> : null}
      </header>
      <div className="my-3 border-t border-dashed border-black" />
      <dl className="grid grid-cols-[auto_1fr] gap-x-2">
        <dt>Receipt</dt>
        <dd className="text-right">{sale.receipt_number}</dd>
        <dt>Date</dt>
        <dd className="text-right">{formatDateTime(sale.created_at)}</dd>
        {sale.cashier_name ? (
          <>
            <dt>Cashier</dt>
            <dd className="text-right">{sale.cashier_name}</dd>
          </>
        ) : null}
      </dl>
      <div className="my-3 border-t border-dashed border-black" />

      <ul className="flex flex-col gap-2">
        {sale.items.map((item, index) => (
          <li key={`${item.product_name}-${index}`}>
            <p>{item.product_name}</p>
            <div className="flex justify-between">
              <span>
                {item.quantity} x {formatIdr(item.unit_price_idr)}
              </span>
              <span>{formatIdr(item.line_total_idr + item.discount_idr)}</span>
            </div>
            {item.discount_idr > 0 ? (
              <div className="flex justify-between">
                <span>Discount</span>
                <span>-{formatIdr(item.discount_idr)}</span>
              </div>
            ) : null}
          </li>
        ))}
      </ul>

      <div className="my-3 border-t border-dashed border-black" />
      <dl className="grid grid-cols-[1fr_auto] gap-x-2">
        <dt>Subtotal</dt>
        <dd>{formatIdr(sale.subtotal_idr)}</dd>
        {sale.item_discount_idr > 0 ? (
          <>
            <dt>Item discounts</dt>
            <dd>-{formatIdr(sale.item_discount_idr)}</dd>
          </>
        ) : null}
        {sale.sale_discount_idr > 0 ? (
          <>
            <dt>Discount</dt>
            <dd>-{formatIdr(sale.sale_discount_idr)}</dd>
          </>
        ) : null}
        {sale.tax_rate_bps > 0 ? (
          <>
            <dt>Tax {formatPercentBps(sale.tax_rate_bps)}</dt>
            <dd>{formatIdr(sale.tax_idr)}</dd>
          </>
        ) : null}
        <dt className="mt-1 text-sm font-bold">TOTAL</dt>
        <dd className="mt-1 text-sm font-bold">{formatIdr(sale.total_idr)}</dd>
      </dl>

      <div className="my-3 border-t border-dashed border-black" />
      <ul className="flex flex-col gap-0.5">
        {sale.payments.map((payment, index) => (
          <li key={index} className="flex justify-between">
            <span>{PAYMENT_LABELS[payment.method] ?? payment.method}</span>
            <span>{formatIdr(payment.amount_idr)}</span>
          </li>
        ))}
        {sale.change_idr > 0 ? (
          <li className="flex justify-between">
            <span>Change</span>
            <span>{formatIdr(sale.change_idr)}</span>
          </li>
        ) : null}
      </ul>

      {sale.store.receipt_footer ? (
        <>
          <div className="my-3 border-t border-dashed border-black" />
          <p className="text-center text-xs">{sale.store.receipt_footer}</p>
        </>
      ) : null}
    </article>
  );
}
