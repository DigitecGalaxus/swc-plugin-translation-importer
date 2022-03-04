use serde::{Deserialize, Serialize};
use swc_plugin::{ast::*, plugin_transform};

/// Static plugin configuration.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Path to `translations.i18n` cache.
    pub translation_cache: String,
}

struct TransformVisitor {
    config: Config,
}

impl TransformVisitor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl VisitMut for TransformVisitor {}

/// Transforms a [`Program`].
///
/// # Arguments
///
/// - `program` - The SWC [`Program`] to transform.
/// - `config` - [`Config`] as JSON.
#[plugin_transform]
pub fn process_transform(program: Program, config: String, _context: String) -> Program {
    let config: Config = serde_json::from_str(&config).expect("failed to parse plugin config");

    program.fold_with(&mut as_folder(TransformVisitor::new(config)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_ecma_transforms_testing::test;

    fn transform_visitor(config: Config) -> impl 'static + Fold + VisitMut {
        as_folder(TransformVisitor::new(config))
    }

    test!(
        swc_ecma_parser::Syntax::default(),
        |_| transform_visitor(Config {
            translation_cache: "testdata/translations.i18n".into()
        }),
        does_absolutely_nothing,
        r#"const t = "Hello, world!";"#,
        r#"const t = "Hello, world!";"#
    );
}
