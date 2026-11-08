//! Defensive cleanup of provider output. Provider data is untrusted input.

use super::provider::{ExternalProductData, RegistrationInfo, RegistrationStatus};

const MAX_NAME_CHARS: usize = 200;
const MAX_BRAND_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 2000;
const MAX_UNIT_CHARS: usize = 20;
const MAX_URL_CHARS: usize = 2048;
const MAX_REGISTRATION_CHARS: usize = 100;

/// Removes control characters, trims, caps length (in characters), and drops blank values.
pub fn clean_text(value: Option<String>, max_chars: usize) -> Option<String> {
    let value = value?;
    let printable: String = value.chars().filter(|c| !c.is_control()).collect();
    let trimmed = printable.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(max_chars).collect())
    }
}

/// Images are only accepted as `https://` URLs. They are stored as text and never fetched here.
fn clean_https_url(value: Option<String>) -> Option<String> {
    let url = clean_text(value, MAX_URL_CHARS)?;
    if url.starts_with("https://") && !url.contains(char::is_whitespace) {
        Some(url)
    } else {
        None
    }
}

fn clean_registration(registration: Option<RegistrationInfo>) -> Option<RegistrationInfo> {
    let mut reg = registration?;
    reg.number = clean_text(reg.number.take(), MAX_REGISTRATION_CHARS);
    // "Verified" without a verification timestamp is not a verification.
    if reg.status == RegistrationStatus::Verified && reg.verified_at.is_none() {
        reg.status = RegistrationStatus::Reported;
    }
    if reg.number.is_none() && reg.status == RegistrationStatus::Unknown && reg.valid_until.is_none() {
        return None;
    }
    Some(reg)
}

pub fn sanitize(data: ExternalProductData) -> ExternalProductData {
    ExternalProductData {
        name: clean_text(data.name, MAX_NAME_CHARS),
        brand: clean_text(data.brand, MAX_BRAND_CHARS),
        manufacturer: clean_text(data.manufacturer, MAX_BRAND_CHARS),
        description: clean_text(data.description, MAX_DESCRIPTION_CHARS),
        unit_hint: clean_text(data.unit_hint, MAX_UNIT_CHARS),
        image_url: clean_https_url(data.image_url),
        registration: clean_registration(data.registration),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn strips_control_characters_and_blank_values() {
        assert_eq!(
            clean_text(Some("  Mie\u{0007} Goreng\n ".into()), 100),
            Some("Mie Goreng".into())
        );
        assert_eq!(clean_text(Some(" \t ".into()), 100), None);
    }

    #[test]
    fn caps_length_by_characters() {
        assert_eq!(clean_text(Some("ÁBCDEF".into()), 3), Some("ÁBC".into()));
    }

    #[test]
    fn rejects_non_https_images() {
        assert_eq!(clean_https_url(Some("http://cdn.example/a.png".into())), None);
        assert_eq!(clean_https_url(Some("javascript:alert(1)".into())), None);
        assert_eq!(
            clean_https_url(Some("https://cdn.example/a.png".into())),
            Some("https://cdn.example/a.png".into())
        );
    }

    #[test]
    fn verified_without_timestamp_is_downgraded() {
        let cleaned = sanitize(ExternalProductData {
            registration: Some(RegistrationInfo {
                number: Some("X-1".into()),
                status: RegistrationStatus::Verified,
                valid_until: NaiveDate::from_ymd_opt(2030, 1, 1),
                verified_at: None,
            }),
            ..Default::default()
        });
        assert_eq!(cleaned.registration.unwrap().status, RegistrationStatus::Reported);
    }

    #[test]
    fn empty_registration_is_dropped() {
        let cleaned = sanitize(ExternalProductData {
            registration: Some(RegistrationInfo::default()),
            ..Default::default()
        });
        assert!(cleaned.registration.is_none());
    }
}
