# ADR 004 — Output Module Structure

## Status
Accepted

## Context

The original `src/output.rs` was 873 lines containing text rendering for all six
commands, JSON rendering, width calculation structs, and shared table utilities.
Adding a new command meant adding ~100 lines to an already large file.

## Decision

Split into a module tree:

```
src/output/
├── mod.rs         Public dispatch functions + Reporter trait
├── json.rs        JSON rendering (serde_json::to_string_pretty wrappers)
└── text/
    ├── mod.rs     Shared helpers: new_table, header, wrap_cell, terminal_width
    ├── routes.rs  Route list + route sources tables
    ├── configs.rs Config list + config sources tables
    ├── providers.rs Provider list table
    └── middleware.rs Middleware aliases, groups, and patterns tables
```

### JSON rendering is trivial

All report types derive `serde::Serialize`. JSON output is always:

```rust
println!("{}", serde_json::to_string_pretty(report).map_err(|e| e.to_string())?);
```

There is one function per report type in `json.rs` but they are all identical in
structure. They exist as named functions (rather than a generic helper) so that
`output/mod.rs` can call them by name in the dispatch match.

### Text rendering is per-report

Each report type has meaningfully different column sets, color logic, and width
breakpoints. Sharing a generic table builder would add abstraction without reducing
complexity. Each `text/<name>.rs` file owns its own width structs and cell builders.

### `Reporter<T>` trait

```rust
pub trait Reporter<T> {
    fn render_text(data: &T) -> Result<(), String>;
    fn render_json(data: &T) -> Result<(), String>;
}
```

Not currently used via dynamic dispatch. It exists as a documented contract: any
future renderer (HTML, LSP-formatted) should implement this trait rather than adding
ad-hoc functions to `output/mod.rs`.

### Terminal width

`terminal_width()` reads `$COLUMNS` first (set by most shells), falling back to 160.
The three breakpoints (< 110, < 150, ≥ 150) map to compact / normal / wide layouts.
Override in tests or CI with `COLUMNS=80 cargo run -- route:list`.

## Consequences

- Adding a new command requires one new `text/<name>.rs` file and one line in `json.rs`.
- Shared cell helpers (`wrap_cell`, `header`, `location_cell`) live in `text/mod.rs`
  with `pub(super)` visibility — they are not part of the public API.
- The `output/` module has no dependency on `src/php/` or `src/analyzers/`.
  It only depends on `src/types.rs`, `comfy-table`, and `serde_json`.
