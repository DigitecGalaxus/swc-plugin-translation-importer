#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::BTreeSet;
use swc_plugin::{ast::*, plugin_transform, syntax_pos::DUMMY_SP, TransformPluginProgramMetadata};

mod helpers;
mod settings;

pub use settings::{Config, Context, Environment};

struct TransformVisitor {
    config: Config,
    context: Context,
    import_variables: BTreeSet<String>,
}

impl TransformVisitor {
    pub fn new(config: Config, context: Context) -> Self {
        Self {
            config,
            context,
            import_variables: BTreeSet::new(),
        }
    }

    /// Returns the appropriate import declarations depending on the
    /// environment.
    fn imports(&self) -> Vec<ModuleItem> {
        match self.context.env_name {
            Environment::Development => self.dev_imports(),
            _ => self.prod_imports(),
        }
    }

    /// Returns the import declarations (actually it's a single one) for dev.
    ///
    /// ```javascript
    /// import { __i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9, __i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c } from "../../.cache/translations.i18n?dev";
    /// ```
    fn dev_imports(&self) -> Vec<ModuleItem> {
        let import_specifiers = self
            .import_variables
            .iter()
            .map(|variable_name| {
                ImportSpecifier::Named(ImportNamedSpecifier {
                    span: DUMMY_SP,
                    local: Ident {
                        span: DUMMY_SP,
                        sym: variable_name.clone().into(),
                        optional: false,
                    },
                    imported: None,
                    is_type_only: false,
                })
            })
            .collect::<Vec<ImportSpecifier>>();

        if import_specifiers.is_empty() {
            vec![]
        } else {
            vec![ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                span: DUMMY_SP,
                specifiers: import_specifiers,
                src: format!("{}?dev", self.config.translation_cache).into(),
                type_only: false,
                asserts: None,
            }))]
        }
    }

    /// Returns the import declarations for prod.
    ///
    /// ```javascript
    /// import __i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9 from "../../.cache/translations.i18n?=096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9";
    /// import __i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c from "../../.cache/translations.i18n?=b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c";
    /// ```
    fn prod_imports(&self) -> Vec<ModuleItem> {
        self.import_variables
            .iter()
            .map(|variable_name| {
                ImportSpecifier::Default(ImportDefaultSpecifier {
                    span: DUMMY_SP,
                    local: Ident {
                        span: DUMMY_SP,
                        sym: variable_name.clone().into(),
                        optional: false,
                    },
                })
            })
            .zip(self.import_variables.iter())
            .map(|(import_specifier, variable_name)| {
                ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
                    span: DUMMY_SP,
                    specifiers: vec![import_specifier],
                    src: format!(
                        "{}?={}",
                        self.config.translation_cache,
                        helpers::strip_prefix(variable_name)
                    )
                    .into(),
                    type_only: false,
                    asserts: None,
                }))
            })
            .collect()
    }
}

impl VisitMut for TransformVisitor {
    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        // Ignore this plugin for Jest runs
        if self.context.env_name == Environment::Test {
            return;
        }

        module_items.visit_mut_children_with(self);

        // Insert imports for encountered translations at top of file
        module_items.splice(..0, self.imports());
    }

    fn visit_mut_call_expr(&mut self, call_expr: &mut CallExpr) {
        if let Callee::Expr(expr) = &mut call_expr.callee {
            if let Expr::Ident(id) = &mut **expr {
                if &id.sym == "__" {
                    let first_argument = call_expr.args.first_mut().unwrap_or_else(|| panic!(
                        r#"Translation function requires an argument e.g. __("Hello World") in {}"#,
                        self.context.filename));

                    if let Expr::Lit(Lit::Str(translation_key)) = &mut *first_argument.expr {
                        let variable_name = helpers::generate_variable_name(&translation_key.value);
                        let variable_identifier = Expr::Ident(Ident {
                            span: DUMMY_SP,
                            sym: variable_name.clone().into(),
                            optional: false,
                        });

                        let argument = match self.context.env_name {
                            // For development add fallback on the key for unknown translations
                            // __(__i18n_Hello || "Hello")
                            Environment::Development => Expr::Bin(BinExpr {
                                span: DUMMY_SP,
                                op: BinaryOp::LogicalOr,
                                left: Box::new(variable_identifier),
                                right: Box::new(Expr::Lit(Lit::Str(translation_key.clone()))),
                            }),
                            // For production it's just the variable name of the translation
                            // __(__i18n_Hello)
                            _ => variable_identifier,
                        };

                        call_expr.args[0] = ExprOrSpread {
                            spread: None,
                            expr: Box::new(argument),
                        };

                        // Remember variable name to generate import later
                        self.import_variables.insert(variable_name);
                    } else {
                        panic!(
                            r#"Translation function requires first argument to be a string e.g. __("Hello World") in {}"#,
                            self.context.filename
                        )
                    }
                }
            }
        }

        call_expr.visit_mut_children_with(self);
    }
}

/// Transforms a [`Program`].
///
/// # Arguments
///
/// - `program` - The SWC [`Program`] to transform.
/// - `config` - [`Config`] as JSON.
#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let config: Config =
        serde_json::from_str(&metadata.plugin_config).expect("failed to parse plugin config");
    let context: Context =
        serde_json::from_str(&metadata.transform_context).expect("failed to parse plugin context");

    program.fold_with(&mut as_folder(TransformVisitor::new(config, context)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_ecma_transforms_testing::test;

    const SOURCE: &str = r#"var foo = 1;
if (foo) console.log(foo);
__("Hello World!!");
__("Hello World??");"#;

    fn transform_visitor(environment: Environment) -> impl Fold {
        as_folder(TransformVisitor::new(
            Config {
                translation_cache: "../../.cache/translations.i18n".into(),
            },
            Context {
                env_name: environment,
                filename: "irrelevant".into(),
            },
        ))
    }

    test!(
        swc_ecma_parser::Syntax::default(),
        |_| transform_visitor(Environment::Development),
        transpile_dev_mode,
        SOURCE,
        r#"import { __i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9, __i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c } from "../../.cache/translations.i18n?dev";
var foo = 1;
if (foo) console.log(foo);
__(__i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9 || "Hello World!!");
__(__i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c || "Hello World??");"#
    );

    test!(
        swc_ecma_parser::Syntax::default(),
        |_| transform_visitor(Environment::Test),
        no_transpile_test_mode,
        SOURCE,
        SOURCE
    );

    test!(
        swc_ecma_parser::Syntax::default(),
        |_| transform_visitor(Environment::Production),
        transpile_prod_mode,
        SOURCE,
        r#"import __i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9 from "../../.cache/translations.i18n?=096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9";
import __i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c from "../../.cache/translations.i18n?=b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c";
var foo = 1;
if (foo) console.log(foo);
__(__i18n_096c0a72c31f9a2d65126d8e8a401a2ab2f2e21d0a282a6ffe6642bbef65ffd9);
__(__i18n_b357e65520993c7fdce6b04ccf237a3f88a0f77dbfdca784f5d646b5b59e498c);"#
    );

    test!(
        swc_ecma_parser::Syntax::default(),
        |_| transform_visitor(Environment::Development),
        nested_code,
        r#"const foo = bar(__("other_translation"));"#,
        r#"import { __i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df } from "../../.cache/translations.i18n?dev";
const foo = bar(__(__i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df || "other_translation"));"#
    );

    test!(
        swc_ecma_parser::Syntax::default(),
        |_| transform_visitor(Environment::Development),
        no_usages,
        r#"const foo = "Hello, world!";"#,
        r#"const foo = "Hello, world!";"#
    );
}
