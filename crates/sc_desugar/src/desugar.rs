//! Top-level desugaring entry point.
//!
//! Takes a parsed module and rewrites sugarcube-specific constructs
//! into standard TypeScript AST nodes.

use swc_ecma_ast as ast;

/// Desugar all sugarcube extensions in a module.
///
/// Currently a placeholder that returns the module unchanged.
/// As the parser is extended to produce sugarcube-specific AST nodes,
/// this function will apply the pipeline, cons, and HKT transforms.
pub fn desugar_module(module: ast::Module) -> ast::Module {
    // TODO: Apply pipeline desugaring
    // TODO: Apply cons desugaring
    // TODO: Apply HKT desugaring
    module
}
