use std::{collections::HashMap, sync::LazyLock};

pub const VOLUME_TAG: isize = 100;

// Social links live in assets/config.json so they can change without rebuilding logic.
static CONFIG: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../assets/config.json")).expect("valid config.json")
});

pub fn github_url() -> &'static str {
    CONFIG["github_url"].as_str()
}

pub fn telegram_url() -> &'static str {
    CONFIG["telegram_url"].as_str()
}
