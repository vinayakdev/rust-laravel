# ADR 001 — Analyzer Trait

## Status
Accepted

## Context

Each analyzer (`routes`, `configs`, `middleware`, `providers`) follows the same pattern:
accept a `&LaravelProject`, return a typed `Result<Report, String>`. Without a shared
contract, future contributors have no clear convention to follow and the dispatch code
in `lib.rs` has no machine-checkable guarantee about what an "analyzer" looks like.

## Decision

Define a `pub trait Analyzer` in `src/analyzers/mod.rs`:

```rust
pub trait Analyzer {
    type Report;
    fn analyze(project: &LaravelProject) -> Result<Self::Report, String>;
}
```

Each analyzer module can optionally implement this trait. The existing `pub fn analyze()`
free functions satisfy the same contract and are kept because Rust trait method dispatch
is less ergonomic for this call pattern (you'd need `<Routes as Analyzer>::analyze()`
instead of `routes::analyze()`).

## Why Not Async

PHP files are small (< 1 MB each), reads are sequential, and the tool runs once and exits.
Async overhead — runtime, `Send` bounds on arena-allocated AST nodes — would complicate
the code for no measurable benefit. If parallelism is ever needed, `rayon` par-iterators
are a better fit than async.

## Why Not Dynamic Dispatch (`Box<dyn Analyzer>`)

Dynamic dispatch would require `dyn Analyzer<Report = ?>` which needs an associated type
on the trait object, which Rust does not support. The six `match` arms in `lib.rs` are
simpler and compile to zero overhead.

## Consequences

- Adding a new analyzer has a documented checklist (see `docs/architecture.md`).
- The trait serves as in-code documentation even if never called through the vtable.
