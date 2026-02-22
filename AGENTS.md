# AGENTS.md — Sugarcube

> Primary development guide for AI agents working on sugarcube.
> Last updated: 2026-02-22.

## 1. Project Purpose

Sugarcube is the **parser frontend** for the [typesugar](../typesugar/) macro system. It replaces `@typesugar/preprocessor` (`~/src/typesugar/packages/preprocessor/`), which uses fragile text-level heuristics (regex-based type-context detection, brace-counting for HKT scope).

Sugarcube outputs **valid TypeScript** — no custom syntax remains in the output. The downstream typesugar transformer expands macros, typeclasses, and derives.

### Pipeline

```
Source (.ts/.tsx) with custom syntax
        │
        ▼
  ┌──────────────┐
  │ sugarcube/sc  │  ◄── THIS REPO: parse + desugar
  └──────────────┘
        │
        ▼
  Valid TypeScript (F<_> → $<F,A>, |> → __binop__, :: → __binop__)
        │
        ├──────────────────────────────────────────────────────┐
        ▼                                                      ▼
  ┌─────────────────────────────┐                        ┌──────────┐
  │ typesugar transformer       │                        │   tsc    │
  │ (ts-patch / unplugin)       │                        │ (types)  │
  │ macro-transformer.ts        │                        └──────────┘
  └─────────────────────────────┘
        │
        ▼
  JavaScript / .d.ts output
```

### What Gets Desugared Here vs. Downstream

Only **three syntax forms** need parser-level support. Everything else (decorators, labeled blocks, expression macros) is already valid TypeScript and handled by typesugar's transformer.

## 2. Syntax Extensions Specification

### Pipeline Operator (`|>`)

- **Precedence**: 1 (lowest of all operators)
- **Associativity**: Left
- **Desugaring**: `a |> f |> g` → `__binop__(__binop__(a, "|>", f), "|>", g)`
- **Type-context exclusion**: Must NOT rewrite inside type annotations, interfaces, or generic parameters
- **Reference implementation**: `~/src/typesugar/packages/preprocessor/src/extensions/pipeline.ts`
- **Runtime resolution**: `__binop__` is resolved by typesugar's transformer via `tryRewriteOperator` in `macro-transformer.ts`

### Cons Operator (`::`)

- **Precedence**: 5 (higher than pipeline, lower than standard JS arithmetic)
- **Associativity**: Right
- **Desugaring**: `1 :: 2 :: []` → `__binop__(1, "::", __binop__(2, "::", []))`
- **Type-context exclusion**: Same as pipeline — no rewriting in type positions
- **Reference implementation**: `~/src/typesugar/packages/preprocessor/src/extensions/cons.ts`

### HKT Type Parameters (`F<_>`)

- **In declarations**: `F<_>` becomes just `F` (strip `<_>`)
- **In type references within scope**: `F<A>` becomes `$<F, A>` using the HKT encoding from `~/src/typesugar/packages/type-system/src/hkt.ts` — `type $<F, A> = (F & { readonly _: A })["_"]`
- **Scope**: Bounded by the containing declaration (interface, type alias, function) where `F<_>` appears
- **Shadowing**: Inner `F<_>` declarations shadow outer ones
- **Reference implementation**: `~/src/typesugar/packages/preprocessor/src/extensions/hkt.ts`

#### HKT Example

```typescript
// Input
interface Functor<F<_>> {
  map: <A, B>(fa: F<A>, f: (a: A) => B) => F<B>;
}

// Output
interface Functor<F> {
  map: <A, B>(fa: $<F, A>, f: (a: A) => B) => $<F, B>;
}
```

### Formal Desugaring Rules (AST Rewrites)

```
Pipeline:   BinExpr(left, Pipeline, right)   → Call(__binop__, [left, "|>", right])
Cons:       BinExpr(left, Cons, right)       → Call(__binop__, [left, "::", right])
HKT decl:  TsTypeParam { name: F, is_hkt }  → TsTypeParam { name: F }
HKT usage:  TsTypeRef { name: F, params: [A] } → TsTypeRef { name: $, params: [F, A] }
            (only when F is in HKT scope)
```

## 3. Architecture

### Crate Map

```
sc_lexer ──▶ sc_parser ──▶ sc_ast
                              │
                              ▼
                         sc_desugar ──▶ swc_ecma_ast (standard)
                              │
                              ▼
                         swc_ecma_codegen ──▶ output.ts
```

### Workspace Crates

| Crate | Type | Purpose |
|---|---|---|
| `sc_ast` | Fork additions | Extended AST types: `ScBinaryOp` (Pipeline, Cons), `ScBinExpr`, `HktTypeParam`, `ScSyntax` feature flags. Re-exports `swc_ecma_ast::*`. |
| `sc_lexer` | Fork additions | Wraps SWC's token stream, merges adjacent tokens into sugarcube operators (`\|` + `>` → Pipeline, `:` + `:` → Cons). |
| `sc_parser` | Fork additions | Text-level preprocessor that rewrites custom syntax before feeding to the standard SWC parser. Contains `preprocess/hkt_pass.rs` (HKT rewriting) and `preprocess/operator_pass.rs` (`\|>` and `::` to `__binop__` calls). |
| `sc_desugar` | New | AST-to-AST transform: walks custom AST → produces standard `swc_ecma_ast` nodes. Modules: `pipeline.rs`, `cons.rs`, `hkt.rs`, `desugar.rs`. |
| `sc_cli` | New | Binary crate producing the `sc` command. Wires lexer → parser → desugar → codegen. |
| `sc_test` | New | Test harness crate. Drives `tests/harness.rs` for golden-file and roundtrip tests. |

### Upstream SWC Dependencies (from crates.io)

| Crate | Version | Role |
|---|---|---|
| `swc_common` | 18 | Spans, source maps, error handling |
| `swc_ecma_ast` | 20 | Standard AST types (used after desugaring) |
| `swc_ecma_parser` | 33 | Standard TypeScript parser |
| `swc_ecma_visit` | 20 | Visitor traits for AST traversal |
| `swc_ecma_codegen` | 23 | Code emitter (works with standard AST post-desugar) |

### Current Architecture: Text-Level Preprocessing

The current implementation uses a **text-level preprocessor** (similar to typesugar's approach) rather than a deep AST fork. The pipeline is:

1. **`sc_parser::preprocess`** — rewrites source text before SWC parsing:
   - `hkt_pass::rewrite_hkt` — finds `F<_>` declarations, strips `<_>`, rewrites `F<A>` → `$<F, A>` within scope
   - `operator_pass::rewrite_operators` — finds `|>` and `::` in expression context, rewrites to `__binop__()` calls
2. **Standard SWC parser** — parses the now-valid TypeScript
3. **`sc_desugar::desugar_module`** — currently a passthrough (the preprocessing already desugared everything at text level)
4. **`swc_ecma_codegen`** — emits the AST back to TypeScript source

The `sc_ast` types (`ScBinExpr`, `ScBinaryOp`, `HktTypeParam`) and `sc_lexer` token merging exist for a future migration to AST-level processing but are not yet used in the main pipeline.

### CLI Commands

| Command | Description | Key Flags |
|---|---|---|
| `sc preprocess <file>` | Parse + desugar + emit standard TS | `-o <output>`, `--source-map`, `--tsx` |
| `sc check <file>` | Parse only, report errors | `--tsx` |
| `sc parse <file>` | Parse and dump AST | `--ast` (JSON), `--tsx` |

## 4. Source Layout

```
sugarcube/
├── Cargo.toml              # Workspace root, pins upstream SWC versions
├── .envrc                  # direnv: dpovey GitHub account, HTTPS remote
├── crates/
│   ├── sc_ast/
│   │   └── src/lib.rs      # ScBinaryOp, ScBinExpr, HktTypeParam, ScSyntax
│   ├── sc_lexer/
│   │   └── src/lib.rs      # ScToken, ScTokenAndSpan, merge_sc_tokens()
│   ├── sc_parser/
│   │   └── src/
│   │       ├── lib.rs      # Module root, re-exports parse_sugarcube
│   │       ├── parse.rs    # parse_sugarcube() — preprocessor + SWC parser
│   │       └── preprocess/
│   │           ├── mod.rs          # preprocess() entry point
│   │           ├── hkt_pass.rs     # rewrite_hkt() — F<_> handling
│   │           └── operator_pass.rs # rewrite_operators() — |> and :: handling
│   ├── sc_desugar/
│   │   └── src/
│   │       ├── lib.rs      # Re-exports desugar_module
│   │       ├── desugar.rs  # desugar_module() — currently passthrough
│   │       ├── pipeline.rs # desugar_pipeline(), make_binop_call()
│   │       ├── cons.rs     # desugar_cons()
│   │       └── hkt.rs      # HktRewriter (VisitMut for $<F, A> rewriting)
│   ├── sc_cli/
│   │   └── src/main.rs     # sc binary: preprocess, check, parse commands
│   └── sc_test/
│       └── src/lib.rs      # (empty — test logic is in tests/harness.rs)
├── tests/
│   ├── harness.rs          # Golden-file and roundtrip test runner
│   └── fixtures/
│       ├── pipeline/       # Pipeline operator tests
│       ├── cons/           # Cons operator tests
│       ├── hkt/            # HKT type parameter tests
│       ├── mixed/          # Cross-extension interaction tests
│       ├── edge-cases/     # Adversarial inputs (strings, comments, etc.)
│       └── roundtrip/      # Output-is-valid-TS validation tests
└── docs/                   # (planned: architecture.md, syntax-reference.md, etc.)
```

## 5. Development Conventions

### Rust Style

- **Clippy defaults, zero warnings** — `cargo clippy` must pass clean
- **No `unwrap()` in library code** — use `Result`/`Option` with `?` propagation. `unwrap()` is acceptable in tests and the test harness.
- **Comments explain "why", not "what"** — don't narrate obvious code
- **Edition 2021** — all crates use the workspace edition

### Building and Running

```bash
cargo build                       # build all crates
cargo build -p sc_cli             # build just the CLI
cargo install --path crates/sc_cli  # install sc locally

sc preprocess input.ts            # run the pipeline
sc preprocess input.ts -o out.ts  # write to file
sc check input.ts                 # syntax check only
sc parse input.ts --ast           # dump AST as JSON
```

### Version Tracking

Upstream SWC versions are pinned in the workspace `Cargo.toml` under `[workspace.dependencies]`. When updating SWC, bump all versions together and follow `docs/rebasing-upstream.md` (planned).

Current pinned versions: `swc_common` 18, `swc_ecma_ast` 20, `swc_ecma_parser` 33, `swc_ecma_visit` 20, `swc_ecma_codegen` 23.

### GitHub

- **Account**: `dpovey` (personal) — configured via `.envrc`
- **Org/Repo**: [typesugar/sugarcube](https://github.com/typesugar/sugarcube)
- **Remote**: HTTPS (SSH key is linked to work account, so `.envrc` rewrites URLs to use token auth)

### Other Dependencies

| Crate | Version | Used For |
|---|---|---|
| `clap` | 4 | CLI argument parsing (derive mode) |
| `serde` | 1 | Serialization for AST types and config |
| `serde_json` | 1 | JSON AST dump in `sc parse --ast` |
| `anyhow` | 1 | Error handling in CLI and parser |

## 6. Cross-Reference Index to Typesugar

Sugarcube reimplements what lives in typesugar's preprocessor package. Use these references when verifying behavior or porting test cases.

| Sugarcube Concern | Typesugar Reference |
|---|---|
| Pipeline operator spec | `~/src/typesugar/packages/preprocessor/src/extensions/pipeline.ts` |
| Cons operator spec | `~/src/typesugar/packages/preprocessor/src/extensions/cons.ts` |
| HKT rewriting spec | `~/src/typesugar/packages/preprocessor/src/extensions/hkt.ts` |
| HKT type encoding (`$<F, A>`) | `~/src/typesugar/packages/type-system/src/hkt.ts` |
| `__binop__` resolution | `~/src/typesugar/src/transforms/macro-transformer.ts` (`tryRewriteOperator`) |
| Type context heuristics | `~/src/typesugar/docs/architecture.md` (section: "Type Context Detection") |
| Full pipeline architecture | `~/src/typesugar/docs/architecture.md` |
| Macro system overview | `~/src/typesugar/AGENTS.md` |
| Preprocessor entry point | `~/src/typesugar/packages/preprocessor/src/preprocess.ts` |
| Scanner/tokenizer | `~/src/typesugar/packages/preprocessor/src/scanner.ts` |

### Test Case Sources for Porting (~100+ cases)

| Test File | Covers |
|---|---|
| `~/src/typesugar/packages/preprocessor/tests/scanner.test.ts` | Token merging, string/comment exclusion |
| `~/src/typesugar/packages/preprocessor/tests/pipeline.test.ts` | Pipeline operator edge cases |
| `~/src/typesugar/packages/preprocessor/tests/cons.test.ts` | Cons operator edge cases |
| `~/src/typesugar/packages/preprocessor/tests/hkt.test.ts` | HKT declaration and usage rewriting |
| `~/src/typesugar/packages/preprocessor/tests/mixed.test.ts` | Cross-extension interactions |
| `~/src/typesugar/tests/red-team-preprocessor.test.ts` | Adversarial/edge-case inputs |

## 7. Testing Conventions

### Test Structure

```
tests/
  fixtures/
    pipeline/
      basic.input.ts          # sugarcube syntax input
      basic.expected.ts        # expected standard TS output
      chained.input.ts
      chained.expected.ts
    cons/
      ...
    hkt/
      ...
    mixed/                     # cross-extension interaction tests
      ...
    edge-cases/                # strings, comments, adversarial inputs
      ...
    roundtrip/                 # files that must survive parse → desugar → reparse
      ...
  harness.rs                   # Test runner (driven by sc_test crate)
```

### Golden Test Runner (`tests/harness.rs`)

The `golden_file_tests` test function:

1. Discovers all `.input.ts` files under `tests/fixtures/`
2. Runs the full pipeline: `parse_sugarcube()` → `desugar_module()` → `swc_ecma_codegen`
3. Compares output against the matching `.expected.ts` file (text diff)
4. Reports all failures at the end with expected vs. actual output

The `roundtrip_tests` test function:

1. Discovers `.input.ts` files under `tests/fixtures/roundtrip/`
2. Runs the full pipeline
3. Re-parses the output with the standard SWC parser (all sugarcube extensions disabled) to verify the output is legal TypeScript

### Running Tests

```bash
cargo test                       # all tests (golden + roundtrip + unit)
cargo test -p sc_test            # just golden/roundtrip
cargo test -p sc_parser          # unit tests in parser
cargo test -p sc_lexer           # unit tests in lexer
```

### Updating Golden Files

```bash
SC_UPDATE_FIXTURES=1 cargo test
```

This overwrites `.expected.ts` files with actual output (like Jest's `--updateSnapshot`). Review the diff before committing.

### Requirements for New Syntax Extensions

Every new extension must include:

- At least 5 basic golden tests (happy path variations)
- Precedence and associativity tests (for operators)
- Type-context exclusion tests (operator not rewritten in type positions)
- String/comment exclusion tests (no rewriting inside strings or comments)
- Interaction tests with every existing extension
- Adversarial edge cases
- Roundtrip validation (output is valid TypeScript)

**Regression rule**: All existing golden tests must continue passing unchanged. The test harness enforces this automatically — any mismatch in any fixture is a test failure.

## 8. Adding New Syntax Extensions (Checklist)

When adding a new syntax extension, follow this checklist. Use `|>` (pipeline) as the reference implementation.

1. **Add feature flag** — add a `bool` field to `ScSyntax` in `crates/sc_ast/src/lib.rs`
2. **Add token** (if new lexeme needed) — add a variant to `ScToken`/`ScBinaryOp` in `sc_ast`, add the merge rule in `sc_lexer/src/lib.rs`
3. **Add AST node or variant** — add a struct or enum variant in `sc_ast/src/lib.rs`
4. **Add preprocessing rule** — add a new pass file in `sc_parser/src/preprocess/` and wire it into `preprocess::preprocess()`
5. **Add desugar transform** — add a module in `sc_desugar/src/` and call it from `desugar_module()`
6. **Add golden-file tests** — create `tests/fixtures/<extension>/` with at least 5 `.input.ts`/`.expected.ts` pairs
7. **Add roundtrip test fixtures** — add files to `tests/fixtures/roundtrip/` that exercise the new syntax
8. **Verify all existing tests pass** — `cargo test` must be green with no regressions
9. **Update the syntax table** in this file (section 2)
10. **Update the cross-reference index** (section 6) if typesugar has a reference implementation

### Walkthrough: How Pipeline Was Added

| Step | File(s) | What Changed |
|---|---|---|
| Feature flag | `sc_ast/src/lib.rs` | Added `pipeline: bool` to `ScSyntax` |
| Token | `sc_ast/src/lib.rs`, `sc_lexer/src/lib.rs` | `ScBinaryOp::Pipeline`, merge rule for `\|` + `>` |
| AST node | `sc_ast/src/lib.rs` | `ScBinExpr` struct with `ScBinaryOp` |
| Preprocessing | `sc_parser/src/preprocess/operator_pass.rs` | `Op::Pipeline`, detection + rewriting to `__binop__()` |
| Desugar | `sc_desugar/src/pipeline.rs` | `desugar_pipeline()` → `make_binop_call()` |
| Tests | `tests/fixtures/pipeline/` | `basic.input.ts`, `chained.input.ts` + expected files |
| Config | `sc_ast/src/lib.rs` | `ScSyntax::default()` enables pipeline |

## 9. Future Syntax Roadmap

Planned additions — agents should be aware of the direction but should NOT implement these without explicit instructions.

| Extension | Priority | TC39 Stage | Desugaring Strategy |
|---|---|---|---|
| Do expressions | High | Stage 1 | `do { stmts; expr }` in expression position → IIFE wrapping |
| Method annotations | High | N/A (TS recovery) | Re-enable richer decorator syntax on class methods |
| Pattern matching | Medium | Stage 1 | `match (x) { when ... }` → if/else or switch chains |
| Record/Tuple literals | Low | Stage 2 | `#{ }` / `#[ ]` → `Object.freeze()` / tuple construction |
| Custom operators (general) | Medium | N/A | User-defined binary operators via config file |

### Design Constraints for Future Extensions

- Every extension must desugar to **valid TypeScript** — sugarcube never emits custom syntax
- The `__binop__` pattern is the standard approach for new binary operators
- HKT-style type rewrites should use the `$<F, A>` encoding from typesugar's type system
- Feature flags must default to `true` in `ScSyntax::default()` (all extensions on by default)
- New extensions must not break existing ones — the test harness enforces this

## 10. Known Limitations and Open Items

- **Text-level preprocessing**: The current implementation rewrites source text before SWC parses it. This works but has the same class of edge-case risks as typesugar's preprocessor. A future iteration may move to true AST-level parsing (the `sc_ast` types and `sc_lexer` token merging are scaffolding for this).
- **`desugar_module` is a passthrough**: Since preprocessing handles desugaring at text level, the AST-level desugar module currently returns the module unchanged.
- **Source maps**: Source maps flow through SWC's standard span-based system but don't account for text-level preprocessing offsets. Positions in error messages may be slightly off for desugared code.
- **`::` ambiguity**: TypeScript doesn't currently use `::` but future TS versions might. Monitor TC39/TS proposals.
- **Error recovery**: SWC's parser has error recovery, but sugarcube's preprocessing doesn't. A malformed `|>` or `::` will produce confusing SWC parse errors on the preprocessed text rather than a clear sugarcube-level error.
- **Test port in progress**: ~100+ test cases from typesugar's preprocessor tests still need to be ported to golden-file format.
