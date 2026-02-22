//! TypeScript parser with sugarcube syntax extensions.
//!
//! Wraps the standard SWC parser and preprocesses source to handle
//! sugarcube-specific syntax:
//!
//! - Pipeline expressions (`a |> f`)
//! - Cons expressions (`x :: xs`)
//! - HKT type parameters (`F<_>`)
//!
//! The preprocessor rewrites custom syntax at the text level before
//! passing to the standard SWC parser.

pub mod parse;
pub mod preprocess;

pub use parse::parse_sugarcube;
