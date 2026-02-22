//! Desugaring pass that rewrites sugarcube AST nodes into standard TypeScript.
//!
//! Transforms:
//! - `a |> f`   → `__binop__(a, "|>", f)`
//! - `a :: b`   → `__binop__(a, "::", b)`
//! - `F<_>` HKT → strips `<_>` from decl, rewrites `F<A>` to `$<F, A>` in scope

pub mod pipeline;
pub mod cons;
pub mod hkt;
pub mod desugar;

pub use desugar::desugar_module;
