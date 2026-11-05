use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Every error the launcher shows to a person.
///
/// `key` names a message in `ui/src/locales/*.json` (under `errors`), `params`
/// fills its placeholders, and `detail` carries the technical cause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppError {
    pub key: String,
    pub params: BTreeMap<String, String>,
    pub detail: String,
}

pub type Result<T> = std::result::Result<T, AppError>;

impl AppError {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            params: BTreeMap::new(),
            detail: String::new(),
        }
    }

    pub fn with_param(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.params.insert(k.into(), v.into());
        self
    }

    pub fn with_detail(mut self, d: impl Into<String>) -> Self {
        self.detail = d.into();
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.detail.is_empty() {
            write!(f, "{}", self.key)
        } else {
            write!(f, "{}: {}", self.key, self.detail)
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("error.io").with_detail(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // reqwest's own message hides the cause ("error sending request"); add the chain.
        let mut detail = e.to_string();
        let mut source = std::error::Error::source(&e);
        while let Some(s) = source {
            detail.push_str(": ");
            detail.push_str(&s.to_string());
            source = s.source();
        }
        AppError::new("error.network").with_detail(detail)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::new("error.parse").with_detail(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_fields() {
        let e = AppError::new("error.x")
            .with_param("file", "a.jar")
            .with_detail("boom");
        assert_eq!(e.key, "error.x");
        assert_eq!(e.params.get("file").map(String::as_str), Some("a.jar"));
        assert_eq!(e.detail, "boom");
    }

    #[test]
    fn io_error_maps_to_error_io() {
        let e: AppError = std::io::Error::new(std::io::ErrorKind::NotFound, "nope").into();
        assert_eq!(e.key, "error.io");
        assert!(e.detail.contains("nope"));
    }

    #[tokio::test]
    async fn network_error_detail_includes_cause() {
        // Nothing listens on port 9 of localhost: connection refused.
        let e: AppError = reqwest::get("http://127.0.0.1:9/")
            .await
            .unwrap_err()
            .into();
        assert_eq!(e.key, "error.network");
        assert!(
            e.detail.to_lowercase().contains("refused") || e.detail.contains("os error"),
            "{}",
            e.detail
        );
    }

    #[test]
    fn json_error_maps_to_error_parse() {
        let e: AppError = serde_json::from_str::<u32>("x").unwrap_err().into();
        assert_eq!(e.key, "error.parse");
    }
}
