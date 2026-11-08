use serde::{Deserialize, Deserializer};

/// Lets a PATCH distinguish "field absent" (`None`), "explicit null" (`Some(None)`)
/// and "new value" (`Some(Some(v))`). Use with `#[serde(default, deserialize_with = "double_option")]`.
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Trims text and turns blank strings into `None`.
pub fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// `%term%` pattern for ILIKE, with LIKE wildcards in the term escaped.
pub fn contains_pattern(term: &str) -> String {
    let escaped = term
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// True when `err` is a unique-constraint violation on a constraint whose name contains `fragment`.
pub fn is_unique_violation(err: &sqlx::Error, fragment: &str) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.code().as_deref() == Some("23505")
                && db.constraint().is_some_and(|c| c.contains(fragment))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_optional, contains_pattern};

    #[test]
    fn blank_text_becomes_none() {
        assert_eq!(clean_optional(Some("   ".into())), None);
        assert_eq!(clean_optional(Some("  Mie  ".into())), Some("Mie".into()));
    }

    #[test]
    fn like_wildcards_are_escaped() {
        assert_eq!(contains_pattern("50%_off"), "%50\\%\\_off%");
    }
}
