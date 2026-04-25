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

## Marketplace Note

The current setup is valid for local dev-extension installs because Zed can load a
directory directly from [zed-extension](/Users/hotdogb/Work/rust-php/zed-extension).

For publishing to the public Zed extensions registry, plan on either:

1. moving the extension files so `extension.toml` is at the repository root, or
2. splitting [zed-extension](/Users/hotdogb/Work/rust-php/zed-extension) into its own repository

Zed's extension docs describe the published unit as a Git repository containing
`extension.toml`.

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

The extension should attach `rust-php-lsp` to `PHP` and `Blade` automatically.
For a normal install, users should only need the extension. The preferred flow is:

1. install the extension
2. let it download the matching `rust-php` binary from GitHub Releases
3. open a Laravel project in Zed

If the binary is already on `PATH`, the extension uses that first.

The only Zed setting that should remain optional is a custom binary path:

```json
{
  "lsp": {
    "rust-php-lsp": {
      "binary": {
        "path": "/absolute/path/to/rust-php"
      }
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

## Shipping This To Other People

To make installation simple, split distribution into two pieces:

1. Ship the Zed extension so the language server auto-registers for `PHP` and `Blade`.
2. Ship GitHub release assets for the `rust-php` binary so the extension can auto-install it.

Recommended binary distribution options:

- GitHub Releases with prebuilt binaries for `macOS`, `Linux`, and `Windows`
- `cargo install rust-php` if you also want a CLI install path outside Zed
- Homebrew tap for macOS/Linux users who do not want Rust installed

Today, the extension already supports this flow:

- first check `lsp.rust-php-lsp.binary.path`
- then check `rust-php` on `PATH`
- then download the matching binary from `vinayakdev/rust-laravel` releases

Manual `languages.*.language_servers` overrides should only be needed when a user
wants a custom ordering relative to other PHP/Blade servers.
