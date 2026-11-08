//! Classifies whatever a scanner or a QR reader produced.
//!
//! Supported inputs:
//! - GTIN barcodes (EAN-8, UPC-E, UPC-A, EAN-13, GTIN-14), with check-digit validation
//! - GS1 Digital Link URLs such as `https://example.org/01/08991002101029/10/ABC`:
//!   only the GTIN path segment is read. The URL is never opened or fetched.
//! - GS1 element strings starting with AI 01, e.g. `(01)08991002101029(10)ABC`
//! - Merchant internal codes (letters, digits and `-._/+`, up to 64 characters)
//! - Anything else is an opaque QR payload. It is recorded and can match a locally
//!   registered QR identifier, but it is never interpreted or followed.

use super::barcode::{normalize_gtin, strip_separators, Gtin};

pub const MAX_INPUT_CHARS: usize = 2048;
const MAX_INTERNAL_CODE_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanKind {
    /// A GTIN, read from a barcode (`via_qr = false`) or from a QR payload (`via_qr = true`).
    Gtin { gtin: Gtin, via_qr: bool },
    /// A merchant-defined alphanumeric code.
    InternalCode,
    /// A QR payload that is not a GTIN. `is_url` marks web links, which are never opened.
    Payload { is_url: bool },
    /// The input looked like a barcode but failed validation.
    Invalid { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanInput {
    /// Trimmed input as received.
    pub raw: String,
    pub kind: ScanKind,
}

impl ScanInput {
    /// `(scheme, value)` pairs used to find local products. Values are canonical.
    pub fn local_keys(&self) -> Vec<(&'static str, String)> {
        match &self.kind {
            ScanKind::Gtin { gtin, via_qr } => {
                let mut keys = vec![("gtin", gtin.canonical.clone())];
                if *via_qr {
                    keys.push(("qr", self.raw.clone()));
                }
                keys
            }
            ScanKind::InternalCode => vec![("internal", self.raw.clone())],
            ScanKind::Payload { .. } => vec![("qr", self.raw.clone())],
            ScanKind::Invalid { .. } => Vec::new(),
        }
    }
}

pub fn parse_scan_input(input: &str) -> ScanInput {
    let raw = input.trim().to_string();

    if raw.is_empty() {
        return invalid(raw, "The code is empty.");
    }
    if raw.chars().count() > MAX_INPUT_CHARS {
        return invalid(raw, "The code is too long to be a product identifier.");
    }

    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        let kind = match gs1_gtin_in_url(&raw) {
            Some(gtin) => ScanKind::Gtin { gtin, via_qr: true },
            None => ScanKind::Payload { is_url: true },
        };
        return ScanInput { raw, kind };
    }

    let compact = strip_separators(&raw);
    let all_digits = !compact.is_empty() && compact.bytes().all(|b| b.is_ascii_digit());

    if all_digits && matches!(compact.len(), 8 | 12 | 13 | 14) {
        let kind = match normalize_gtin(&compact) {
            Ok(gtin) => ScanKind::Gtin { gtin, via_qr: false },
            Err(err) => ScanKind::Invalid {
                reason: format!("Invalid barcode: {err}."),
            },
        };
        return ScanInput { raw, kind };
    }

    if let Some(digits) = gs1_element_gtin(&raw) {
        let kind = match normalize_gtin(&digits) {
            Ok(gtin) => ScanKind::Gtin { gtin, via_qr: true },
            Err(err) => ScanKind::Invalid {
                reason: format!("Invalid GS1 GTIN (AI 01): {err}."),
            },
        };
        return ScanInput { raw, kind };
    }

    if all_digits || is_internal_code(&raw) {
        return ScanInput {
            raw,
            kind: ScanKind::InternalCode,
        };
    }

    ScanInput {
        raw,
        kind: ScanKind::Payload { is_url: false },
    }
}

fn invalid(raw: String, reason: &str) -> ScanInput {
    ScanInput {
        raw,
        kind: ScanKind::Invalid {
            reason: reason.to_string(),
        },
    }
}

/// Reads the GTIN from a GS1 Digital Link path: `.../01/<14 digits>...`.
fn gs1_gtin_in_url(url: &str) -> Option<Gtin> {
    let start = url.find("/01/")?;
    let digits: String = url[start + 4..].chars().take(14).collect();
    if digits.len() == 14 && digits.bytes().all(|b| b.is_ascii_digit()) {
        normalize_gtin(&digits).ok()
    } else {
        None
    }
}

/// Reads the 14-digit GTIN from a GS1 element string: `(01)<14 digits>` or `01<14 digits>...`.
fn gs1_element_gtin(raw: &str) -> Option<String> {
    let rest = if let Some(rest) = raw.strip_prefix("(01)") {
        rest
    } else if raw.len() >= 16 && raw.starts_with("01") {
        &raw[2..]
    } else {
        return None;
    };
    let digits: String = rest.chars().take(14).collect();
    if digits.len() == 14 && digits.bytes().all(|b| b.is_ascii_digit()) {
        Some(digits)
    } else {
        None
    }
}

fn is_internal_code(raw: &str) -> bool {
    raw.chars().count() <= MAX_INTERNAL_CODE_CHARS
        && raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '/' | '+'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::product_identification::barcode::GtinKind;

    fn gtin_of(input: &ScanInput) -> (&Gtin, bool) {
        match &input.kind {
            ScanKind::Gtin { gtin, via_qr } => (gtin, *via_qr),
            other => panic!("expected GTIN, got {other:?}"),
        }
    }

    #[test]
    fn plain_barcode_is_a_gtin() {
        let scan = parse_scan_input("  4006381333931\n");
        let (gtin, via_qr) = gtin_of(&scan);
        assert!(!via_qr);
        assert_eq!(gtin.kind, GtinKind::Ean13);
        assert_eq!(scan.raw, "4006381333931");
    }

    #[test]
    fn spaced_barcode_is_accepted() {
        let scan = parse_scan_input("8991 0021-01029");
        assert_eq!(gtin_of(&scan).0.canonical, "08991002101029");
    }

    #[test]
    fn bad_checksum_is_invalid_not_internal() {
        let scan = parse_scan_input("4006381333932");
        assert!(matches!(scan.kind, ScanKind::Invalid { .. }));
        assert!(scan.local_keys().is_empty());
    }

    #[test]
    fn gs1_element_string_yields_gtin_via_qr() {
        let scan = parse_scan_input("(01)04006381333931(17)260101");
        let (gtin, via_qr) = gtin_of(&scan);
        assert!(via_qr);
        assert_eq!(gtin.canonical, "04006381333931");
    }

    #[test]
    fn gs1_digital_link_url_yields_gtin_and_is_not_followed() {
        let scan = parse_scan_input("https://example.org/01/04006381333931/10/ABC");
        let (gtin, via_qr) = gtin_of(&scan);
        assert!(via_qr);
        assert_eq!(gtin.canonical, "04006381333931");
    }

    #[test]
    fn plain_url_is_an_unsupported_payload() {
        let scan = parse_scan_input("https://example.com/promo?x=1");
        assert_eq!(scan.kind, ScanKind::Payload { is_url: true });
        assert_eq!(scan.local_keys(), vec![("qr", scan.raw.clone())]);
    }

    #[test]
    fn merchant_codes_are_internal() {
        assert_eq!(parse_scan_input("ROTI-001").kind, ScanKind::InternalCode);
        assert_eq!(parse_scan_input("123456").kind, ScanKind::InternalCode);
    }

    #[test]
    fn structured_payloads_are_unsupported() {
        let scan = parse_scan_input("BEGIN:VCARD\nVERSION:3.0");
        assert_eq!(scan.kind, ScanKind::Payload { is_url: false });
    }

    #[test]
    fn empty_input_is_invalid() {
        assert!(matches!(parse_scan_input("   ").kind, ScanKind::Invalid { .. }));
    }

    #[test]
    fn oversized_input_is_rejected() {
        let long = "A".repeat(MAX_INPUT_CHARS + 1);
        assert!(matches!(parse_scan_input(&long).kind, ScanKind::Invalid { .. }));
    }
}
