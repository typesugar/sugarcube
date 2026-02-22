# Integration Guide

How to use sugarcube with typesugar, build tools, and other tooling.

## Overview

Sugarcube is a preprocessing step that converts custom syntax extensions (`|>`, `::`, `F<_>`) into valid TypeScript. It sits before typesugar's macro transformer and tsc in the build pipeline:

```
source.ts (sugarcube syntax)
    │
    ▼
  sc preprocess
    │
    ▼
output.ts (standard TypeScript)
    │
    ├─────────────────────────────┐
    ▼                             ▼
typesugar transformer           tsc
(macro expansion)           (type checking)
    │
    ▼
output.js / .d.ts
```

## 1. Standalone CLI

The simplest way to use sugarcube is as a standalone preprocessing step.

### Basic usage

```bash
# Desugar to stdout
sc preprocess src/mymodule.ts

# Desugar to a file
sc preprocess src/mymodule.ts -o dist/mymodule.ts

# Process TSX files
sc preprocess src/App.tsx --tsx -o dist/App.tsx

# Generate source maps
sc preprocess src/mymodule.ts -o dist/mymodule.ts --source-map
```

### Other commands

```bash
# Syntax-check only (no output, exit code indicates success/failure)
sc check src/mymodule.ts

# Dump the parsed AST as JSON (useful for debugging)
sc parse src/mymodule.ts --ast
```

### Batch processing

Sugarcube processes one file at a time. For batch processing, use shell scripting or a build tool:

```bash
for f in src/**/*.ts; do
  out="dist/${f#src/}"
  mkdir -p "$(dirname "$out")"
  sc preprocess "$f" -o "$out" --source-map
done
```

## 2. As a Rust Library (Future)

The `sc_parser` and `sc_desugar` crates are designed to be usable as libraries from other Rust tools (e.g., custom bundlers, editor plugins, or WASM targets).

### API sketch

```rust
use sc_ast::ScSyntax;
use sc_parser::parse_sugarcube;
use sc_desugar::desugar_module;
use swc_ecma_codegen::{text_writer::JsWriter, Emitter, Node};

let syntax = ScSyntax::default();
let parsed = parse_sugarcube(source, filename, &syntax)?;
let module = desugar_module(parsed.module);

// Emit back to TypeScript using SWC's codegen
let mut buf = Vec::new();
let writer = JsWriter::new(parsed.source_map.clone(), "\n", &mut buf, None);
let mut emitter = Emitter {
    cfg: swc_ecma_codegen::Config::default()
        .with_target(swc_ecma_ast::EsVersion::latest()),
    cm: parsed.source_map,
    comments: None,
    wr: writer,
};
module.emit_with(&mut emitter)?;

let output = String::from_utf8(buf)?;
```

### Selective extensions

Disable individual extensions by setting feature flags:

```rust
let syntax = ScSyntax {
    pipeline: true,
    cons: true,
    hkt: false,  // disable HKT rewriting
};
```

### WASM target (planned)

A future goal is to compile `sc_parser` + `sc_desugar` to WASM so JavaScript build tools can call sugarcube without spawning a subprocess. This would enable tighter integration with unplugin-typesugar and editor plugins.

## 3. With typesugar's unplugin

[unplugin-typesugar](https://github.com/typesugar/typesugar) provides framework-agnostic build integration (Vite, Webpack, Rollup, esbuild). When sugarcube is installed, unplugin calls `sc` instead of the JavaScript preprocessor.

### How it works

1. unplugin intercepts `.ts` / `.tsx` files during the build
2. If `sc` is on `$PATH`, unplugin spawns `sc preprocess <file> --source-map` and captures stdout
3. If `sc` is not available, unplugin falls back to the JavaScript preprocessor (`@typesugar/preprocessor`)
4. The preprocessed output (valid TypeScript) is passed to typesugar's transformer for macro expansion

### Configuration

In your `vite.config.ts` (or equivalent):

```typescript
import typesugar from 'unplugin-typesugar/vite';

export default {
  plugins: [
    typesugar({
      // unplugin auto-detects sc on PATH; you can also set the path explicitly:
      preprocessor: 'sc',
      // Or force the JS preprocessor:
      // preprocessor: 'js',
    }),
  ],
};
```

### Error handling

When `sc` exits with a non-zero code, unplugin captures stderr and reports it as a build error. Parse errors from sugarcube show the original source location (subject to source map accuracy — see section 5).

### Performance

The Rust-based `sc` binary is significantly faster than the JavaScript preprocessor for large files. For typical project sizes the difference is negligible, but for monorepos with hundreds of sugarcube files, it can noticeably improve build times.

## 4. With ts-patch

[ts-patch](https://github.com/nonara/ts-patch) patches the TypeScript compiler to support custom transformers. typesugar uses this to run its macro transformer during `tsc` compilation.

### Pipeline with ts-patch

1. **Sugarcube preprocesses** the source files (either via unplugin during dev, or as a pre-build step)
2. **tsc (patched by ts-patch)** reads the preprocessed files
3. **typesugar's transformer** runs as a ts-patch plugin, expanding macros, typeclasses, and derives
4. **tsc emits** the final JavaScript and `.d.ts` output

### tsconfig.json setup

```json
{
  "compilerOptions": {
    "plugins": [
      {
        "transform": "@typesugar/transformer",
        "transformProgram": true
      }
    ]
  }
}
```

### What the transformer sees

After sugarcube, the transformer receives valid TypeScript with:

- `__binop__(a, "|>", f)` calls where pipeline operators were
- `__binop__(1, "::", [])` calls where cons operators were
- `$<F, A>` type references where HKT usages were

The transformer's `tryRewriteOperator` function in `macro-transformer.ts` recognizes `__binop__` calls and resolves them through typesugar's operator dispatch system.

### Pre-build script approach

If you're not using unplugin (e.g., for a pure tsc build):

```json
{
  "scripts": {
    "prebuild": "for f in src/**/*.ts; do sc preprocess \"$f\" -o \"build/${f}\"; done",
    "build": "tsc -p tsconfig.json --rootDir build"
  }
}
```

## 5. Source Map Chaining

End-to-end source mapping requires composing sugarcube's source map with typesugar's transformer source map.

### How it works

1. **Sugarcube** generates a source map mapping preprocessed output positions back to original source positions (`--source-map` flag)
2. **typesugar's transformer** generates its own source map mapping transformed output back to its input (the preprocessed TypeScript)
3. **Build tools** (Vite, Webpack) compose these maps automatically when both are provided as standard v3 source maps

### Current limitations

Sugarcube's source maps flow through SWC's standard span-based system. Because the current implementation uses text-level preprocessing (rewriting source text before SWC parses it), the spans don't fully account for preprocessing offsets. This means:

- **Most positions are accurate** — SWC's parser assigns spans based on the preprocessed text, which maps 1:1 for unchanged regions
- **Desugared regions may be slightly off** — a `__binop__(a, "|>", f)` call maps back to the preprocessed text, not the original `a |> f`. The offset difference is usually small (a few characters)
- **HKT rewrites can shift more** — `$<F, A>` is a different length than `F<A>`, and the `<_>` removal shifts subsequent positions

### Practical impact

For development (error messages, debugger breakpoints), the current accuracy is generally good enough — you'll land on the right line, possibly off by a few characters within the line. For production source maps in stack traces, the composed map is accurate for unchanged code and approximately correct for desugared regions.

A future iteration may improve this by tracking preprocessing offsets and adjusting spans before codegen.
