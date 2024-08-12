use std::ops::Deref;

use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::{AnalyzerPlugin, PluginSuite};
use cairo_lang_syntax::node::ast::{Expr, ExprMatch, Pattern, Statement};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{TypedStablePtr, TypedSyntaxNode};

pub fn cairo_lint_plugin_suite() -> PluginSuite {
    let mut suite = PluginSuite::default();
    suite.add_analyzer_plugin::<CairoLint>();
    suite
}
#[derive(Debug, Default)]
pub struct CairoLint;

#[derive(Debug, PartialEq)]
pub enum CairoLintKind {
    DestructMatch,
    MatchForEquality,
    EmptyWithBrackets,
    Unknown,
}

pub fn diagnostic_kind_from_message(message: &str) -> CairoLintKind {
    match message {
        CairoLint::DESTRUCT_MATCH => CairoLintKind::DestructMatch,
        CairoLint::MATCH_FOR_EQUALITY => CairoLintKind::MatchForEquality,
        CairoLint::EMPTY_WITH_BRACKETS => CairoLintKind::EmptyWithBrackets,
        _ => CairoLintKind::Unknown,
    }
}

impl CairoLint {
    const DESTRUCT_MATCH: &'static str =
        "you seem to be trying to use `match` for destructuring a single pattern. Consider using `if let`";
    const MATCH_FOR_EQUALITY: &'static str =
        "you seem to be trying to use `match` for an equality check. Consider using `if`";
    const EMPTY_WITH_BRACKETS: &'static str = "enum variant has empty brackets";

    pub fn check_destruct_match(
        &self,
        db: &dyn SyntaxGroup,
        match_expr: &ExprMatch,
        diagnostics: &mut Vec<PluginDiagnostic>,
    ) {
        let arms = match_expr.arms(db).deref().elements(db);
        let mut is_single_armed = false;
        let mut is_destructuring = false;
        if arms.len() == 2 {
            for arm in arms {
                let patterns = arm.patterns(db).elements(db);
                match patterns[0].clone() {
                    Pattern::Underscore(_) => {
                        let tuple_expr = match arm.expression(db) {
                            Expr::Block(block_expr) => {
                                let statements = block_expr.statements(db).elements(db);
                                if statements.is_empty() {
                                    is_single_armed = true;
                                }
                                if statements.len() == 1 {
                                    match &statements[0] {
                                        Statement::Expr(statement_expr) => {
                                            if let Expr::Tuple(tuple_expr) = statement_expr.expr(db) {
                                                Some(tuple_expr)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    }
                                } else {
                                    None
                                }
                            }
                            Expr::Tuple(tuple_expr) => Some(tuple_expr),
                            _ => None,
                        };
                        is_single_armed = tuple_expr.is_some_and(|list| list.expressions(db).elements(db).is_empty())
                            || is_single_armed;
                    }

                    Pattern::Enum(pat) => {
                        is_destructuring = !pat.pattern(db).as_syntax_node().get_text(db).is_empty();
                    }
                    Pattern::Struct(pat) => {
                        is_destructuring = !pat.as_syntax_node().get_text(db).is_empty();
                    }
                    _ => (),
                };
            }
        };
        match (is_single_armed, is_destructuring) {
            (true, false) => diagnostics.push(PluginDiagnostic {
                stable_ptr: match_expr.stable_ptr().untyped(),
                message: Self::MATCH_FOR_EQUALITY.to_string(),
                severity: Severity::Warning,
            }),
            (true, true) => diagnostics.push(PluginDiagnostic {
                stable_ptr: match_expr.stable_ptr().untyped(),
                message: Self::DESTRUCT_MATCH.to_string(),
                severity: Severity::Warning,
            }),
            (_, _) => (),
        }
    }
    pub fn check_variant(&self, db: &dyn SyntaxGroup, variant: &Pattern) -> Option<PluginDiagnostic> {
        if self.is_redundant_parentheses(db, variant) {
            return Some(PluginDiagnostic {
                stable_ptr: variant.stable_ptr().untyped(),
                message: "This enum variant has redundant parentheses and can be simplified.".to_string(),
                severity: Severity::Warning,
            });
        }
        None
    }

    fn is_redundant_parentheses(&self, db: &dyn SyntaxGroup, pattern: &Pattern) -> bool {
        let syntax_node = pattern.as_syntax_node();
        // Check if the pattern is of type `PatternEnum`
        if syntax_node.kind(db) == SyntaxKind::PatternEnum {
            pattern.as_syntax_node().get_text(db).contains("()")
        } else {
            false
        }
    }
}

impl AnalyzerPlugin for CairoLint {
    fn diagnostics(&self, db: &dyn SemanticGroup, module_id: ModuleId) -> Vec<PluginDiagnostic> {
        let mut diags = Vec::new();
        let Ok(items) = db.module_items(module_id) else {
            return diags;
        };
        for item in items.iter() {
            match item {
                ModuleItemId::FreeFunction(func_id) => {
                    //
                    let func = db.module_free_function_by_id(*func_id).unwrap().unwrap();
                    let descendants = func.as_syntax_node().descendants(db.upcast());
                    for descendant in descendants.into_iter() {
                        match descendant.kind(db.upcast()) {
                            SyntaxKind::ExprMatch => self.check_destruct_match(
                                db.upcast(),
                                &ExprMatch::from_syntax_node(db.upcast(), descendant),
                                &mut diags,
                            ),
                            SyntaxKind::PatternEnum => {
                                let pattern = Pattern::from_syntax_node(db.upcast(), descendant);
                                if let Some(diag) = self.check_variant(db.upcast(), &pattern) {
                                    diags.push(diag);
                                }
                            }
                            SyntaxKind::ItemExternFunction => (),
                            _ => (),
                        }
                    }
                }
                ModuleItemId::ExternFunction(_) => (),
                _ => (),
            }
        }
        diags
    }
}
