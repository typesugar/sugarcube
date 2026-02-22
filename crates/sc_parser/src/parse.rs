use anyhow::Result;
use sc_ast::ScSyntax;
use swc_common::{
    comments::SingleThreadedComments, errors::Handler, sync::Lrc, FileName, SourceMap,
};
use swc_ecma_ast::EsVersion;
use swc_ecma_parser::{Syntax, TsSyntax};

use crate::preprocess;

/// Result of parsing a sugarcube source file.
pub struct ParseResult {
    pub module: swc_ecma_ast::Module,
    pub comments: SingleThreadedComments,
    pub source_map: Lrc<SourceMap>,
    /// The preprocessed source (after sugarcube rewrites, before SWC parsing).
    pub preprocessed_source: String,
}

/// Parse a TypeScript/TSX source string with sugarcube extensions.
///
/// 1. Preprocess: rewrite `|>`, `::`, `F<_>` to standard TS at text level.
/// 2. Parse: feed the preprocessed text to the standard SWC parser.
pub fn parse_sugarcube(
    source: &str,
    filename: &str,
    syntax: &ScSyntax,
) -> Result<ParseResult> {
    let preprocessed = preprocess::preprocess(source, syntax);

    let source_map: Lrc<SourceMap> = Default::default();
    let source_file = source_map.new_source_file(
        Lrc::new(FileName::Custom(filename.to_string())),
        preprocessed.clone(),
    );

    let comments = SingleThreadedComments::default();

    let handler = Handler::with_emitter_writer(Box::new(std::io::stderr()), Some(source_map.clone()));

    let is_tsx = filename.ends_with(".tsx");
    let ts_syntax = Syntax::Typescript(TsSyntax {
        tsx: is_tsx,
        decorators: true,
        ..Default::default()
    });

    let module = swc_ecma_parser::parse_file_as_module(
        &source_file,
        ts_syntax,
        EsVersion::latest(),
        Some(&comments),
        &mut vec![],
    )
    .map_err(|e| {
        e.into_diagnostic(&handler).emit();
        anyhow::anyhow!("failed to parse {filename}")
    })?;

    Ok(ParseResult {
        module,
        comments,
        source_map,
        preprocessed_source: preprocessed,
    })
}
