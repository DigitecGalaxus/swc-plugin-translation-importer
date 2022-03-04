use sha2::{Digest, Sha256};

const TRANSLATION_PREFIX: &str = "__i18n_";

/// Turns `translation_key` into a variable name by prefixing its SHA256 hash.
pub fn generate_variable_name(translation_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(translation_key);
    let hash = hasher.finalize();

    format!("{TRANSLATION_PREFIX}{hash:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_variable_name1() {
        let variable_name = generate_variable_name("Hello World!!");
        assert_eq!(
            "__i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9",
            variable_name
        );
    }

    #[test]
    fn generate_variable_name2() {
        let variable_name = generate_variable_name("Hello World??");
        assert_eq!(
            "__i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c",
            variable_name
        );
    }
}
