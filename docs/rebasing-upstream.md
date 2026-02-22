# Rebasing on Upstream SWC

Guide for pulling new SWC releases into sugarcube's pinned dependencies.

Sugarcube doesn't fork SWC's source code — it depends on SWC crates from crates.io and adds its own crates on top. "Rebasing" here means bumping the pinned SWC versions and adapting to any breaking API changes.

## 1. Identify the Upstream Version

Check the currently pinned versions in `Cargo.toml`:

```bash
grep 'swc_' Cargo.toml
```

Current pins (as of this writing):

| Crate | Pinned Version |
|-------|---------------|
| `swc_common` | 18 |
| `swc_ecma_ast` | 20 |
| `swc_ecma_parser` | 33 |
| `swc_ecma_visit` | 20 |
| `swc_ecma_codegen` | 23 |

Check the latest SWC releases on [crates.io](https://crates.io/crates/swc_ecma_parser) or the [SWC GitHub releases](https://github.com/nicedayfor/swc/releases). SWC crates are versioned independently — look for a set of compatible versions released around the same time.

## 2. Create a Rebase Branch

```bash
git checkout -b rebase/swc-vX.Y.Z
```

Use the `swc_ecma_parser` version as the branch name reference since it's the most prominent dependency.

## 3. Review Upstream Changes

Before bumping, review what changed in SWC between the old and new versions. Focus on crates sugarcube directly uses:

### `swc_ecma_ast` (AST types)

This is the most likely source of breaking changes. Check for:

- **Renamed or restructured AST node types** — sugarcube's `sc_ast` re-exports `swc_ecma_ast::*` and the desugar module constructs standard AST nodes
- **Changed fields on `Expr`, `TsType`, `TsTypeParam`**, etc. — the test harness and desugar crate build these
- **Serde changes** — `sc parse --ast` serializes the AST to JSON

```bash
# Compare changelogs or diff the upstream source
cargo doc -p swc_ecma_ast --open
```

### `swc_ecma_parser` (parser)

- **Changed `Syntax` or parser configuration** — `parse_sugarcube()` creates parser config
- **Token changes** — `sc_lexer` pattern-matches on `Token` variants and `BinOpToken`
- **`TokenAndSpan` field changes** — the lexer reads `token`, `span`, and `had_line_break`

### `swc_ecma_codegen` (code emitter)

- **Changed `Emitter` or `JsWriter` API** — the CLI and test harness construct these
- **Config changes** — `swc_ecma_codegen::Config` and `EsVersion`

### `swc_ecma_visit` (visitor traits)

- **Changed `VisitMut` or `Visit` trait signatures** — `sc_desugar` uses `VisitMut` for AST traversal

### `swc_common` (spans, source maps)

- **Changed `Span`, `BytePos`, source map API** — used throughout

## 4. Bump Versions in `Cargo.toml`

Update all SWC versions together in `[workspace.dependencies]`:

```toml
[workspace.dependencies]
swc_common = { version = "NEW", features = ["sourcemap"] }
swc_ecma_ast = { version = "NEW", features = ["serde-impl"] }
swc_ecma_codegen = "NEW"
swc_ecma_parser = "NEW"
swc_ecma_visit = "NEW"
```

Run `cargo update` to resolve the dependency graph, then attempt a build:

```bash
cargo update
cargo build 2>&1 | head -50
```

## 5. Fix Compilation Errors

Work through compiler errors crate by crate, starting from the lowest level:

### `sc_ast`

Since it re-exports `swc_ecma_ast::*`, most changes here are transparent. Check that `ScBinExpr` still wraps `Box<Expr>` correctly if `Expr` changed.

### `sc_lexer`

Update `merge_sc_tokens()` if `Token`, `BinOpToken`, or `TokenAndSpan` fields changed:

```rust
// If Token variants were renamed:
matches!(tokens[i].token, Token::BinOp(BinOpToken::BitOr))
//                                     ^^^ check this still exists
```

### `sc_parser`

The text-level preprocessor (`operator_pass.rs`, `hkt_pass.rs`) operates on strings, so it's mostly unaffected by AST changes. The `parse_sugarcube()` function creates parser config — update it if the `swc_ecma_parser` API changed.

### `sc_desugar`

The desugar module constructs standard AST nodes. If field names or constructors changed on `Expr`, `CallExpr`, `TsTypeRef`, etc., update the code that builds them.

### `sc_cli` and `sc_test` (test harness)

Update `Emitter`, `JsWriter`, and `Config` usage if the codegen API changed.

## 6. Verify

Run the full test suite and clippy:

```bash
cargo test
cargo clippy
```

All golden-file tests must pass. If the codegen output changed (e.g., whitespace or formatting differences), review the diffs carefully:

- **Semantic equivalence**: if the output is different but functionally identical (whitespace, semicolons), update the golden files: `SC_UPDATE_FIXTURES=1 cargo test`
- **Behavioral changes**: if the output is semantically different, investigate whether it's an SWC bug fix (good) or a regression (report upstream)

Also verify roundtrip tests — the output must still parse as valid TypeScript.

## 7. Document the Update

Update these files with the new version numbers:

| File | What to update |
|------|---------------|
| `Cargo.toml` | `[workspace.dependencies]` version pins |
| `AGENTS.md` | Section 3 (upstream SWC dependencies table) and section 5 (version tracking) |

Commit with a message like:

```
Bump SWC dependencies to swc_ecma_parser 34, swc_ecma_ast 21, etc.
```

## Troubleshooting

### Version incompatibility between SWC crates

SWC crates must be from compatible versions. If `cargo build` shows duplicate type errors (e.g., "expected `swc_common::Span` found `swc_common::Span`"), you have version mismatches. Ensure all SWC crates are pinned to versions from the same SWC release cycle.

### Yanked crate versions

Occasionally SWC yanks a crate version after release. If `cargo update` fails, check crates.io for the latest non-yanked version.

### Feature flag changes

SWC sometimes moves functionality behind feature flags. If a type or function disappeared, check if it moved behind a feature in the new version. Common features: `serde-impl` on `swc_ecma_ast`, `sourcemap` on `swc_common`.
