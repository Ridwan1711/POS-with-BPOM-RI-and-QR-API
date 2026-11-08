use serde::Serialize;

pub const DEFAULT_LIMIT: i64 = 25;
pub const MAX_LIMIT: i64 = 100;

/// Clamps caller-supplied paging parameters to safe bounds.
pub fn normalize_page(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    (
        limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
        offset.unwrap_or(0).max(0),
    )
}

#[derive(Debug, Serialize)]
pub struct PageMeta {
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
}

/// Paginated list response: `{"data": [...], "pagination": {...}}`.
#[derive(Debug, Serialize)]
pub struct Paged<T> {
    pub data: Vec<T>,
    pub pagination: PageMeta,
}

impl<T> Paged<T> {
    pub fn new(data: Vec<T>, limit: i64, offset: i64, total: i64) -> Self {
        Self {
            data,
            pagination: PageMeta { limit, offset, total },
        }
    }
}

/// Non-paginated list response: `{"data": [...]}`.
#[derive(Debug, Serialize)]
pub struct DataResponse<T> {
    pub data: Vec<T>,
}

#[cfg(test)]
mod tests {
    use super::normalize_page;

    #[test]
    fn defaults_and_clamps() {
        assert_eq!(normalize_page(None, None), (25, 0));
        assert_eq!(normalize_page(Some(10_000), Some(-5)), (100, 0));
        assert_eq!(normalize_page(Some(0), Some(40)), (1, 40));
    }
}
