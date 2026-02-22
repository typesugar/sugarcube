//! Text-level preprocessor that rewrites sugarcube syntax into standard TypeScript.
//!
//! Runs before the SWC parser to handle custom syntax that SWC can't parse.
//! This mirrors typesugar's preprocessor but in Rust.
//!
//! Processing order:
//! 1. HKT (`F<_>`) — rewrite declarations and usages
//! 2. Pipeline (`|>`) and Cons (`::`) — rewrite operators

use sc_ast::ScSyntax;

mod hkt_pass;
mod operator_pass;
mod util;

/// Preprocess a sugarcube source string, rewriting custom syntax to standard TS.
pub fn preprocess(source: &str, syntax: &ScSyntax) -> String {
    let mut result = source.to_string();

    if syntax.hkt {
        result = hkt_pass::rewrite_hkt(&result);
    }

    if syntax.pipeline || syntax.cons {
        result = operator_pass::rewrite_operators(&result, syntax);
    }

    result
}
