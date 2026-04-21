# Project Structure

This file is the short, plain-English map of the Rust codebase.

If you already understand Laravel but not Rust, read the project in this order:

1. `src/main.rs`
2. `src/lib.rs`
3. `src/cli.rs`
4. `src/project.rs`
5. `src/analyzers/*`
6. `src/lsp/*`
7. `src/debug/*`

## What This Codebase Can Do

Today this project can:

- scan a Laravel project without booting Laravel
- list effective routes
- show where routes were registered from
- list middleware aliases, groups, and route parameter patterns
- list config keys, env usage, defaults, and merged config sources
- list service providers
- list views, Blade components, and Livewire components
- list Eloquent models and inferred schema details
- list migrations and replay them to infer current columns
- expose an LSP server for completions, hovers, and go-to-definition
- run a terminal debugger and a web debugger for exploring analyzer output
- compare static route output with `php artisan route:list --json` in the web debugger

## Mental Model

Think about the codebase in four layers:

1. `project` layer
   Resolves which Laravel app should be analyzed.

2. `php` layer
   Shared PHP parsing and filesystem helpers. This is the low-level utility layer.

3. `analyzers` layer
   The real domain logic. Each analyzer turns Laravel source code into a typed Rust report.

4. `delivery` layer
   CLI output, LSP responses, and debug UIs. These layers consume analyzer reports instead of reparsing Laravel code.

That separation is the main architectural rule in this repo.

## Source Tree

```text
src/
├── main.rs                 Thin binary entry point
├── lib.rs                  Command dispatcher
├── cli.rs                  CLI argument parsing
├── project.rs              Laravel project discovery and resolution
├── types.rs                Shared report/data structs
│
├── php/                    Reusable PHP helpers
│   ├── ast.rs              AST text extraction and expression helpers
│   ├── env.rs              `.env` / `.env.example` loading and expansion
│   ├── psr4.rs             Composer PSR-4 resolution
│   └── walk.rs             AST walker utilities
│
├── analyzers/              Laravel feature analyzers
│   ├── mod.rs              Analyzer trait and module exports
│   ├── providers.rs        Provider discovery
│   ├── middleware.rs       Middleware alias/group/pattern discovery
│   ├── models.rs           Eloquent model inspection
│   ├── migrations.rs       Migration parsing and schema replay
│   ├── views.rs            Blade/Livewire/view inspection
│   ├── configs/            Config file + merge source analysis
│   └── routes/             Route file collection and route parsing
│
├── output/                 Human-readable and JSON rendering
│   ├── mod.rs              Output dispatch
│   ├── json.rs             JSON printing
│   └── text/               Table renderers for terminal output
│
├── lsp/                    Editor-facing query engine
│   ├── context.rs          Detects what symbol/helper the cursor is on
│   ├── index.rs            Builds searchable config/route/env indexes
│   ├── overrides.rs        Unsaved document overrides
│   ├── query.rs            Completion, hover, and definition responses
│   └── server.rs           JSON-RPC / stdio transport
│
└── debug/                  Developer exploration tools
    ├── browser.rs          Terminal UI
    ├── command.rs          Shared debug command catalog
    ├── reports.rs          Shared debug report rendering and route comparison
    ├── web.rs              Local HTTP server for the web UI
    └── mod.rs              Debug module entry points
```

## Laravel-Style Reading Guide

If you want to read this like a Laravel app, map it like this:

- `src/project.rs` is similar to application bootstrapping and app discovery
- `src/analyzers/*` are similar to Laravel subsystems or bounded services
- `src/types.rs` is similar to framework DTOs / view models
- `src/output/*` is similar to presenters / resources
- `src/lsp/*` is similar to an API layer built on top of domain services
- `src/debug/*` is similar to internal tooling around the same domain services

The important takeaway: the analyzers are the real core. Everything else is delivery.

## Best Entry Points By Goal

- Want CLI behavior: read `src/cli.rs`, then `src/lib.rs`
- Want route logic: start at `src/analyzers/routes/mod.rs`
- Want config logic: start at `src/analyzers/configs/mod.rs`
- Want model or migration logic: start at `src/analyzers/models.rs` and `src/analyzers/migrations.rs`
- Want editor/LSP behavior: read `src/lsp/server.rs`, then `src/lsp/index.rs`, then `src/lsp/query.rs`
- Want debug UI behavior: read `src/debug/browser.rs` and `src/debug/web.rs`

## Current Cleanup Notes

This repo now follows these organization rules more clearly:

- shared debugger command/report logic lives in `src/debug/` instead of being duplicated
- analyzer logic stays separate from output logic
- LSP stays as a thin layer over analyzer indexes
- embedded unit-test blocks were removed from production source files to keep runtime modules shorter
