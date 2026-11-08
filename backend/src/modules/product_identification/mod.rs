//! Product identification.
//!
//! Turns a scanned barcode or QR payload into one of:
//! - an existing local product (never duplicated),
//! - a suggestion built from external provider data (for merchant review),
//! - an explicit "could not identify" outcome that never claims a product does not exist
//!   when a provider merely failed.

pub mod barcode;
pub mod handlers;
pub mod provider;
pub mod providers;
pub mod repository;
pub mod sanitize;
pub mod scan;
pub mod service;

pub use barcode::{normalize_gtin, Gtin, GtinKind};
pub use scan::{parse_scan_input, ScanInput, ScanKind};

/// A code classified for storage as a product identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedCode {
    /// `gtin`, `qr` or `internal`.
    pub scheme: &'static str,
    pub kind: &'static str,
    /// Exactly what was scanned (separators removed for GTINs).
    pub raw_value: String,
    /// Canonical value used for matching.
    pub value: String,
}

/// Classifies a merchant-entered code for storage. Rejects barcodes that fail validation.
pub fn classify_code(code: &str) -> Result<ClassifiedCode, String> {
    let scan = parse_scan_input(code);
    let raw = scan.raw.clone();
    match scan.kind {
        ScanKind::Gtin { gtin, .. } => Ok(ClassifiedCode {
            scheme: "gtin",
            kind: gtin.kind.as_str(),
            raw_value: barcode::strip_separators(&raw),
            value: gtin.canonical,
        }),
        ScanKind::InternalCode => Ok(ClassifiedCode {
            scheme: "internal",
            kind: "internal",
            raw_value: raw.clone(),
            value: raw,
        }),
        ScanKind::Payload { .. } => Ok(ClassifiedCode {
            scheme: "qr",
            kind: "qr_payload",
            raw_value: raw.clone(),
            value: raw,
        }),
        ScanKind::Invalid { reason } => Err(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtin_is_stored_under_gtin_scheme_with_canonical_value() {
        let c = classify_code("4006381333931").unwrap();
        assert_eq!(c.scheme, "gtin");
        assert_eq!(c.kind, "ean13");
        assert_eq!(c.value, "04006381333931");
        assert_eq!(c.raw_value, "4006381333931");
    }

    #[test]
    fn invalid_barcode_is_rejected_for_storage() {
        assert!(classify_code("4006381333932").is_err());
    }

    #[test]
    fn internal_code_keeps_its_text() {
        let c = classify_code(" ROTI-001 ").unwrap();
        assert_eq!((c.scheme, c.value.as_str()), ("internal", "ROTI-001"));
    }
}
