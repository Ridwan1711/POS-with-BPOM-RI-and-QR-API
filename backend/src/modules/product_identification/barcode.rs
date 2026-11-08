//! GTIN normalization and check-digit validation.
//!
//! Every GTIN-family barcode is converted to its canonical GTIN-14 form (left-padded with
//! zeros). That makes equivalent codes match each other: an EAN-13 `0400638133393`, its
//! UPC-A form `400638133393`, and a GTIN-14 `00400638133393` all share one canonical value.
//! UPC-E codes are expanded to their UPC-A equivalent before canonicalization.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GtinKind {
    Ean8,
    UpcE,
    UpcA,
    Ean13,
    Gtin14,
}

impl GtinKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ean8 => "ean8",
            Self::UpcE => "upce",
            Self::UpcA => "upca",
            Self::Ean13 => "ean13",
            Self::Gtin14 => "gtin14",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gtin {
    pub kind: GtinKind,
    /// 14-digit canonical GTIN, used for all matching.
    pub canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BarcodeError {
    #[error("the code contains characters other than digits")]
    NotNumeric,
    #[error("a GTIN has 8, 12, 13 or 14 digits, but this code has {0}")]
    UnsupportedLength(usize),
    #[error("the check digit does not match, so the barcode was probably misread")]
    CheckDigit,
}

/// GS1 mod-10 check digit for `body` (ASCII digits, check digit excluded).
/// The rightmost body digit has weight 3, weights alternate 3/1 moving left.
pub fn gtin_check_digit(body: &[u8]) -> u8 {
    let n = body.len();
    let sum: u32 = body
        .iter()
        .enumerate()
        .map(|(i, &digit)| {
            let weight = if (n - 1 - i) % 2 == 0 { 3 } else { 1 };
            u32::from(digit - b'0') * weight
        })
        .sum();
    ((10 - sum % 10) % 10) as u8
}

/// True when the last digit of the ASCII-digit string is a valid check digit.
fn check_digit_is_valid(digits: &[u8]) -> bool {
    let (body, check) = digits.split_at(digits.len() - 1);
    gtin_check_digit(body) == check[0] - b'0'
}

/// Expands an 8-digit UPC-E code (number system 0 or 1) to its 12-digit UPC-A form.
/// Returns `None` when the code is not a valid UPC-E.
pub fn expand_upce(digits: &[u8]) -> Option<String> {
    if digits.len() != 8 || (digits[0] != b'0' && digits[0] != b'1') {
        return None;
    }
    let x = |i: usize| digits[i] - b'0';
    let (x1, x2, x3, x4, x5, x6) = (x(1), x(2), x(3), x(4), x(5), x(6));

    // The last compressed digit decides where the zero padding goes.
    let middle: [u8; 10] = match x6 {
        0..=2 => [x1, x2, x6, 0, 0, 0, 0, x3, x4, x5],
        3 => [x1, x2, x3, 0, 0, 0, 0, 0, x4, x5],
        4 => [x1, x2, x3, x4, 0, 0, 0, 0, 0, x5],
        _ => [x1, x2, x3, x4, x5, 0, 0, 0, 0, x6],
    };

    let mut upca: Vec<u8> = Vec::with_capacity(12);
    upca.push(digits[0]);
    upca.extend(middle.iter().map(|d| d + b'0'));
    let check = gtin_check_digit(&upca);
    // The 8th digit of a UPC-E is the UPC-A check digit, so it must match.
    if check + b'0' != digits[7] {
        return None;
    }
    upca.push(digits[7]);
    String::from_utf8(upca).ok()
}

fn pad_to_gtin14(digits: &str) -> String {
    format!("{:0>14}", digits)
}

/// Normalizes a purely numeric GTIN-family code. Whitespace must already be removed.
pub fn normalize_gtin(code: &str) -> Result<Gtin, BarcodeError> {
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BarcodeError::NotNumeric);
    }
    let bytes = code.as_bytes();
    match bytes.len() {
        8 => {
            if let Some(upca) = expand_upce(bytes) {
                return Ok(Gtin {
                    kind: GtinKind::UpcE,
                    canonical: pad_to_gtin14(&upca),
                });
            }
            if check_digit_is_valid(bytes) {
                return Ok(Gtin {
                    kind: GtinKind::Ean8,
                    canonical: pad_to_gtin14(code),
                });
            }
            Err(BarcodeError::CheckDigit)
        }
        12 if check_digit_is_valid(bytes) => Ok(Gtin {
            kind: GtinKind::UpcA,
            canonical: pad_to_gtin14(code),
        }),
        13 if check_digit_is_valid(bytes) => Ok(Gtin {
            kind: GtinKind::Ean13,
            canonical: pad_to_gtin14(code),
        }),
        14 if check_digit_is_valid(bytes) => Ok(Gtin {
            kind: GtinKind::Gtin14,
            canonical: code.to_string(),
        }),
        12..=14 => Err(BarcodeError::CheckDigit),
        n => Err(BarcodeError::UnsupportedLength(n)),
    }
}

/// Removes spaces, tabs, line breaks and hyphens that scanners or printed labels insert.
pub fn strip_separators(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_check_digit(body: &str) -> String {
        format!("{body}{}", gtin_check_digit(body.as_bytes()))
    }

    #[test]
    fn ean13_valid_and_canonicalized() {
        let g = normalize_gtin("4006381333931").unwrap();
        assert_eq!(g.kind, GtinKind::Ean13);
        assert_eq!(g.canonical, "04006381333931");
    }

    #[test]
    fn ean13_bad_check_digit_is_rejected() {
        assert_eq!(normalize_gtin("4006381333932"), Err(BarcodeError::CheckDigit));
    }

    #[test]
    fn upca_matches_its_ean13_equivalent() {
        let upca = normalize_gtin("036000291452").unwrap();
        let ean = normalize_gtin("0036000291452").unwrap();
        assert_eq!(upca.kind, GtinKind::UpcA);
        assert_eq!(ean.kind, GtinKind::Ean13);
        assert_eq!(upca.canonical, ean.canonical);
    }

    #[test]
    fn upce_expands_to_upca_per_standard_example() {
        let g = normalize_gtin("04252614").unwrap();
        assert_eq!(g.kind, GtinKind::UpcE);
        assert_eq!(g.canonical, "00042100005264");
    }

    #[test]
    fn upce_check_digit_must_match() {
        // Same body as the valid example but a wrong final digit.
        assert!(expand_upce(b"04252615").is_none());
    }

    #[test]
    fn ean8_valid() {
        let g = normalize_gtin("96385074").unwrap();
        assert_eq!(g.kind, GtinKind::Ean8);
        assert_eq!(g.canonical, "00000096385074");
    }

    #[test]
    fn gtin14_valid_is_accepted() {
        let code = with_check_digit("1001234567890");
        let g = normalize_gtin(&code).unwrap();
        assert_eq!(g.kind, GtinKind::Gtin14);
        assert_eq!(g.canonical, code);
    }

    #[test]
    fn unsupported_lengths_are_reported() {
        assert_eq!(normalize_gtin("12345"), Err(BarcodeError::UnsupportedLength(5)));
        assert_eq!(normalize_gtin("ABC"), Err(BarcodeError::NotNumeric));
    }

    #[test]
    fn separators_are_stripped() {
        assert_eq!(strip_separators(" 8991-0021 01028\n"), "8991002101028");
    }
}
