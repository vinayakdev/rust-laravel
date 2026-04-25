# Architecture Overview

## Purpose

`rust-php` is a static analysis tool that introspects a Laravel PHP project without executing it.
It reads PHP source files, resolves provider registration chains, and produces structured reports
about routes, config keys, middleware, and service providers.

## Module Map

```
src/
├── main.rs              Entry point — calls lib::run(), exits with error code on failure
├── lib.rs               Command dispatch: parse CLI → resolve project → run analyzer → render output
├── cli.rs               Argument parsing, help text
├── types.rs             All public report types (RouteReport, ConfigReport, …) — no logic
├── project.rs           Laravel project resolution (path → LaravelProject struct)
├── debug/               Shared terminal/web debug tooling
│   ├── mod.rs           Debug entry points used by lib.rs
│   ├── browser.rs       Terminal debugger UI
│   ├── command.rs       Shared debug command catalog
│   ├── reports.rs       Shared JSON/text report rendering + route comparison
│   └── web.rs           Local HTTP server for the web UI
│
├── php/                 Shared PHP-ecosystem utilities (no analyzer-specific logic)
│   ├── ast.rs           PHP AST helpers: span_text, expr_name, expr_to_string, expr_to_path, strip_root, normalize_path
│   ├── env.rs           .env / .env.example loading and ${PLACEHOLDER} expansion
│   ├── psr4.rs          PSR-4 autoload mapping collection and class→file resolution
│   └── walk.rs          Recursive PHP statement walker (eliminates repeated traversal boilerplate)
│
├── analyzers/           One sub-module per concern; each exposes pub fn analyze(project) → Report
│   ├── mod.rs           Analyzer trait definition
│   ├── providers.rs     Discovers service providers from bootstrap/providers.php and composer.json
│   ├── middleware.rs     Extracts middleware aliases, groups, and route patterns from providers
│   ├── routes/
│   │   ├── mod.rs       Orchestrates route collection and sorting
│   │   ├── collector.rs Finds route files (direct + loadRoutesFrom in providers)
│   │   ├── context.rs   RouteContext, MiddlewareIndex, middleware resolution
│   │   ├── chain.rs     Route chain flattening (Route::get()->name()->middleware()), modifiers, builders
│   │   └── parser.rs    Chunk-based fallback parser for malformed PHP, ScanState, sanitize_closure_bodies
│   └── configs/
│       ├── mod.rs       Orchestrates config collection and sorting
│       ├── collector.rs Finds config files (config/ dir + mergeConfigFrom in providers)
│       └── extractor.rs Line-by-line PHP config array key + env() extraction
│
└── output/
    ├── mod.rs           Public print_* functions + Reporter trait
    ├── json.rs          JSON rendering (serde_json::to_string_pretty wrappers)
    └── text/
        ├── mod.rs       Shared table helpers: new_table, header, wrap_cell, terminal_width
        ├── routes.rs    Route list and route sources tables
        ├── configs.rs   Config list and config sources tables
        ├── providers.rs Provider list table
        └── middleware.rs Middleware aliases, groups, and patterns tables
```

## Data Flow

```
CLI args
  │
  ▼
cli::parse()  ──►  CliOptions { command, json, project }
  │
  ▼
project::resolve()  ──►  LaravelProject { root, name }
  │
  ▼
analyzers::<X>::analyze(&project)  ──►  XReport { items, project_name, … }
  │
  ▼
output::print_<x>(&report, mode)
  ├── OutputMode::Json  ──►  output::json::print_<x>()  ──►  serde_json stdout
  └── OutputMode::Text  ──►  output::text::<x>::print_*()  ──►  comfy-table stdout
```

## Key Design Rules

1. **PHP parsing first, always.** Use `php-parser` (the `bumpalo`-backed crate) for any
   structured PHP file. Never write ad-hoc regex or line-by-line scanners for PHP that
   `php-parser` can handle. See `docs/adr/002-php-parsing-strategy.md`.

2. **Shared helpers live in `php/`.** If a helper is used by more than one analyzer,
   it belongs in `src/php/`. Analyzers must not define their own copies of `span_text`,
   `expr_name`, `expr_to_string`, or `normalize_path`.

3. **Analyzers are pure functions.** `analyze()` takes `&LaravelProject`, returns `Result<Report, String>`.
   No global state, no caching across calls, no side effects beyond reading files.

4. **Deterministic output.** All collections are sorted before they leave an analyzer
   (by file, line, column, then name). This makes diffs and tests stable.

5. **Graceful degradation.** Missing files and malformed PHP produce `source_missing`
   status entries, not hard errors. The tool should always produce partial output
   rather than crashing.

## Adding a New Analyzer

1. Create `src/analyzers/<name>/mod.rs` with `pub fn analyze(project: &LaravelProject) -> Result<NameReport, String>`.
2. Add the report struct to `src/types.rs`.
3. Add `pub mod <name>;` to `src/analyzers/mod.rs`.
4. Add a new `Command` variant to `src/cli.rs`.
5. Dispatch it in `src/lib.rs`.
6. Add text renderer to `src/output/text/<name>.rs` and JSON renderer to `src/output/json.rs`.
7. Add `print_<name>()` to `src/output/mod.rs`.
