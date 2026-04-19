# rust-php

Rust tooling for exploring Laravel codebases while building toward editor and LSP features.

## What This Project Does

Right now the project gives you:

- `route:list`: parse Laravel route files and print discovered routes
- `route:list --json`: emit the same route data as machine-readable JSON
- `config:list`: scan PHP files for `config(...)` references
- broken-file recovery for route parsing, so useful output still appears even when some PHP is malformed

The long-term direction is:

- reusable analyzers in Rust
- CLI commands for debugging those analyzers on real Laravel apps
- LSP-friendly data structures and output

## Workspace Layout

```text
rust-php/
  src/
    analyzers/
      routes.rs
      configs.rs
    cli.rs
    lib.rs
    main.rs
    output.rs
    project.rs
    types.rs
  laravel-example/
    <your-laravel-projects-here>
```

Put Laravel apps under `laravel-example/` like this:

```text
laravel-example/
  demo-app/
    app/
    config/
    routes/
```

## Commands

Development:

```bash
cargo run -- route:list
cargo run -- route:list --json
cargo run -- config:list
```

Target a Laravel project by name:

```bash
cargo run -- route:list --project demo-app
cargo run -- route:list --project demo-app --json
```

Target a Laravel project by path:

```bash
cargo run -- route:list --project ./laravel-example/demo-app
```

Release binary:

```bash
cargo build --release
./target/release/rust-php route:list
./target/release/rust-php route:list --json
./target/release/rust-php config:list
```

## Project Resolution

When you pass `--project`, resolution works like this:

1. If it is an existing path, use it directly.
2. Otherwise resolve it under `./laravel-example/<name>`.

When you do not pass `--project`, resolution works like this:

1. If the current directory looks like a Laravel app, use it.
2. Otherwise, if `./laravel-example` itself looks like a Laravel app, use it.
3. Otherwise auto-pick a single Laravel app under `./laravel-example/`.

## Output Modes

### Text output

Human-friendly table output grouped by file:

```text
routes/web.php
  LINE  METHOD    URI                 NAME            ACTION                  MIDDLEWARE
    16  GET       /products/{slug}    products.show   ProductController@show  -
```

### JSON output

Structured output for extension/LSP work:

```json
{
  "project_name": "demo-app",
  "project_root": "/abs/path/to/demo-app",
  "route_count": 2,
  "routes": [
    {
      "file": "routes/web.php",
      "line": 16,
      "methods": ["GET"],
      "uri": "/products/{slug}",
      "name": "products.show",
      "action": "ProductController@show",
      "middleware": []
    }
  ]
}
```

## Code Structure

### `src/lib.rs`

Top-level orchestration. This is the place where commands are resolved and dispatched. For an extension or LSP, this becomes the natural integration entrypoint.

### `src/cli.rs`

Argument parsing. Keep CLI-only concerns here so analyzers stay reusable.

### `src/project.rs`

Resolves which Laravel project to inspect. This is useful both for local debugging and for future editor integration where a workspace folder maps to a project root.

### `src/types.rs`

Shared report types. This is the contract between analyzers and output layers.

### `src/output.rs`

Human-readable and JSON rendering. Keeping output separate makes it easy to plug the analyzers into:

- a CLI
- tests
- JSON-RPC handlers
- LSP features

### `src/analyzers/routes.rs`

Laravel route analyzer. This is currently the most advanced module:

- full-file parse for valid code
- fallback chunk recovery for broken route files
- support for route chains, prefixes, names, middleware, and controller groups

### `src/analyzers/configs.rs`

Starter analyzer for config references. This is intentionally simple and can later move from string scanning to parser-backed extraction.

## How To Extend This

Recommended pattern for new features:

1. Add a new report type in `src/types.rs`.
2. Add a new analyzer in `src/analyzers/`.
3. Add a new CLI command in `src/cli.rs`.
4. Add output formatting in `src/output.rs`.
5. Wire the command in `src/lib.rs`.

This keeps parsing logic, transport, and presentation separate.

## LSP Roadmap

A practical path from this CLI to an LSP backend:

1. Keep analyzers pure: input project, output typed report.
2. Add file-level indexing so changed files can be reanalyzed incrementally.
3. Add span/range-aware types for diagnostics and symbol navigation.
4. Add JSON-RPC request handlers around the analyzers.
5. Add watchers and cache invalidation for edited PHP files.

Likely future features:

- route definition navigation
- route name completion
- controller action lookup
- config key completion
- diagnostics for unresolved route/controller references

## Rust Learning Notes

This codebase is a good place to learn a few Rust habits:

### 1. Separate data from side effects

The analyzers produce typed reports. Printing is a separate concern.

Why this matters:

- easier to test
- easier to reuse in an extension
- easier to serialize to JSON

### 2. Use modules to control complexity

Instead of one large `main.rs`, the code is split by responsibility. This is the first step toward maintainable Rust, especially when the project grows.

### 3. Optimize the common path first

Valid Laravel projects are the common case, so the route analyzer parses full files first. The chunk recovery path is only used when code is malformed.

### 4. Prefer explicit types at boundaries

`RouteReport`, `RouteEntry`, `ConfigReport`, and `ConfigReference` make the code easier to reason about than passing around loose maps or tuples.

## Debugging Workflow

When adding new analyzers or LSP behavior:

1. Put a real Laravel app under `laravel-example/`.
2. Run the analyzer in text mode first.
3. Run it again with `--json`.
4. Compare the output with what Laravel or your editor shows.
5. Only after the CLI output is correct, wire it into LSP features.

That CLI-first workflow is usually much faster than debugging through an editor integration immediately.

## Next Good Steps

- make `config:list` parser-backed instead of string-backed
- add `project:list`
- add file/line ranges instead of only line numbers
- add controller symbol resolution
- add tests with example Laravel fixtures
- add a JSON-RPC layer for editor integration
