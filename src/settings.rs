use serde::{Deserialize, Serialize};

/// Static plugin configuration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Path to `translations.i18n` cache.
    pub translation_cache: String,
}

/// Additional context for the plugin.
#[derive(Debug)]
pub struct Context {
    /// The target environment (from `NODE_ENV`).
    pub env_name: Environment,
    /// The name of the current file.
    pub filename: String,
}

/// The target environment.
#[derive(Debug, PartialEq, Eq)]
pub enum Environment {
    /// Development mode uses fallback for unknown words and an import for all
    /// words.
    ///
    /// ```javascript
    /// import { __i18n_ItemNumber, __i18n_Save } from "../translations.i18n?dev"
    /// ```
    Development,
    /// Test mode is for running with Jest, where the plugin is ignored.
    Test,
    /// Production mode uses a separate import for every word. This will help
    /// webpack and the minifier to move words only into those chunks where
    /// they are needed.
    ///
    /// ```javascript
    /// import __i18n_ItemNumber from "../translations.i18n?ItemNumber"
    /// ```
    Production,
}

impl TryFrom<&str> for Environment {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            _ => Err(format!("{value} is not a valid environment")),
        }
    }
}
