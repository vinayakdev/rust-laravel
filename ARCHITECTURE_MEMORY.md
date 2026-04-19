# Rust PHP Analyzer Memory

## What this codebase is

This repository is a Rust CLI that statically analyzes Laravel projects.

It does not boot Laravel, execute PHP, or depend on Composer runtime behavior. It reads project files from disk, parses PHP source, infers Laravel structure, and emits either terminal tables or JSON reports.

The main Rust application lives under `src/`. The Laravel example and test projects are inputs for the analyzer, not the analyzer itself.

## Main execution flow

1. `src/main.rs` calls `rust_php::run()`.
2. `src/lib.rs` parses CLI args, resolves the target Laravel project, runs one analyzer, and sends the report to the output layer.
3. `src/project.rs` resolves the Laravel project from:
   - `--project <path-or-name>`
   - the current directory if it looks like a Laravel app
   - a single project under `./laravel-example/`
4. `src/output/` renders reports either as text tables or JSON.

## Supported CLI commands

- `route:list`
- `route:sources`
- `middleware:list`
- `config:list`
- `config:sources`
- `provider:list`

These commands all analyze a Laravel project and return structured report data.

## Core architecture

### Provider analysis

File: `src/analyzers/providers.rs`

This is a foundational analyzer because other analyzers depend on discovered providers.

It finds providers from:

- `bootstrap/providers.php`
- root `composer.json` under `extra.laravel.providers`
- local package `composer.json` files under `packages/*/*/composer.json`

It then resolves provider classes to source files using PSR-4 mappings gathered from Composer metadata in `src/php/psr4.rs`.

Output includes:

- provider class
- where it was declared
- registration kind
- package name when known
- resolved source file if found
- status like `static_exact` or `source_missing`

### Middleware analysis

File: `src/analyzers/middleware.rs`

This analyzer reads resolved provider source files and looks for static `Route::...` middleware registration calls.

It extracts:

- middleware aliases via `Route::aliasMiddleware`
- middleware groups via `Route::middlewareGroup`
- route parameter patterns via `Route::pattern`

This middleware data is later reused by the route analyzer to expand middleware and attach parameter patterns to routes.

### Route analysis

Files:

- `src/analyzers/routes/mod.rs`
- `src/analyzers/routes/collector.rs`
- `src/analyzers/routes/parser.rs`
- `src/analyzers/routes/chain.rs`
- `src/analyzers/routes/context.rs`

Route analysis works in two stages.

First, the collector finds route files from:

- direct `routes/**/*.php`
- provider calls to `loadRoutesFrom(...)`

Second, the parser reads those route files and reconstructs route definitions.

It understands:

- `Route::get`, `post`, `put`, `patch`, `delete`, `options`, `any`, `match`
- route groups
- prefixes
- name prefixes
- controller context
- middleware chains
- middleware alias/group expansion
- route parameter pattern attachment
- route registration attribution

The route parser first tries a full PHP parse. If that fails, it falls back to chunk-based parsing for route-heavy files that are syntactically awkward but still analyzable.

### Config analysis

Files:

- `src/analyzers/configs/mod.rs`
- `src/analyzers/configs/collector.rs`
- `src/analyzers/configs/extractor.rs`

Config analysis also has two stages.

First, the collector gathers config files from:

- root `config/*.php`
- provider calls to `mergeConfigFrom(...)`

Second, the extractor walks config file text line by line and derives config items.

It records:

- full config key
- file, line, and column
- env key from `env(...)` when present
- default value when it can infer one
- resolved env value from `.env` or `.env.example`
- registration source attribution

This is simpler than the route analyzer. It is not a full PHP evaluator; it is a targeted extractor for common Laravel config structures.

## Shared PHP support layer

Files under `src/php/` provide the parsing helpers used by analyzers.

- `ast.rs`: string/path extraction helpers, line/column helpers, path normalization
- `psr4.rs`: Composer JSON reading and PSR-4 class-to-file resolution
- `env.rs`: `.env` / `.env.example` loading and `${VAR}` expansion
- `walk.rs`: recursive AST expression walking helpers

The Rust app uses the `php-parser` crate to parse PHP into an AST.

## Output model

File: `src/types.rs`

The codebase defines typed report structs for:

- routes
- configs
- providers
- middleware

Those reports are serialized to JSON or rendered as text tables in `src/output/`.

## Practical summary

This project is best understood as a static Laravel inspection engine written in Rust.

The dependency chain is roughly:

1. resolve Laravel project
2. discover providers
3. extract middleware and provider-linked resources
4. analyze routes and configs
5. render reports as text or JSON

## Important limitations

- No PHP execution
- No Laravel bootstrap
- No service container execution
- No Composer install/runtime dependency
- Provider-linked analysis is strongest when provider classes resolve through PSR-4
- Route parsing is more robust than config parsing
- Config extraction is heuristic and line-oriented, not full AST evaluation

## Files to open first next time

- `src/lib.rs`
- `src/project.rs`
- `src/analyzers/providers.rs`
- `src/analyzers/middleware.rs`
- `src/analyzers/routes/mod.rs`
- `src/analyzers/routes/collector.rs`
- `src/analyzers/routes/parser.rs`
- `src/analyzers/routes/chain.rs`
- `src/analyzers/configs/collector.rs`
- `src/analyzers/configs/extractor.rs`
- `src/php/psr4.rs`
- `src/types.rs`
