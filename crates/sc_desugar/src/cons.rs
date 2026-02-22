//! Desugaring for the cons operator (`::`).
//!
//! `a :: b` → `__binop__(a, "::", b)`

use sc_ast::ScBinExpr;
use swc_ecma_ast as ast;

use crate::pipeline::make_binop_call;

/// Desugar a cons binary expression into a `__binop__` call.
pub fn desugar_cons(expr: &ScBinExpr) -> ast::Expr {
    make_binop_call(expr.span, &expr.left, "::", &expr.right)
}
