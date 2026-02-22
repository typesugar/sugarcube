# Sugarcube Architecture

Internal architecture reference for contributors. For syntax details, see [syntax-reference.md](./syntax-reference.md).

## Crate Map

```
sc_ast             Pure data types (ScBinaryOp, ScBinExpr, HktTypeParam, ScSyntax)
  │                Re-exports swc_ecma_ast::*
  │
  ├─► sc_lexer     Token merging (| + > → Pipeline, : + : → Cons)
  │     │          Wraps SWC token stream
  │     │
  │     └─► sc_parser     Text-level preprocessor + SWC parser
  │           │            preprocess/ → hkt_pass, operator_pass
  │           │
  │           └─► sc_desugar     AST-to-AST transform (currently passthrough)
  │                 │            pipeline.rs, cons.rs, hkt.rs, desugar.rs
  │                 │
  │                 └─► sc_cli     Binary crate ("sc" command)
  │                       │        Wires: preprocess → parse → desugar → codegen
  │                       │
  └─────────────────────► sc_test  Golden-file + roundtrip test harness
```

External dependencies flow downward from the workspace `Cargo.toml`:

| Upstream Crate | Version | Role |
|---|---|---|
| `swc_common` | 18 | Spans, source maps, error handling |
| `swc_ecma_ast` | 20 | Standard TypeScript AST (re-exported by `sc_ast`) |
| `swc_ecma_parser` | 33 | Standard TypeScript parser |
| `swc_ecma_visit` | 20 | Visitor traits for AST traversal |
| `swc_ecma_codegen` | 23 | Code emitter (standard AST → TypeScript text) |

## Crate Details

### `sc_ast`

Purpose: Extended AST types that augment SWC's standard AST.

| Type | Role |
|---|---|
| `ScBinaryOp` | Enum: `Pipeline`, `Cons` — custom binary operators |
| `ScBinExpr` | Binary expression node with `ScBinaryOp`, boxed `Expr` left/right, and `Span` |
| `HktTypeParam` | Marker for type parameters declared with `F<_>` syntax |
| `ScSyntax` | Feature flags (`pipeline`, `cons`, `hkt`) controlling which extensions are active |

Everything from `swc_ecma_ast` is re-exported, so downstream crates can import from `sc_ast` as a superset.

**Fork status**: No SWC code forked. This crate only adds new types alongside the re-export.

### `sc_lexer`

Purpose: Post-process SWC's token stream to merge adjacent tokens into sugarcube operators.

| Type/Function | Role |
|---|---|
| `ScToken` | Enum: `Standard(Token)` or `ScOperator(ScBinaryOp)` |
| `ScTokenAndSpan` | Token with span and `had_line_break` flag |
| `merge_sc_tokens()` | Scans `&[TokenAndSpan]`, merges `|`+`>` → Pipeline and `:`+`:` → Cons |

Merge rules require tokens to be byte-adjacent (`span.hi == next.span.lo`) — whitespace between `|` and `>` prevents merging.

**Fork status**: No forked code. Consumes SWC's `TokenAndSpan` output.

**Note**: The lexer exists for a future AST-level approach. The current pipeline uses text-level preprocessing and does not call `merge_sc_tokens()` in production.

### `sc_parser`

Purpose: Parse TypeScript source with sugarcube extensions. Current implementation is a text-level preprocessor that rewrites source before SWC parsing.

| Module | Role |
|---|---|
| `parse.rs` | `parse_sugarcube()` — entry point: preprocess → SWC parse → return `ParseResult` |
| `preprocess.rs` | `preprocess()` — orchestrates HKT pass then operator pass |
| `preprocess/hkt_pass.rs` | `rewrite_hkt()` — finds `F<_>` declarations, strips `<_>`, rewrites `F<A>` → `$<F, A>` |
| `preprocess/operator_pass.rs` | `rewrite_operators()` — finds `|>` and `::` in expression context, rewrites to `__binop__()` calls |

The `ParseResult` struct contains:
- `module: swc_ecma_ast::Module` — the parsed AST
- `comments: SingleThreadedComments` — preserved comments
- `source_map: Lrc<SourceMap>` — for error reporting and codegen
- `preprocessed_source: String` — the intermediate text after rewriting

### `sc_desugar`

Purpose: AST-to-AST transform from sugarcube nodes to standard TypeScript nodes.

| Module | Role |
|---|---|
| `desugar.rs` | `desugar_module()` — entry point (currently a passthrough) |
| `pipeline.rs` | `desugar_pipeline()` and `make_binop_call()` — build `__binop__` call expressions |
| `cons.rs` | `desugar_cons()` — delegates to `make_binop_call()` with `"::"` |
| `hkt.rs` | `HktRewriter` — `VisitMut` impl that rewrites `F<A>` → `$<F, A>` in type references |

**Current state**: `desugar_module()` returns the module unchanged because all desugaring happens at the text level in `sc_parser::preprocess`. The individual transform functions (`desugar_pipeline`, `desugar_cons`, `HktRewriter`) are implemented and ready for use when the parser moves to AST-level processing.

### `sc_cli`

Purpose: The `sc` binary.

| Command | Description |
|---|---|
| `sc preprocess <file>` | Full pipeline: parse → desugar → emit standard TS |
| `sc check <file>` | Parse only, report errors |
| `sc parse <file> [--ast]` | Parse and dump AST (debug format or JSON) |

All commands accept `--tsx` for TSX files. `preprocess` accepts `-o <file>` and `--source-map`.

### `sc_test`

Purpose: Test harness crate. The actual test logic lives in `tests/harness.rs` and is compiled as an integration test of `sc_test`.

## Parser Pipeline

### Current Implementation: Text-Level Preprocessing

```
Source text (.ts/.tsx)
  │
  │ 1. sc_lexer::merge_sc_tokens()     [NOT USED — scaffolding for future]
  │
  ▼
  │ 2. sc_parser::preprocess::preprocess()
  │    ├── hkt_pass::rewrite_hkt()           Rewrites F<_> declarations and usages
  │    └── operator_pass::rewrite_operators() Rewrites |> and :: to __binop__()
  │
  ▼
Valid TypeScript text (no custom syntax)
  │
  │ 3. swc_ecma_parser::parse_file_as_module()
  │    Standard SWC parser, TS/TSX mode, decorators enabled
  │
  ▼
swc_ecma_ast::Module
  │
  │ 4. sc_desugar::desugar_module()     [PASSTHROUGH — desugaring already done at text level]
  │
  ▼
swc_ecma_ast::Module (unchanged)
  │
  │ 5. swc_ecma_codegen::Emitter
  │    JsWriter with source map support
  │
  ▼
Output TypeScript text + optional source map
```

### Step 2: Preprocessing Details

**HKT pass** (`hkt_pass::rewrite_hkt`):

1. Scan for uppercase identifiers followed by `<_>` (or `<_, _>` for multi-arity)
2. For each declaration, compute the enclosing scope (backward to `}` or `;`, forward to matching `}`)
3. Find all usages of the declared name with type arguments within scope
4. Apply replacements in reverse order (to preserve byte offsets):
   - Declarations: strip the `<_>` suffix
   - Usages: `F<A>` → `$<F, A>`

**Operator pass** (`operator_pass::rewrite_operators`):

1. Find all `|>` and `::` occurrences not inside strings, comments, or type contexts
2. Type context detection tracks: `type` aliases, `interface` blocks, type annotation depth (after `:`), angle bracket depth
3. Select the next operator to process: highest precedence first; for ties, leftmost (left-assoc) or rightmost (right-assoc)
4. Find left and right operand boundaries by scanning for expression delimiters
5. Replace `left |> right` with `__binop__(left, "|>", right)`
6. Repeat until no operators remain (iterative, max 1000 iterations)

Processing order matters: HKT runs first because it operates on type-level syntax that shouldn't interact with operator rewriting.

### Step 3: SWC Parser Configuration

```rust
Syntax::Typescript(TsSyntax {
    tsx: /* from filename or --tsx flag */,
    decorators: true,
    ..Default::default()
})
```

Target: `EsVersion::latest()`. Comments are captured via `SingleThreadedComments`.

## AST Extension Points

### `ScBinaryOp`

```rust
pub enum ScBinaryOp {
    Pipeline,  // |>  — precedence 1, left-associative
    Cons,      // ::  — precedence 5, right-associative
}
```

These live alongside SWC's `BinaryOp` (which covers standard JS operators). They are not variants of `BinaryOp` — they're a separate enum used only in `ScBinExpr`.

### `ScBinExpr`

```rust
pub struct ScBinExpr {
    pub span: Span,
    pub op: ScBinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}
```

Contains standard `swc_ecma_ast::Expr` children. The desugar functions consume `ScBinExpr` and produce standard `Expr::Call` nodes.

### `HktTypeParam`

```rust
pub struct HktTypeParam {
    pub span: Span,
    pub name: String,
}
```

Marker type. In the future AST-level approach, the parser would produce `HktTypeParam` nodes in type parameter lists, and the desugar pass would use `HktRewriter` to walk the AST and rewrite type references.

### `ScSyntax`

```rust
pub struct ScSyntax {
    pub pipeline: bool,  // default: true
    pub cons: bool,      // default: true
    pub hkt: bool,       // default: true
}
```

Controls which extensions are active. Checked by:
- `preprocess()` — gates whether HKT and operator passes run
- `merge_sc_tokens()` — gates whether token merging is attempted
- Test harness — uses all-false `ScSyntax` when verifying output is valid standard TypeScript

## Desugaring Rules

### Pipeline (`|>`)

```
BinExpr(left, Pipeline, right)
  → CallExpr(
      callee: Ident("__binop__"),
      args: [left, Str("|>"), right]
    )
```

Left-associative chaining:

```
a |> f |> g
  → __binop__(__binop__(a, "|>", f), "|>", g)
```

### Cons (`::`)

```
BinExpr(left, Cons, right)
  → CallExpr(
      callee: Ident("__binop__"),
      args: [left, Str("::"), right]
    )
```

Right-associative chaining:

```
1 :: 2 :: []
  → __binop__(1, "::", __binop__(2, "::", []))
```

### HKT Type Parameters

**Declaration rewrite** — strip `<_>`:

```
TsTypeParam { name: F, constraint: <_> }
  → TsTypeParam { name: F }
```

**Usage rewrite** — within the declaring scope:

```
TsTypeRef { name: F, params: [A] }
  → TsTypeRef { name: $, params: [TsTypeRef(F), A] }
```

This uses the HKT encoding from typesugar's type system: `type $<F, A> = (F & { readonly _: A })["_"]`.

### Mixed Precedence

When pipeline and cons appear together, cons binds tighter:

```
a :: b |> f
  → __binop__(__binop__(a, "::", b), "|>", f)
```

## Feature Flag System

`ScSyntax` is the single source of truth for which extensions are enabled. All crates accept it as a parameter.

| Flag | Default | Controls |
|---|---|---|
| `pipeline` | `true` | `|>` operator rewriting and token merging |
| `cons` | `true` | `::` operator rewriting and token merging |
| `hkt` | `true` | `F<_>` declaration stripping and `F<A>` → `$<F, A>` rewriting |

Adding a new extension means adding a `bool` field to `ScSyntax`, defaulting to `true`, and checking it in the relevant preprocessing pass and token merging logic.

The test harness uses `ScSyntax::default()` for golden-file tests (all extensions on) and all-false `ScSyntax` for roundtrip validation (confirms output is standard TS).

## Source Map Strategy

Source maps flow through SWC's standard span-based system:

1. `SourceMap` is created in `parse_sugarcube()` and shared across parsing and codegen
2. SWC's parser attaches `Span` values to every AST node, referencing byte offsets in the `SourceFile`
3. `swc_ecma_codegen::Emitter` uses `JsWriter` which can optionally produce a source map
4. Spans on emitted nodes map back to positions in the source file

**Current limitation**: The `SourceFile` registered with the `SourceMap` contains the *preprocessed* text, not the original source. This means source positions in error messages and source maps reflect the preprocessed text, not the user's original input. For small rewrites (operator → function call), the offset is minor. For HKT rewrites that change string lengths, positions may shift noticeably.

A future AST-level approach would eliminate this problem: the parser would consume the original source directly, and desugared nodes would carry spans from their original positions.

## Current Implementation vs. Future Direction

### Text-Level Preprocessing (Current)

The preprocessor rewrites source text before SWC parses it. This mirrors typesugar's TypeScript preprocessor approach.

**Advantages**:
- Simple to implement — string manipulation, no parser fork required
- Leverages SWC's parser for all the hard work (expression parsing, type checking support)
- Easy to verify — the preprocessed text is human-readable

**Disadvantages**:
- Operand boundary detection is heuristic (scanning for `;`, `,`, `=`, etc.)
- Type context detection uses keyword tracking, not AST-level scope information
- Source map positions are slightly off for rewritten code
- Error messages from SWC reference the preprocessed text, not the original

### AST-Level Processing (Future)

The `sc_ast` types, `sc_lexer` token merging, and `sc_desugar` transform functions are scaffolding for a future migration:

1. **Lexer** produces `ScToken` stream with merged operators
2. **Parser** (forked SWC parser or custom layer) consumes `ScToken` and produces an AST with `ScBinExpr` and `HktTypeParam` nodes
3. **Desugar** walks the extended AST and produces a standard `swc_ecma_ast::Module`
4. **Codegen** emits from the standard AST with accurate source positions

This eliminates all heuristic boundary detection and gives precise source maps. The trade-off is maintaining a parser fork against upstream SWC releases.
