//! Integer-only money helpers. All amounts are whole Indonesian Rupiah (IDR).

/// Tax on `taxable` at `rate_bps` basis points (1/100 of a percent), rounded half up.
/// Returns `None` on arithmetic overflow.
pub fn tax_on(taxable: i64, rate_bps: i32) -> Option<i64> {
    taxable
        .checked_mul(i64::from(rate_bps))?
        .checked_add(5_000)?
        .checked_div(10_000)
}

#[cfg(test)]
mod tests {
    use super::tax_on;

    #[test]
    fn tax_is_zero_when_rate_is_zero() {
        assert_eq!(tax_on(123_456, 0), Some(0));
    }

    #[test]
    fn tax_applies_eleven_percent_ppn() {
        assert_eq!(tax_on(10_000, 1_100), Some(1_100));
        assert_eq!(tax_on(12_500, 1_100), Some(1_375));
    }

    #[test]
    fn tax_rounds_half_up() {
        assert_eq!(tax_on(5, 1_000), Some(1)); // 0.5 -> 1
        assert_eq!(tax_on(1, 1_100), Some(0)); // 0.11 -> 0
    }

    #[test]
    fn tax_reports_overflow() {
        assert_eq!(tax_on(i64::MAX, 3_000), None);
    }
}
