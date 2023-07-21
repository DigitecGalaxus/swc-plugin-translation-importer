#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::BTreeSet;
use swc_core::{
    common::DUMMY_SP,
    ecma::{
        ast::*,
        visit::{as_folder, FoldWith, VisitMut, VisitMutWith},
    },
    plugin::{
        metadata::TransformPluginMetadataContextKind, plugin_transform,
        proxies::TransformPluginProgramMetadata,
    },
};

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
                src: Box::new(Str {
                    span: DUMMY_SP,
                    value: format!("{}?dev", self.config.translation_cache).into(),
                    raw: None,
                }),
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
                    src: Box::new(Str {
                        span: DUMMY_SP,
                        value: format!(
                            "{}?={}",
                            self.config.translation_cache,
                            helpers::strip_prefix(variable_name)
                        )
                        .into(),
                        raw: None,
                    }),
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
                match id.sym.as_ref() {
                    "__" | "__icu" | "__md" | "__byLanguage" | "__icuByLanguage"
                    | "__mdByLanguage" => {
                        let first_argument = call_expr.args.first_mut().unwrap_or_else(|| panic!(
                            r#"Translation function requires an argument e.g. __("Hello World") in {}"#,
                            self.context.filename));

                        if let Expr::Lit(Lit::Str(translation_key)) = &mut *first_argument.expr {
                            let variable_name =
                                helpers::generate_variable_name(&translation_key.value);
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
                    _ => {}
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
    let config: Config = serde_json::from_str(
        &metadata
            .get_transform_plugin_config()
            .expect("failed to get plugin config for swc-plugin-translation-importer"),
    )
    .expect("failed to parse plugin config");

    let context = Context {
        filename: metadata
            .get_context(&TransformPluginMetadataContextKind::Filename)
            .expect("failed to get filename"),
        env_name: Environment::try_from(
            metadata
                .get_context(&TransformPluginMetadataContextKind::Env)
                .expect("failed to get env")
                .as_str(),
        )
        .expect("failed to parse environment"),
    };

    program.fold_with(&mut as_folder(TransformVisitor::new(config, context)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::ecma::{
        transforms::testing::test,
        visit::{as_folder, Fold},
    };

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
        Default::default(),
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
        Default::default(),
        |_| transform_visitor(Environment::Test),
        no_transpile_test_mode,
        SOURCE,
        SOURCE
    );

    test!(
        Default::default(),
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
        Default::default(),
        |_| transform_visitor(Environment::Development),
        nested_code,
        r#"const foo = bar(__("other_translation"));"#,
        r#"import { __i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df } from "../../.cache/translations.i18n?dev";
        const foo = bar(__(__i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df || "other_translation"));"#
    );

    test!(
        Default::default(),
        |_| transform_visitor(Environment::Development),
        icu_code,
        r#"const foo = __icu("Buy n pieces", { numberOfProducts: p.minAmount });"#,
        r#"import { __i18n_d1b6589d9678069ddad863d441fe188e5362130e5be23215a5ff66458ef94441 } from "../../.cache/translations.i18n?dev";
        const foo = __icu(__i18n_d1b6589d9678069ddad863d441fe188e5362130e5be23215a5ff66458ef94441 || "Buy n pieces", {
            numberOfProducts: p.minAmount
        });"#
    );

    test!(
        Default::default(),
        |_| transform_visitor(Environment::Development),
        markdown_code,
        r#"const foo = __md("other_translation");"#,
        r#"import { __i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df } from "../../.cache/translations.i18n?dev";
        const foo = __md(__i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df || "other_translation");"#
    );

    test!(
        Default::default(),
        |_| transform_visitor(Environment::Development),
        by_language_code,
        r#"const foo = __byLanguage("other_translation");"#,
        r#"import { __i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df } from "../../.cache/translations.i18n?dev";
        const foo = __byLanguage(__i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df || "other_translation");"#
    );

    test!(
        Default::default(),
        |_| transform_visitor(Environment::Development),
        icu_by_language,
        r#"const foo = __icuByLanguage("Pluralized items ordered", language, { category, stockCount });"#,
        r#"import { __i18n_5dec8611ff95ef5ade67844d5a0b16a7c2020762363f0231d8bf4ba7aeca7474 } from "../../.cache/translations.i18n?dev";
        const foo = __icuByLanguage(__i18n_5dec8611ff95ef5ade67844d5a0b16a7c2020762363f0231d8bf4ba7aeca7474 || "Pluralized items ordered", language, {
            category,
            stockCount
        });"#
    );

    test!(
        Default::default(),
        |_| transform_visitor(Environment::Development),
        md_by_language,
        r#"const foo = __mdByLanguage("other_translation");"#,
        r#"import { __i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df } from "../../.cache/translations.i18n?dev";
        const foo = __mdByLanguage(__i18n_c4622ceee64504cbc2c5b05ecb9e66c4235c6d03826437c16da0ce2e061479df || "other_translation");"#
    );

    test!(
        Default::default(),
        |_| transform_visitor(Environment::Development),
        no_usages,
        r#"const foo = "Hello, world!";"#,
        r#"const foo = "Hello, world!";"#
    );
}
