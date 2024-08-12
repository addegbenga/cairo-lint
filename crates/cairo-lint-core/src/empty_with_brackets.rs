use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::Pattern;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{TypedStablePtr, TypedSyntaxNode};

#[derive(Default)]
pub struct EmptyWithBrackets;

impl EmptyWithBrackets {
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
