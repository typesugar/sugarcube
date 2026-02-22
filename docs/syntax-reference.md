# Sugarcube Syntax Reference

Complete reference for all sugarcube syntax extensions. For internal architecture, see [architecture.md](./architecture.md).

Sugarcube desugars custom syntax into **valid TypeScript** — no custom syntax remains in the output. The downstream [typesugar](https://github.com/typesugar/typesugar) transformer handles macro expansion and typeclass resolution.

---

## Pipeline Operator (`|>`)

**Status**: Stable

### Grammar

```
PipelineExpr ::= Expr "|>" Expr
```

The `|>` token is two adjacent characters with no whitespace between them. `| >` (with a space) is parsed as bitwise OR followed by greater-than.

### Precedence and Associativity

- **Precedence**: 1 (lowest of all operators, including standard JS operators)
- **Associativity**: Left

This means `|>` always binds last. Arithmetic, comparisons, and all other expressions bind tighter.

### Desugaring

```
a |> f        →  __binop__(a, "|>", f)
a |> f |> g   →  __binop__(__binop__(a, "|>", f), "|>", g)
```

The `__binop__` function is resolved by typesugar's transformer via `tryRewriteOperator` in `macro-transformer.ts`.

### AST Rewrite Rule

```
BinExpr(left, Pipeline, right)
  → CallExpr(
      callee: Ident("__binop__"),
      args: [left, Str("|>"), right]
    )
```

### Examples

**Basic piping**:

```typescript
// Input
const result = data |> transform |> format;

// Output
const result = __binop__(__binop__(data, "|>", transform), "|>", format);
```

**With function calls**:

```typescript
// Input
const x = getUsers() |> filter(isActive) |> map(getName);

// Output
const x = __binop__(__binop__(getUsers(), "|>", filter(isActive)), "|>", map(getName));
```

**With arrow functions**:

```typescript
// Input
const doubled = [1, 2, 3] |> xs => xs.map(x => x * 2);

// Output
const doubled = __binop__([1, 2, 3], "|>", xs => xs.map(x => x * 2));
```

**Mixed with cons**:

```typescript
// Input (:: binds tighter than |>)
const result = 1 :: 2 :: [] |> reverse;

// Output
const result = __binop__(__binop__(1, "::", __binop__(2, "::", [])), "|>", reverse);
```

**Multiline pipeline**:

```typescript
// Input
const result = rawData
  |> parse
  |> validate
  |> transform
  |> serialize;

// Output
const result = __binop__(__binop__(__binop__(__binop__(rawData, "|>", parse), "|>", validate), "|>", transform), "|>", serialize);
```

### Edge Cases

- **Inside strings**: `"|>"` is not rewritten. The preprocessor skips string literals, template literals, and comments.
- **Inside comments**: `// a |> b` and `/* a |> b */` are left untouched.
- **In type positions**: `|>` inside type annotations, `type` aliases, and `interface` declarations is not rewritten. The preprocessor tracks type context via keyword detection and colon/angle-bracket depth.
- **Whitespace**: `|>` requires no space between `|` and `>`. However, spaces around the operator are fine: `a |> b`, `a|>b`, `a |>b` all work.
- **Newlines**: The operator can span lines — `a\n|> b` works because the preprocessor operates on the full source text, not line-by-line.
- **No operand**: Bare `|>` without left or right operand will produce a malformed `__binop__()` call that SWC will reject as a parse error.

### Type Context Behavior

The preprocessor does **not** rewrite `|>` when it appears inside:
- Type annotations (after `:` in variable/parameter declarations)
- Generic type parameters (inside `<` ... `>`)
- `type` alias declarations
- `interface` declarations

This prevents accidental rewriting of union types containing `>` that might look like `|>` to a naive scanner.

### Feature Flag

```rust
ScSyntax { pipeline: true, ..Default::default() }
```

Set `pipeline: false` to disable. When disabled, `|>` is passed through as-is (which will cause SWC parse errors since it's not valid TypeScript).

### Reference Implementation

- Preprocessor: `crates/sc_parser/src/preprocess/operator_pass.rs`
- AST desugar (future): `crates/sc_desugar/src/pipeline.rs`
- Typesugar equivalent: `~/src/typesugar/packages/preprocessor/src/extensions/pipeline.ts`

---

## Cons Operator (`::`)

**Status**: Stable

### Grammar

```
ConsExpr ::= Expr "::" Expr
```

The `::` token is two adjacent colons with no whitespace. `: :` (with a space) is parsed as two separate colons.

### Precedence and Associativity

- **Precedence**: 5 (higher than pipeline, lower than standard JS arithmetic)
- **Associativity**: Right

Right-associativity means `1 :: 2 :: []` naturally builds a list from the right: `1 :: (2 :: [])`.

### Desugaring

```
a :: b        →  __binop__(a, "::", b)
1 :: 2 :: []  →  __binop__(1, "::", __binop__(2, "::", []))
```

### AST Rewrite Rule

```
BinExpr(left, Cons, right)
  → CallExpr(
      callee: Ident("__binop__"),
      args: [left, Str("::"), right]
    )
```

### Examples

**Basic cons**:

```typescript
// Input
const list = 1 :: [];

// Output
const list = __binop__(1, "::", []);
```

**Chained cons (right-associative)**:

```typescript
// Input
const list = 1 :: 2 :: 3 :: [];

// Output
const list = __binop__(1, "::", __binop__(2, "::", __binop__(3, "::", [])));
```

**With expressions**:

```typescript
// Input
const result = head(xs) :: tail(xs);

// Output
const result = __binop__(head(xs), "::", tail(xs));
```

**Cons then pipeline**:

```typescript
// Input
const result = 1 :: 2 :: [] |> reverse;

// Output (:: binds tighter, then |> wraps the whole thing)
const result = __binop__(__binop__(1, "::", __binop__(2, "::", [])), "|>", reverse);
```

**Pattern-like usage**:

```typescript
// Input
const [head, ...tail] = data;
const rebuilt = head :: tail;

// Output
const [head, ...tail] = data;
const rebuilt = __binop__(head, "::", tail);
```

### Edge Cases

- **Inside strings**: `"::"` is not rewritten.
- **Inside comments**: `// x :: y` is left untouched.
- **In type positions**: `::` is not rewritten inside type annotations or type declarations. TypeScript doesn't currently use `::` in types, but the preprocessor guards against it.
- **Single colon**: A single `:` (type annotation) is never confused with `::` — the preprocessor checks for two adjacent colons specifically.
- **Ambiguity note**: TypeScript does not use `::` today, but future TS versions might (TC39 bind operator proposal used `::` historically). Monitor TC39 proposals.

### Type Context Behavior

Same as pipeline — the preprocessor does not rewrite `::` in type annotations, generics, `type` aliases, or `interface` blocks.

### Feature Flag

```rust
ScSyntax { cons: true, ..Default::default() }
```

Set `cons: false` to disable.

### Reference Implementation

- Preprocessor: `crates/sc_parser/src/preprocess/operator_pass.rs`
- AST desugar (future): `crates/sc_desugar/src/cons.rs`
- Typesugar equivalent: `~/src/typesugar/packages/preprocessor/src/extensions/cons.ts`

---

## HKT Type Parameters (`F<_>`)

**Status**: Stable

### Grammar

```
HktTypeParam   ::= UpperIdent "<" "_" ("," "_")* ">"
HktUsage       ::= UpperIdent "<" TypeArgList ">"    (within HKT scope)
```

`F<_>` declares that `F` is a higher-kinded type parameter. Within the scope of that declaration, all type references `F<A>` are rewritten to `$<F, A>`.

### Desugaring

**Declarations**: Strip `<_>` from the type parameter.

```
interface Functor<F<_>>  →  interface Functor<F>
type Apply<F<_>, A>      →  type Apply<F, A>
```

**Usages** (within declaring scope): Rewrite `F<A>` to `$<F, A>`.

```
F<A>       →  $<F, A>
F<A, B>    →  $<F, A, B>
```

The `$` type is defined in typesugar's type system:

```typescript
type $<F, A> = (F & { readonly _: A })["_"];
```

### AST Rewrite Rules

```
TsTypeParam { name: F, <_> }        → TsTypeParam { name: F }
TsTypeRef { name: F, params: [A] }  → TsTypeRef { name: $, params: [F, A] }
```

The usage rewrite only applies when `F` was declared with `<_>` in an enclosing scope.

### Examples

**Basic Functor**:

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

**Type alias with HKT**:

```typescript
// Input
type Lift<F<_>, A> = F<A>;

// Output
type Lift<F, A> = $<F, A>;
```

**Function with HKT constraint**:

```typescript
// Input
function traverse<F<_>, A, B>(
  xs: Array<A>,
  f: (a: A) => F<B>
): F<Array<B>>;

// Output
function traverse<F, A, B>(
  xs: Array<A>,
  f: (a: A) => $<F, B>
): $<F, Array<B>>;
```

**Nested/shadowed HKT**:

```typescript
// Input
interface Outer<F<_>> {
  inner: <G<_>>(fa: F<G<number>>) => G<string>;
}

// Output — F<...> and G<...> both rewritten within their respective scopes
interface Outer<F> {
  inner: <G>(fa: $<F, $<G, number>>) => $<G, string>;
}
```

**Outside scope — not rewritten**:

```typescript
// Input
interface Functor<F<_>> {
  map: <A, B>(fa: F<A>) => F<B>;
}
const x: F<number> = foo;  // F<number> is outside Functor's scope

// Output
interface Functor<F> {
  map: <A, B>(fa: $<F, A>) => $<F, B>;
}
const x: F<number> = foo;  // unchanged — not in HKT scope
```

### Edge Cases

- **Scope boundaries**: The HKT declaration's scope extends from the start of the containing declaration (backward to `}` or `;`) to the end (forward to the matching `}`). Usages outside this scope are not rewritten.
- **Shadowing**: An inner `F<_>` declaration shadows an outer one. The preprocessor picks the innermost (smallest) scope when multiple declarations of the same name overlap.
- **Non-uppercase identifiers**: Only identifiers starting with an uppercase ASCII letter are candidates for HKT. `f<_>` is not treated as HKT.
- **Inside strings/comments**: `F<_>` in strings and comments is not processed.
- **Multi-arity HKT**: `F<_, _>` declares a two-argument HKT. The `<_, _>` is stripped, and `F<A, B>` becomes `$<F, A, B>`.
- **Not a declaration**: `F<A>` where `F` was not declared with `<_>` in any enclosing scope is left unchanged.

### Type Context Behavior

HKT rewriting is inherently type-level — it only affects type parameter declarations and type references. It does not interact with expression-level code.

### Feature Flag

```rust
ScSyntax { hkt: true, ..Default::default() }
```

Set `hkt: false` to disable. When disabled, `F<_>` passes through to SWC, which will likely reject `_` as a type argument.

### Reference Implementation

- Preprocessor: `crates/sc_parser/src/preprocess/hkt_pass.rs`
- AST desugar (future): `crates/sc_desugar/src/hkt.rs` (`HktRewriter`)
- Typesugar equivalent: `~/src/typesugar/packages/preprocessor/src/extensions/hkt.ts`
- HKT encoding: `~/src/typesugar/packages/type-system/src/hkt.ts`

---

## Operator Precedence Table

All sugarcube operators relative to each other:

| Precedence | Operator | Associativity | Desugars To |
|---|---|---|---|
| 5 | `::` (cons) | Right | `__binop__(l, "::", r)` |
| 1 | `\|>` (pipeline) | Left | `__binop__(l, "\|>", r)` |

Standard JavaScript operators all have higher precedence than both sugarcube operators. Within sugarcube, `::` always binds before `|>`.

---

## Planned Extensions

The following extensions are planned but **not yet implemented**. Do not use them — they will cause parse errors.

### Do Expressions

**Status**: Planned (TC39 Stage 1)

```
DoExpr ::= "do" "{" Statement* Expression "}"
```

Allows block expressions in expression position:

```typescript
// Planned syntax
const x = do {
  const a = getA();
  const b = getB();
  a + b
};

// Expected desugaring: IIFE wrapping
const x = (() => {
  const a = getA();
  const b = getB();
  return a + b;
})();
```

### Method Annotations

**Status**: Planned

Re-enable richer decorator syntax on class methods beyond what TypeScript currently supports. Details TBD based on TC39 decorator evolution.

### Pattern Matching

**Status**: Planned (TC39 Stage 1)

```
MatchExpr ::= "match" "(" Expr ")" "{" WhenClause+ "}"
WhenClause ::= "when" Pattern ":" Expr
```

```typescript
// Planned syntax
const label = match (status) {
  when "active": "Running"
  when "paused": "On Hold"
  when "stopped": "Finished"
};

// Expected desugaring: if/else or switch chain
```

### Custom Operators (General)

**Status**: Planned

User-defined binary operators via a configuration file, all desugaring through the `__binop__` pattern.

### Design Constraints for Future Extensions

- Every extension must desugar to **valid TypeScript** — sugarcube never emits custom syntax
- The `__binop__` pattern is the standard approach for new binary operators
- HKT-style type rewrites should use the `$<F, A>` encoding
- Feature flags must default to `true` in `ScSyntax::default()`
- New extensions must not break existing ones — the test harness enforces this
