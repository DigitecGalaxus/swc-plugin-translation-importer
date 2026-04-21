#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::BTreeSet;
use swc_core::{
    common::DUMMY_SP,
    ecma::{
        ast::*,
        visit::{visit_mut_pass, VisitMut, VisitMutWith},
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
                        ctxt: Default::default(),
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
                with: None,
                phase: ImportPhase::Evaluation,
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
                        ctxt: Default::default(),
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
                    with: None,
                    phase: ImportPhase::Evaluation,
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

        let imports = self.imports();
        // Abort early if there aren't any translations in the file
        if imports.is_empty() {
            return;
        }

        // We add our imports just before the first other import, which ensures that we'll come
        // after any "use client" or similar directives that need to be first in the file.
        // https://github.com/DigitecGalaxus/swc-plugin-translation-importer/issues/13
        let insert_index = get_first_import_index(module_items).unwrap_or(0);
        module_items.splice(insert_index..insert_index, self.imports());
    }

    /// Object literals whose only (non-computed, non-spread) properties are
    /// named `__`, `__icu`, or `__md` are treated as translator objects:
    /// recursion is skipped so forwarding calls like
    /// `__byLanguage(key, lang, ...)` inside them don't trip the
    /// "first argument must be a string literal" check.
    ///
    /// Mixing translation property names with anything else in the same
    /// object literal is rejected — it's almost always a mistake, and
    /// silently skipping or partially transforming such an object is worse
    /// than a loud failure.
    fn visit_mut_object_lit(&mut self, obj: &mut ObjectLit) {
        let mut has_translation_helper_properties = false;
        for prop in &obj.props {
            let matched = match prop {
                PropOrSpread::Prop(prop) => match &**prop {
                    Prop::KeyValue(kv) => is_translation_property(&kv.key),
                    Prop::Method(m) => is_translation_property(&m.key),
                    _ => false,
                },
                PropOrSpread::Spread(_) => false,
            };
            if matched {
                has_translation_helper_properties = true;
            } else if has_translation_helper_properties {
                // If we already have translation helper properties, then any non-matching property is a problem
                panic!(
                    r#"Object literal mixes translation properties (__, __icu, __md) with other properties in {}; keep the translator object pure"#,
                    self.context.filename
                );
            }
        }
        // If this object has translation helper properties, we assume it's a translator wrapper and allow
        // calls like __byLanguage to be nested inside it without panicking
        if !has_translation_helper_properties {
            obj.visit_mut_children_with(self);
        }
    }

    fn visit_mut_call_expr(&mut self, call_expr: &mut CallExpr) {
        if let Callee::Expr(expr) = &mut call_expr.callee {
            if let Expr::Ident(id) = &mut **expr {
                match id.sym.as_str() {
                    "__" | "__icu" | "__md" | "__byLanguage" | "__icuByLanguage"
                    | "__mdByLanguage" => {
                        let first_argument = call_expr.args.first_mut().unwrap_or_else(|| panic!(
                            r#"Translation function requires an argument e.g. __("Hello World") in {}"#,
                            self.context.filename));

                        if let Expr::Lit(Lit::Str(translation_key)) = &mut *first_argument.expr {
                            let variable_name = helpers::generate_variable_name(
                                translation_key
                                    .value
                                    .as_str()
                                    .expect("translation key must be valid UTF-8"),
                            );
                            let variable_identifier = Expr::Ident(Ident {
                                ctxt: Default::default(),
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

/// Checks whether a property name is one of the translation helper properties (`__`, `__icu`, or `__md`).
fn is_translation_property(key: &PropName) -> bool {
    match key {
        PropName::Ident(ident) => is_translation_name(ident.sym.as_str()),
        PropName::Str(s) => s.value.as_str().is_some_and(is_translation_name),
        _ => false,
    }
}

fn is_translation_name(name: &str) -> bool {
    matches!(name, "__" | "__icu" | "__md")
}

/// Returns the index of the first import within the module items if one exists.
fn get_first_import_index(module_items: &[ModuleItem]) -> Option<usize> {
    module_items
        .iter()
        .position(|module_item| is_import_decl(module_item).unwrap_or(false))
}

/// Checks whether a module item is an import declaration.
fn is_import_decl(module_item: &ModuleItem) -> Option<bool> {
    module_item.as_module_decl()?.as_import().map(|_| true)
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

    program.apply(visit_mut_pass(
        &mut (TransformVisitor::new(config, context)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::ecma::{
        transforms::testing::test,
        visit::{visit_mut_pass, VisitMutPass},
    };

    const SOURCE: &str = r#"var foo = 1;
if (foo) console.log(foo);
__("Hello World!!");
__("Hello World??");"#;

    fn transform_visitor(environment: Environment) -> VisitMutPass<TransformVisitor> {
        visit_mut_pass(TransformVisitor::new(
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
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        transpile_dev_mode,
        SOURCE
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Test),
        no_transpile_test_mode,
        SOURCE
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Production),
        transpile_prod_mode,
        SOURCE
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        nested_code,
        r#"const foo = bar(__("other_translation"));"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        icu_code,
        r#"const foo = __icu("Buy n pieces", { numberOfProducts: p.minAmount });"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        markdown_code,
        r#"const foo = __md("other_translation");"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        by_language_code,
        r#"const foo = __byLanguage("other_translation");"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        icu_by_language,
        r#"const foo = __icuByLanguage("Pluralized items ordered", language, { category, stockCount });"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        md_by_language,
        r#"const foo = __mdByLanguage("other_translation");"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        no_usages,
        r#"const foo = "Hello, world!";"#
    );

    // Forwarding wrapper: object literal with only translation-named props is
    // skipped entirely, so the non-literal first argument of `__byLanguage`
    // does not cause a panic.
    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        translator_object_wrapper,
        r#"export const buildTranslatorByLanguage = (language) => ({
            __: (key, ...interpolations) => __byLanguage(key, language, ...interpolations),
            __icu: (key, icuMessageData) => __icuByLanguage(key, language, icuMessageData),
        });
        const greeting = (lang) => {
            const { __ } = buildTranslatorByLanguage(lang);
            return __("Hello [0]!", "World");
        };"#
    );

    // Mixing translator wrapper props with anything else in the same object
    // literal is rejected. This catches cases where a wrapper accidentally
    // gained an extra property — failing loudly is safer than partial work.
    #[test]
    #[should_panic(expected = "mixes translation properties")]
    fn mixed_object_panics() {
        fn ident_key(name: &str) -> PropName {
            PropName::Ident(IdentName {
                span: DUMMY_SP,
                sym: name.into(),
            })
        }
        fn prop(key: &str, value: Expr) -> PropOrSpread {
            PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
                key: ident_key(key),
                value: Box::new(value),
            })))
        }

        let mut obj = ObjectLit {
            span: DUMMY_SP,
            props: vec![
                prop(
                    "__",
                    Expr::Arrow(ArrowExpr {
                        span: DUMMY_SP,
                        ctxt: Default::default(),
                        params: vec![],
                        body: Box::new(BlockStmtOrExpr::Expr(Box::new(Expr::Lit(Lit::Null(
                            Null { span: DUMMY_SP },
                        ))))),
                        is_async: false,
                        is_generator: false,
                        type_params: None,
                        return_type: None,
                    }),
                ),
                prop(
                    "foo",
                    Expr::Lit(Lit::Num(Number {
                        span: DUMMY_SP,
                        value: 42.0,
                        raw: None,
                    })),
                ),
            ],
        };

        let mut visitor = TransformVisitor::new(
            Config {
                translation_cache: "../../.cache/translations.i18n".into(),
            },
            Context {
                env_name: Environment::Development,
                filename: "mixed.js".into(),
            },
        );
        visitor.visit_mut_object_lit(&mut obj);
    }

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        use_client,
        r#""use client";
        import { useTranslate } from "next-i18n";
        const { __ } = useTranslate(lang);
        __("Hello World!!");"#
    );

    test!(
        module,
        Default::default(),
        |_| transform_visitor(Environment::Development),
        use_strict,
        r#""use strict";
        import { useTranslate } from "next-i18n";
        import { unused } from "unused";
        const { __ } = useTranslate(lang);
        __("Hello World!!");"#
    );
}
