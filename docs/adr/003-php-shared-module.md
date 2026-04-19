# ADR 003 — Shared PHP Utilities (`src/php/`)

## Status
Accepted

## Context

Before this refactor, four analyzer files each contained their own copy of:

| Function | Duplicated in |
|---|---|
| `span_text()` | routes, providers, middleware, configs |
| `parse_php_string_literal()` | routes, middleware, configs |
| `expr_name()` | routes, middleware, configs |
| `expr_to_string()` | routes, middleware, configs |
| `expr_to_string_list()` | routes, middleware |
| `expr_to_path()` + `expr_to_path_fragment()` | routes, configs |
| `normalize_path()` | routes, configs |
| `strip_root()` | routes, providers, configs |
| PSR-4 mapping logic | providers only (but conceptually reusable) |
| `.env` parsing | configs only (but conceptually reusable) |

Any bug fix or improvement had to be applied in multiple places.

## Decision

Extract all PHP-ecosystem utilities into `src/php/`, with no dependency on
`src/project.rs` or `src/types.rs`:

```
src/php/
├── ast.rs    — AST expression helpers and path utilities
├── env.rs    — .env / .env.example loading and ${VAR} expansion
├── psr4.rs   — PSR-4 autoload mapping, class-to-file resolution, read_json
└── walk.rs   — Recursive PHP statement walker
```

### `php::ast`

All functions take `ExprId<'_>` and `source: &[u8]` (the raw bytes of the file).
No project context is needed — callers pass `project_root: &Path` where needed
rather than `&LaravelProject`, keeping this module dependency-free.

`expr_name` uses the **permissive** variant (falls back to `span_text` for unknown
expression types). This is what middleware and configs needed. Routes also works
correctly with this variant because class identifiers in static calls (`Route::get`)
are represented by the parser in a way that the span fallback resolves correctly.

### `php::walk`

```rust
pub fn walk_stmts<'ast, F>(stmts: &[StmtId<'ast>], include_class_methods: bool, f: &mut F)
where F: FnMut(ExprId<'ast>)
```

`include_class_methods: true` descends into method bodies inside class/trait/enum
declarations. Use this for provider analysis where middleware and config registration
happen inside `boot()` methods.

`include_class_methods: false` skips class declarations entirely. Use this for route
files where routes are registered at the top level, not inside class methods.

The walker eliminates ~300 lines of nearly-identical recursive match blocks that were
duplicated across `routes.rs`, `middleware.rs`, and `configs.rs`.

## Why Not a Separate Crate

The php-parser crate is not published on crates.io (it's a path dependency). A shared
internal crate would add workspace boilerplate without benefit. `src/php/` as a module
tree achieves the same isolation.

## Consequences

- Bug fixes to `expr_to_string`, `normalize_path`, etc. are made once.
- New analyzers get these utilities for free via `use crate::php::ast::*`.
- `src/php/` has no upward dependencies — it can be tested in isolation.
- Do not add analyzer-specific logic to `src/php/`. If something is only used by
  one analyzer, it belongs in that analyzer's module.
