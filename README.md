# rust-php

Rust tooling for exploring Laravel codebases while building toward editor and LSP features.

## What This Project Does

Right now the project gives you:

- `route:list`: parse Laravel route files and print discovered routes
- `route:list --json`: emit the same route data as machine-readable JSON
- `config:list`: inspect config definitions, `config(...)` usages, `env(...)` usages, and env-file declarations
  Current text output is normalized into `key`, `env_key`, `default_value`, and resolved `env_value`.
- broken-file recovery for route parsing, so useful output still appears even when some PHP is malformed

The long-term direction is:

- reusable analyzers in Rust
- CLI commands for debugging those analyzers on real Laravel apps
- LSP-friendly data structures and output

## Quick Reference

| File                                                    | Purpose                                                           |
| ------------------------------------------------------- | ----------------------------------------------------------------- |
| [README.md](/README.md)                                 | Project overview, architecture, commands, and roadmap             |
| [example.md](/example.md)                               | Concrete input/output examples for `route:list` and `config:list` |
| [laravel-example/README.md](/laravel-example/README.md) | Where to place Laravel projects for analysis                      |

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

Important:

- By default, this tool should parse Laravel code under `laravel-example/`.
- The Rust workspace itself is not the target application code.
- Use `--project <name>` or `--project <path>` when you want to be explicit.

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

1. If `./laravel-example` itself looks like a Laravel app, use it.
2. Otherwise, if the current directory looks like a Laravel app, use it.
3. Otherwise auto-pick a single Laravel app under `./laravel-example/`.

This means local debugging should normally target Laravel code from `laravel-example/`.

## Output Modes

### Text output

Human-friendly table output grouped by file:

```text
routes/web.php
  LINE:COL  METHOD    URI                 NAME            ACTION                  MIDDLEWARE
  16:12     GET       /products/{slug}    products.show   ProductController@show  -
```

Terminal notes:

- boxed Unicode tables are used for better readability
- long values are truncated with `…` on narrower terminals
- config output uses semantic colors:
  - green: env value is present
  - yellow: default value is being used
  - red: env key is referenced but missing from `.env`

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
      "column": 12,
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

Config analyzer. It currently reports:

- config definitions from `config/*.php`
- `config(...)` usages
- `env(...)` usages
- `.env` and `.env.example` declarations

It is still scanner-based and can later move to parser-backed extraction.

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
