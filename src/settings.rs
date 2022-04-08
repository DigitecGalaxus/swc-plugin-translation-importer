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
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// The target environment (from `NODE_ENV`).
    pub env_name: Environment,
    /// The name of the current file.
    pub filename: String,
}

/// The target environment.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_serialization() {
        let context = Context {
            env_name: Environment::Production,
            filename: "irrelevant".into(),
        };

        let serialized = serde_json::to_string(&context).unwrap();

        assert_eq!(
            r#"{"envName":"production","filename":"irrelevant"}"#,
            &serialized
        );
    }
}
