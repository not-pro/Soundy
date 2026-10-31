use std::{collections::HashMap, sync::LazyLock};

pub const VOLUME_TAG: isize = 100;
pub const STATUS_TAG: isize = 101;
pub const STATUS_DOT_TAG: isize = 102;

// Social links live in assets/config.json so they can change without rebuilding logic.
static CONFIG: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../assets/config.json")).expect("valid config.json")
});

fn url(key: &str) -> &'static str {
    CONFIG
        .get(key)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("missing {key} in config.json"))
}

pub fn github_url() -> &'static str {
    url("github_url")
}

pub fn telegram_url() -> &'static str {
    url("telegram_url")
}
