# Contributing to Sugarcube

## Prerequisites

- **Rust toolchain** via [rustup](https://rustup.rs/) on the stable channel
- **Git** for version control
- Clone the repo: `git clone https://github.com/typesugar/sugarcube.git`

Verify your setup:

```bash
rustc --version   # stable channel
cargo --version
```

## Building

```bash
cargo build                            # build all crates
cargo build -p sc_cli                  # build just the CLI
cargo install --path crates/sc_cli     # install `sc` to ~/.cargo/bin
```

After installing, the `sc` binary is available globally:

```bash
sc preprocess input.ts                 # desugar to stdout
sc preprocess input.ts -o output.ts    # desugar to file
sc check input.ts                      # parse only, report errors
sc parse input.ts --ast                # dump AST as JSON
```

## Running Tests

```bash
cargo test                             # all tests (golden + roundtrip + unit)
cargo test -p sc_parser                # unit tests in the parser crate
cargo test -p sc_lexer                 # unit tests in the lexer crate
cargo test -p sc_test                  # golden-file and roundtrip tests only
```

### Golden-file tests

Tests live in `tests/fixtures/` as paired `.input.ts` / `.expected.ts` files. The harness runs each input through the full pipeline and diffs the output against the expected file.

To update expected files after an intentional output change:

```bash
SC_UPDATE_FIXTURES=1 cargo test
```

Always review the diff before committing updated fixtures.

### Roundtrip tests

Files in `tests/fixtures/roundtrip/` are run through the pipeline, then the output is re-parsed with a standard SWC parser (all sugarcube extensions disabled) to verify the output is legal TypeScript.

## Adding a New Syntax Extension

This is the most important workflow for keeping the fork sustainable. Use the pipeline operator (`|>`) as the reference — every step below links to the real code that was added for it.

### Step 1: Add a feature flag to `ScSyntax`

In `crates/sc_ast/src/lib.rs`, add a `bool` field to `ScSyntax` and enable it by default:

```rust
pub struct ScSyntax {
    pub pipeline: bool,
    pub cons: bool,
    pub hkt: bool,
    pub my_extension: bool,  // add your flag
}

impl Default for ScSyntax {
    fn default() -> Self {
        Self {
            pipeline: true,
            cons: true,
            hkt: true,
            my_extension: true,  // on by default
        }
    }
}
```

### Step 2: Add token types (if the extension introduces new lexemes)

Still in `crates/sc_ast/src/lib.rs`, add a variant to `ScBinaryOp` (for binary operators) or create a new AST node type:

```rust
pub enum ScBinaryOp {
    Pipeline,
    Cons,
    MyOp,  // new variant
}
```

Then in `crates/sc_lexer/src/lib.rs`, add a merge rule in `merge_sc_tokens()` that fuses adjacent SWC tokens into your operator. For example, pipeline merges `|` + `>`:

```rust
// In merge_sc_tokens(), after the existing merge rules:
if syntax.my_extension && i + 1 < tokens.len() {
    if matches!(tokens[i].token, Token::...)
        && matches!(tokens[i + 1].token, Token::...)
        && tokens[i].span.hi == tokens[i + 1].span.lo
    {
        let span = Span::new(tokens[i].span.lo, tokens[i + 1].span.hi);
        result.push(ScTokenAndSpan {
            token: ScToken::ScOperator(ScBinaryOp::MyOp),
            span,
            had_line_break: tokens[i].had_line_break,
        });
        i += 2;
        continue;
    }
}
```

The adjacency check (`span.hi == span.lo`) ensures tokens separated by whitespace aren't merged.

### Step 3: Add a preprocessing pass

Create a new file in `crates/sc_parser/src/preprocess/`, e.g. `my_extension_pass.rs`. Follow the pattern from `operator_pass.rs`:

1. Scan the source text for your syntax
2. Skip strings, comments, and template literals (use the `skip_non_code` helper)
3. Skip type contexts if the extension is expression-only
4. Rewrite to valid TypeScript

For binary operators, the standard desugaring target is `__binop__()`:

```
a MY_OP b  →  __binop__(a, "MY_OP", b)
```

Wire it into the preprocessing pipeline by calling your pass from the `preprocess()` function alongside the existing passes.

### Step 4: Add a desugar module (for AST-level transforms)

Create a module in `crates/sc_desugar/src/` and call it from `desugar_module()`. Currently, the text-level preprocessor handles most desugaring, so this module may be a passthrough. It exists as scaffolding for a future migration to AST-level processing.

### Step 5: Add golden-file tests

Create `tests/fixtures/<my_extension>/` with at least 5 test pairs:

```
tests/fixtures/my_extension/
  basic.input.ts              # simple usage
  basic.expected.ts           # expected output
  chained.input.ts            # chaining / nesting
  chained.expected.ts
  in_string.input.ts          # must NOT rewrite inside strings
  in_string.expected.ts
  in_type_context.input.ts    # must NOT rewrite in type annotations
  in_type_context.expected.ts
  with_pipeline.input.ts      # interaction with existing extensions
  with_pipeline.expected.ts
```

Generate expected files initially:

```bash
SC_UPDATE_FIXTURES=1 cargo test
```

Then review each `.expected.ts` to confirm correctness.

### Step 6: Add roundtrip fixtures

Add at least one file to `tests/fixtures/roundtrip/` that uses your extension. The roundtrip test verifies that the output parses as valid TypeScript.

### Step 7: Verify everything

```bash
cargo test                   # all golden + roundtrip tests green
cargo clippy                 # no warnings
```

All existing tests must continue to pass unchanged. The test harness enforces this — any mismatch in any fixture is a failure.

### Step 8: Update documentation

- Add your extension to the syntax table in `AGENTS.md` (section 2)
- Update the cross-reference index (section 6) if typesugar has a reference implementation
- Add examples to `docs/syntax-reference.md`

### Quick reference: what was added for pipeline

| Step | File(s) | What changed |
|------|---------|-------------|
| Feature flag | `sc_ast/src/lib.rs` | `pipeline: bool` on `ScSyntax` |
| Token | `sc_ast/src/lib.rs`, `sc_lexer/src/lib.rs` | `ScBinaryOp::Pipeline`, merge rule for `\|` + `>` |
| AST node | `sc_ast/src/lib.rs` | `ScBinExpr` struct with `ScBinaryOp` |
| Preprocessing | `sc_parser/src/preprocess/operator_pass.rs` | `Op::Pipeline`, detection + rewriting to `__binop__()` |
| Desugar | `sc_desugar/src/pipeline.rs` | `desugar_pipeline()`, `make_binop_call()` |
| Tests | `tests/fixtures/pipeline/` | `basic.input.ts`, `chained.input.ts` + expected files |
| Config | `sc_ast/src/lib.rs` | `ScSyntax::default()` enables pipeline |

### Test requirements checklist

Every new extension must include:

- [ ] At least 5 basic golden tests (happy path variations)
- [ ] Precedence and associativity tests (for operators)
- [ ] Type-context exclusion tests (operator not rewritten in type positions)
- [ ] String/comment exclusion tests (no rewriting inside strings or comments)
- [ ] Interaction tests with every existing extension
- [ ] Adversarial edge cases
- [ ] Roundtrip validation (output is valid TypeScript)

## Rebasing on Upstream SWC

When a new SWC release comes out, see [rebasing-upstream.md](rebasing-upstream.md) for the full process.

## Code Style

- **Clippy clean** — `cargo clippy` must pass with zero warnings
- **No `unwrap()` in library code** — use `Result`/`Option` with `?` propagation. `unwrap()` is fine in tests and the test harness.
- **Edition 2021** — all crates use the workspace edition
- **Comments explain "why", not "what"** — don't narrate obvious code. If the code itself makes the intent clear, skip the comment.

```rust
// Good: explains a non-obvious constraint
// Adjacency check prevents merging tokens separated by whitespace,
// which would misinterpret `x | y > z` as a pipeline.
if tokens[i].span.hi == tokens[i + 1].span.lo { ... }

// Bad: narrates the obvious
// Check if the spans are adjacent
if tokens[i].span.hi == tokens[i + 1].span.lo { ... }
```
