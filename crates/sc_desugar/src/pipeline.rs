//! Desugaring for the pipeline operator (`|>`).
//!
//! `a |> f` → `__binop__(a, "|>", f)`

use sc_ast::ScBinExpr;
use swc_common::Span;
use swc_ecma_ast as ast;

/// Desugar a pipeline binary expression into a `__binop__` call.
pub fn desugar_pipeline(expr: &ScBinExpr) -> ast::Expr {
    make_binop_call(expr.span, &expr.left, "|>", &expr.right)
}

/// Build `__binop__(left, op_str, right)`.
pub(crate) fn make_binop_call(
    span: Span,
    left: &ast::Expr,
    op_str: &str,
    right: &ast::Expr,
) -> ast::Expr {
    let callee_ident = ast::Ident::new_no_ctxt("__binop__".into(), span);

    ast::Expr::Call(ast::CallExpr {
        span,
        callee: ast::Callee::Expr(Box::new(ast::Expr::Ident(callee_ident))),
        args: vec![
            ast::ExprOrSpread {
                spread: None,
                expr: Box::new(left.clone()),
            },
            ast::ExprOrSpread {
                spread: None,
                expr: Box::new(ast::Expr::Lit(ast::Lit::Str(ast::Str {
                    span,
                    value: op_str.into(),
                    raw: None,
                }))),
            },
            ast::ExprOrSpread {
                spread: None,
                expr: Box::new(right.clone()),
            },
        ],
        type_args: None,
        ..Default::default()
    })
}
