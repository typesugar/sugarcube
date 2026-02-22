//! Extended lexer for sugarcube.
//!
//! Wraps the standard SWC lexer and post-processes its token stream to
//! merge adjacent tokens into sugarcube-specific tokens:
//!
//! - `|` + `>` → Pipeline (`|>`)
//! - `:` + `:` → Cons (`::`)

use sc_ast::{ScBinaryOp, ScSyntax};
use swc_common::Span;
use swc_ecma_parser::token::{BinOpToken, Token, TokenAndSpan};

/// A token produced by the sugarcube lexer.
///
/// Either a standard SWC token or a sugarcube-specific operator.
#[derive(Debug, Clone, PartialEq)]
pub enum ScToken {
    /// A standard token from the SWC lexer.
    Standard(Token),
    /// A sugarcube binary operator merged from adjacent standard tokens.
    ScOperator(ScBinaryOp),
}

/// A token with its source span.
#[derive(Debug, Clone)]
pub struct ScTokenAndSpan {
    pub token: ScToken,
    pub span: Span,
    pub had_line_break: bool,
}

/// Merge adjacent standard tokens into sugarcube operators.
///
/// Given a stream of SWC tokens, looks for sequences like `|` `>` (without
/// intervening whitespace/tokens) and merges them into `ScToken::ScOperator(Pipeline)`.
pub fn merge_sc_tokens(tokens: &[TokenAndSpan], syntax: &ScSyntax) -> Vec<ScTokenAndSpan> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        if syntax.pipeline
            && i + 1 < tokens.len()
            && matches!(tokens[i].token, Token::BinOp(BinOpToken::BitOr))
            && matches!(tokens[i + 1].token, Token::BinOp(BinOpToken::Gt))
            && tokens[i].span.hi == tokens[i + 1].span.lo
        {
            let span = Span::new(tokens[i].span.lo, tokens[i + 1].span.hi);
            result.push(ScTokenAndSpan {
                token: ScToken::ScOperator(ScBinaryOp::Pipeline),
                span,
                had_line_break: tokens[i].had_line_break,
            });
            i += 2;
            continue;
        }

        if syntax.cons
            && i + 1 < tokens.len()
            && matches!(tokens[i].token, Token::Colon)
            && matches!(tokens[i + 1].token, Token::Colon)
            && tokens[i].span.hi == tokens[i + 1].span.lo
        {
            let span = Span::new(tokens[i].span.lo, tokens[i + 1].span.hi);
            result.push(ScTokenAndSpan {
                token: ScToken::ScOperator(ScBinaryOp::Cons),
                span,
                had_line_break: tokens[i].had_line_break,
            });
            i += 2;
            continue;
        }

        result.push(ScTokenAndSpan {
            token: ScToken::Standard(tokens[i].token.clone()),
            span: tokens[i].span,
            had_line_break: tokens[i].had_line_break,
        });
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sc_syntax_default_enables_all() {
        let s = ScSyntax::default();
        assert!(s.pipeline);
        assert!(s.cons);
        assert!(s.hkt);
    }
}
