use serde::{Deserialize, Serialize};
use swc_plugin::{ast::*, plugin_transform};

/// Static plugin configuration.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Config {}

/// Additional context for the plugin.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// The name of the current file.
    #[serde(default)]
    pub filename: Option<String>,
}

struct TransformVisitor {
    config: Config,
}

impl TransformVisitor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl VisitMut for TransformVisitor {
    fn visit_mut_var_declarator(&mut self, var_declarator: &mut VarDeclarator) {
        if let Pat::Ident(BindingIdent { id, .. }) = &mut var_declarator.name {
            let body = reqwest::blocking::get("http://127.0.0.1:8000/file.txt")
                .unwrap()
                .text()
                .unwrap();
            id.sym = body.into();
        }
    }
}

/// Transforms a [`Program`].
///
/// # Arguments
///
/// - `program` - The SWC [`Program`] to transform.
/// - `config` - [`Config`] as JSON.
/// - `context` - [`Context`] as JSON.
#[plugin_transform]
pub fn process_transform(program: Program, config: String, context: String) -> Program {
    let config: Config = serde_json::from_str(&config).expect("failed to parse plugin config");
    let context: Context = serde_json::from_str(&context).expect("failed to parse plugin context");

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
        |_| transform_visitor(Default::default()),
        does_absolutely_nothing,
        r#"const original_name = "Hello, world!";"#,
        r#"const nice = "Hello, world!";"#
    );
}
