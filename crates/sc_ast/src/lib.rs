//! Extended ECMAScript/TypeScript AST for sugarcube.
//!
//! Re-exports the standard SWC AST and adds custom nodes for:
//! - Pipeline operator (`|>`)
//! - Cons operator (`::`)
//! - HKT type parameters (`F<_>`)

pub use swc_ecma_ast::*;

use serde::{Deserialize, Serialize};
use swc_common::Span;

/// Binary operators added by sugarcube beyond the standard JS/TS set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScBinaryOp {
    /// Pipeline operator `|>` — precedence 1 (lowest), left-associative.
    Pipeline,
    /// Cons operator `::` — precedence 5, right-associative.
    Cons,
}

impl std::fmt::Display for ScBinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScBinaryOp::Pipeline => write!(f, "|>"),
            ScBinaryOp::Cons => write!(f, "::"),
        }
    }
}

/// A binary expression using a sugarcube-specific operator.
///
/// Contains boxed SWC `Expr` nodes. Not serde-serializable by default;
/// use the JSON AST dump via `swc_ecma_ast`'s own serde support instead.
#[derive(Debug, Clone, PartialEq)]
pub struct ScBinExpr {
    pub span: Span,
    pub op: ScBinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

/// Marker indicating a type parameter was declared with HKT syntax (`F<_>`).
///
/// During desugaring, references like `F<A>` within the declaring scope
/// are rewritten to `$<F, A>`.
#[derive(Debug, Clone, PartialEq)]
pub struct HktTypeParam {
    pub span: Span,
    pub name: String,
}

/// Feature flags controlling which sugarcube extensions are active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScSyntax {
    pub pipeline: bool,
    pub cons: bool,
    pub hkt: bool,
}

impl Default for ScSyntax {
    fn default() -> Self {
        Self {
            pipeline: true,
            cons: true,
            hkt: true,
        }
    }
}
