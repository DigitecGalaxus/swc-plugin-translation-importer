use serde::{Deserialize, Serialize};

/// Static plugin configuration.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Path to `translations.i18n` cache.
    pub translation_cache: String,
    /// The target environment.
    pub environment: Environment,
}

/// The target environment.
#[derive(Serialize, Deserialize)]
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
    /// webpack and terser to move words only into those chunks where they are
    /// needed.
    ///
    /// ```javascript
    /// import __i18n_ItemNumber from "../translations.i18n?ItemNumber"
    /// ```
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Environment::Development
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_serialization() {
        let config = Config {
            translation_cache: "whatever".into(),
            environment: Environment::Production,
        };

        let serialized = serde_json::to_string(&config).unwrap();

        assert_eq!(
            r#"{"translationCache":"whatever","environment":"production"}"#,
            &serialized
        );
    }
}
