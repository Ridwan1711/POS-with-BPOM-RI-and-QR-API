//! Checkout: prices, discounts, tax, payments, stock, and receipt numbering.
//!
//! Concurrency model:
//! - The store row is locked (FOR UPDATE) first. This serializes checkouts per store and
//!   makes receipt numbers gap-free. It also makes the idempotency check race-free.
//! - Product rows are then locked in ID order, so no two transactions can deadlock.
//! - Stock is checked and decremented while those locks are held, so it cannot go negative
//!   unless the store explicitly allows it.
//! - Everything commits in one transaction or not at all.

use std::collections::HashMap;

use serde_json::json;
use uuid::Uuid;

use super::{query::{self, SaleView}, CreateSaleRequest};
use crate::{
    app::AppState,
    audit,
    auth::StoreContext,
    error::{AppError, AppResult},
    money::tax_on,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineInput {
    pub unit_price_idr: i64,
    pub quantity: i32,
    pub discount_idr: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totals {
    pub subtotal_idr: i64,
    pub item_discount_idr: i64,
    pub sale_discount_idr: i64,
    pub taxable_idr: i64,
    pub tax_idr: i64,
    pub total_idr: i64,
    pub line_totals_idr: Vec<i64>,
}

fn overflow() -> AppError {
    AppError::unprocessable("amount_overflow", "The sale amount is too large.", serde_json::Value::Null)
}

/// Pure calculation of every monetary value on a sale.
pub fn calculate_totals(lines: &[LineInput], sale_discount_idr: i64, tax_rate_bps: i32) -> AppResult<Totals> {
    let mut subtotal: i64 = 0;
    let mut item_discount: i64 = 0;
    let mut line_totals = Vec::with_capacity(lines.len());

    for line in lines {
        let gross = line
            .unit_price_idr
            .checked_mul(i64::from(line.quantity))
            .ok_or_else(overflow)?;
        if line.discount_idr < 0 || line.discount_idr > gross {
            return Err(AppError::unprocessable(
                "invalid_item_discount",
                "An item discount cannot be larger than the line amount.",
                json!({ "line_amount_idr": gross, "discount_idr": line.discount_idr }),
            ));
        }
        subtotal = subtotal.checked_add(gross).ok_or_else(overflow)?;
        item_discount = item_discount.checked_add(line.discount_idr).ok_or_else(overflow)?;
        line_totals.push(gross - line.discount_idr);
    }

    let after_items = subtotal - item_discount;
    if sale_discount_idr < 0 || sale_discount_idr > after_items {
        return Err(AppError::unprocessable(
            "invalid_sale_discount",
            "The sale discount cannot be larger than the subtotal after item discounts.",
            json!({ "subtotal_after_items_idr": after_items, "discount_idr": sale_discount_idr }),
        ));
    }

    let taxable = after_items - sale_discount_idr;
    let tax = tax_on(taxable, tax_rate_bps).ok_or_else(overflow)?;
    Ok(Totals {
        subtotal_idr: subtotal,
        item_discount_idr: item_discount,
        sale_discount_idr,
        taxable_idr: taxable,
        tax_idr: tax,
        total_idr: taxable + tax,
        line_totals_idr: line_totals,
    })
}

/// Validates tendered amounts against the total. Returns `(paid, change)`.
///
/// Rules: paid must cover the total. Change is only given in cash, and non-cash payments
/// may not exceed the total (cash is never "returned" as card money).
pub fn check_payments(total_idr: i64, payments: &[(String, i64)]) -> AppResult<(i64, i64)> {
    let paid: i64 = payments.iter().map(|(_, amount)| *amount).sum();
    let cash: i64 = payments
        .iter()
        .filter(|(method, _)| method == "cash")
        .map(|(_, amount)| *amount)
        .sum();
    let non_cash = paid - cash;

    if paid < total_idr {
        return Err(AppError::unprocessable(
            "insufficient_payment",
            format!("Payment is short by Rp {}.", total_idr - paid),
            json!({ "total_idr": total_idr, "paid_idr": paid, "shortfall_idr": total_idr - paid }),
        ));
    }
    let change = paid - total_idr;
    if change > 0 && cash == 0 {
        return Err(AppError::unprocessable(
            "overpayment_requires_cash",
            "Change can only be given for cash payments. Match the card or e-wallet amount to the total.",
            json!({ "total_idr": total_idr, "paid_idr": paid }),
        ));
    }
    if non_cash > total_idr {
        return Err(AppError::unprocessable(
            "non_cash_exceeds_total",
            "Non-cash payments cannot be more than the total.",
            json!({ "total_idr": total_idr, "non_cash_idr": non_cash }),
        ));
    }
    Ok((paid, change))
}

#[derive(sqlx::FromRow)]
struct StoreSaleSettings {
    allow_negative_stock: bool,
    tax_rate_bps: i32,
    next_receipt_seq: i64,
    day_key: String,
}

#[derive(sqlx::FromRow)]
struct ProductForSale {
    id: Uuid,
    name: String,
    sku: Option<String>,
    unit: String,
    selling_price_idr: i64,
    stock_quantity: i32,
    is_active: bool,
}

/// Creates a sale. Returns the receipt and `true` when it is a replay of an earlier request.
pub async fn create_sale(
    state: &AppState,
    ctx: &StoreContext,
    req: CreateSaleRequest,
) -> AppResult<(SaleView, bool)> {
    let mut tx = state.pool.begin().await?;

    let store = sqlx::query_as::<_, StoreSaleSettings>(
        "SELECT allow_negative_stock, tax_rate_bps, next_receipt_seq,
                to_char(now() AT TIME ZONE timezone, 'YYYYMMDD') AS day_key
         FROM stores WHERE id = $1 FOR UPDATE",
    )
    .bind(ctx.store_id)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(client_request_id) = req.client_request_id {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM sales WHERE store_id = $1 AND client_request_id = $2",
        )
        .bind(ctx.store_id)
        .bind(client_request_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(sale_id) = existing {
            drop(tx);
            let view = query::get_sale(&state.pool, ctx.store_id, sale_id).await?;
            return Ok((view, true));
        }
    }

    if let Some(customer_id) = req.customer_id {
        let known: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM customers WHERE id = $1 AND store_id = $2)",
        )
        .bind(customer_id)
        .bind(ctx.store_id)
        .fetch_one(&mut *tx)
        .await?;
        if !known {
            return Err(AppError::unprocessable(
                "unknown_customer",
                "The selected customer does not exist in this store.",
                json!({ "customer_id": customer_id }),
            ));
        }
    }

    let product_ids: Vec<Uuid> = req.items.iter().map(|item| item.product_id).collect();
    let products: Vec<ProductForSale> = sqlx::query_as(
        "SELECT id, name, sku, unit, selling_price_idr, stock_quantity, is_active
         FROM products
         WHERE store_id = $1 AND id = ANY($2)
         ORDER BY id
         FOR UPDATE",
    )
    .bind(ctx.store_id)
    .bind(product_ids.clone())
    .fetch_all(&mut *tx)
    .await?;

    let by_id: HashMap<Uuid, ProductForSale> = products.into_iter().map(|p| (p.id, p)).collect();
    let unknown: Vec<Uuid> = product_ids.iter().copied().filter(|id| !by_id.contains_key(id)).collect();
    if !unknown.is_empty() {
        return Err(AppError::unprocessable(
            "unknown_product",
            "One or more products were not found in this store.",
            json!({ "product_ids": unknown }),
        ));
    }

    let mut lines = Vec::with_capacity(req.items.len());
    for item in &req.items {
        let product = &by_id[&item.product_id];
        if !product.is_active {
            return Err(AppError::unprocessable(
                "product_inactive",
                format!("{} is no longer for sale.", product.name),
                json!({ "product_id": product.id }),
            ));
        }
        if !store.allow_negative_stock && product.stock_quantity < item.quantity {
            return Err(AppError::Conflict {
                code: "insufficient_stock",
                message: format!(
                    "Only {} {} of {} left in stock.",
                    product.stock_quantity, product.unit, product.name
                ),
                details: json!({
                    "product_id": product.id,
                    "available": product.stock_quantity,
                    "requested": item.quantity,
                }),
            });
        }
        lines.push(LineInput {
            unit_price_idr: product.selling_price_idr,
            quantity: item.quantity,
            discount_idr: item.discount_idr,
        });
    }

    let totals = calculate_totals(&lines, req.discount_idr, store.tax_rate_bps)?;
    let payments: Vec<(String, i64)> = req
        .payments
        .iter()
        .map(|p| (p.method.clone(), p.amount_idr))
        .collect();
    let (paid, change) = check_payments(totals.total_idr, &payments)?;

    let receipt_number = format!("{}-{:06}", store.day_key, store.next_receipt_seq);
    sqlx::query("UPDATE stores SET next_receipt_seq = next_receipt_seq + 1 WHERE id = $1")
        .bind(ctx.store_id)
        .execute(&mut *tx)
        .await?;

    let sale_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sales (
            store_id, receipt_number, client_request_id, cashier_id, customer_id,
            subtotal_idr, item_discount_idr, sale_discount_idr, tax_rate_bps, tax_idr,
            total_idr, paid_idr, change_idr
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id",
    )
    .bind(ctx.store_id)
    .bind(&receipt_number)
    .bind(req.client_request_id)
    .bind(ctx.user.id)
    .bind(req.customer_id)
    .bind(totals.subtotal_idr)
    .bind(totals.item_discount_idr)
    .bind(totals.sale_discount_idr)
    .bind(store.tax_rate_bps)
    .bind(totals.tax_idr)
    .bind(totals.total_idr)
    .bind(paid)
    .bind(change)
    .fetch_one(&mut *tx)
    .await?;

    for (index, item) in req.items.iter().enumerate() {
        let product = &by_id[&item.product_id];
        let line_no = index as i32 + 1;
        sqlx::query(
            "INSERT INTO sale_items
                (sale_id, line_no, product_id, product_name, sku, unit, unit_price_idr,
                 quantity, discount_idr, line_total_idr)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(sale_id)
        .bind(line_no)
        .bind(product.id)
        .bind(&product.name)
        .bind(&product.sku)
        .bind(&product.unit)
        .bind(product.selling_price_idr)
        .bind(item.quantity)
        .bind(item.discount_idr)
        .bind(totals.line_totals_idr[index])
        .execute(&mut *tx)
        .await?;

        let new_stock: i32 = sqlx::query_scalar(
            "UPDATE products SET stock_quantity = stock_quantity - $2, updated_at = now()
             WHERE id = $1
             RETURNING stock_quantity",
        )
        .bind(product.id)
        .bind(item.quantity)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO inventory_movements
                (store_id, product_id, quantity_delta, quantity_after, reason,
                 reference_type, reference_id, created_by)
             VALUES ($1, $2, $3, $4, 'sale', 'sale', $5, $6)",
        )
        .bind(ctx.store_id)
        .bind(product.id)
        .bind(-item.quantity)
        .bind(new_stock)
        .bind(sale_id)
        .bind(ctx.user.id)
        .execute(&mut *tx)
        .await?;
    }

    for (index, payment) in req.payments.iter().enumerate() {
        sqlx::query(
            "INSERT INTO payments (sale_id, line_no, method, amount_idr, reference)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(sale_id)
        .bind(index as i32 + 1)
        .bind(&payment.method)
        .bind(payment.amount_idr)
        .bind(&payment.reference)
        .execute(&mut *tx)
        .await?;
    }

    audit::record(
        &mut tx,
        Some(ctx.store_id),
        Some(ctx.user.id),
        "sale.create",
        "sale",
        Some(sale_id.to_string()),
        json!({
            "receipt_number": receipt_number,
            "total_idr": totals.total_idr,
            "items": req.items.len(),
        }),
    )
    .await?;
    tx.commit().await?;

    let view = query::get_sale(&state.pool, ctx.store_id, sale_id).await?;
    Ok((view, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(price: i64, qty: i32, discount: i64) -> LineInput {
        LineInput {
            unit_price_idr: price,
            quantity: qty,
            discount_idr: discount,
        }
    }

    #[test]
    fn totals_without_tax_or_discounts() {
        let t = calculate_totals(&[line(12_500, 2, 0), line(3_000, 1, 0)], 0, 0).unwrap();
        assert_eq!(t.subtotal_idr, 28_000);
        assert_eq!(t.total_idr, 28_000);
        assert_eq!(t.line_totals_idr, vec![25_000, 3_000]);
    }

    #[test]
    fn item_then_sale_discount_then_tax() {
        // 2 x 10,000 with a 1,000 item discount = 19,000; minus 4,000 sale discount = 15,000;
        // 11% PPN = 1,650; total 16,650.
        let t = calculate_totals(&[line(10_000, 2, 1_000)], 4_000, 1_100).unwrap();
        assert_eq!(t.item_discount_idr, 1_000);
        assert_eq!(t.taxable_idr, 15_000);
        assert_eq!(t.tax_idr, 1_650);
        assert_eq!(t.total_idr, 16_650);
    }

    #[test]
    fn rejects_discount_larger_than_line() {
        assert!(calculate_totals(&[line(5_000, 1, 6_000)], 0, 0).is_err());
    }

    #[test]
    fn rejects_sale_discount_larger_than_subtotal() {
        assert!(calculate_totals(&[line(5_000, 1, 0)], 5_001, 0).is_err());
    }

    #[test]
    fn cash_overpayment_returns_change() {
        let (paid, change) = check_payments(16_650, &[("cash".into(), 20_000)]).unwrap();
        assert_eq!((paid, change), (20_000, 3_350));
    }

    #[test]
    fn card_overpayment_is_rejected() {
        assert!(check_payments(16_650, &[("debit_card".into(), 20_000)]).is_err());
    }

    #[test]
    fn short_payment_is_rejected() {
        assert!(check_payments(16_650, &[("cash".into(), 16_649)]).is_err());
    }

    #[test]
    fn split_cash_and_card_is_allowed_when_card_does_not_exceed_total() {
        let (_, change) = check_payments(
            16_650,
            &[("qris".into(), 10_000), ("cash".into(), 10_000)],
        )
        .unwrap();
        assert_eq!(change, 3_350);
        assert!(check_payments(16_650, &[("qris".into(), 17_000), ("cash".into(), 1_000)]).is_err());
    }
}
