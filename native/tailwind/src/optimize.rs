//! The last pass on the tree before the print: `optimizeAst` and
//! `handleNesting` of `ast.ts` of Tailwind CSS.

use crate::ast::AstNode;
use crate::design_system::DesignSystem;

/// Make the tree ready for the printer.
pub fn optimize_ast(ast: Vec<AstNode>, _design_system: &DesignSystem, _polyfills: crate::Polyfills) -> Vec<AstNode> {
    ast
}
