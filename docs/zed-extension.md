# Zed Dev Extension Setup

This repository now includes a Zed dev extension in [zed-extension](/Users/hotdogb/Work/rust-php/zed-extension).

## What It Launches

The extension starts:

```bash
rust-php lsp
```

The main binary now supports that command directly.

## Install The Dev Extension

1. Open Zed.
2. Run `zed: install dev extension`.
3. Select the [zed-extension](/Users/hotdogb/Work/rust-php/zed-extension) directory.

## Build The Server Binary

Build the analyzer binary first:

```bash
cargo build
```

or for a release binary:

```bash
cargo build --release
```

## Zed Settings

Point Zed at the binary and enable the server for PHP:

```json
{
  "lsp": {
    "rust-php-lsp": {
      "binary": {
        "path": "/absolute/path/to/rust-php"
      }
    }
  },
  "languages": {
    "PHP": {
      "language_servers": ["rust-php-lsp", "..."]
    }
  }
}
```

Example binary paths:

- debug: `/Users/hotdogb/Work/rust-php/target/debug/rust-php`
- release: `/Users/hotdogb/Work/rust-php/target/release/rust-php`

## Current Feature Set

Working in the first slice:

- config key completion in common `config(...)` and `Config::...(...)` calls
- env key completion in `env(...)` calls
- route name completion in common `route(...)` helpers
- Laravel helper snippet completion in PHP and Blade, with context-aware insertion
- go to definition for config keys and named routes
- go to definition for env keys in `.env` / `.env.example`
- hover for config defaults/env values, env keys, and route details

Current limitations:

- route features only work for named routes the analyzer currently extracts
- the LSP index is rebuilt from on-disk project state at server startup
- unsaved Laravel route/config source files are not yet re-analyzed into the project index
