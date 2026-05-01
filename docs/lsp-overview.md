# LSP Overview — rust-php

## What this codebase does

**rust-php** is a Rust-based Language Server Protocol (LSP) server for Laravel/PHP projects. It analyzes a Laravel project's codebase — routes, controllers, models, views, migrations, middleware, configs, public assets — and provides IDE intelligence (completions, go-to-definition, hover docs, diagnostics, code actions) for editors like Zed.

It's structured as a Cargo workspace with specialized crates (`rust-php-routes`, `rust-php-models`, etc.), a `rust-php-editor` crate that builds a `ProjectIndex`, and a `rust-php-lsp` crate that wraps that into a JSON-RPC LSP server over stdio. There's also a `zed-extension` crate that wraps the binary for Zed's extension system.

---

## LSP Input points (messages received from the editor)

| Method | Handler |
|--------|---------|
| `initialize` | Sets project root, builds the `ProjectIndex` |
| `initialized` | No-op notification |
| `shutdown` | Sets `shutdown_requested` flag |
| `exit` | Sets `exiting` flag, breaks message loop |
| `textDocument/didOpen` | Caches document text, triggers reindex if relevant |
| `textDocument/didChange` | Updates document text, marks dirty (defers reindex) |
| `textDocument/didSave` | Clears dirty flag, validates PHP parse, triggers reindex |
| `textDocument/didClose` | Removes document from cache, triggers reindex |
| `textDocument/completion` | Detects completion context, returns items |
| `textDocument/definition` | Detects symbol/route context, returns locations |
| `textDocument/hover` | Detects symbol/route context, returns markdown docs |
| `textDocument/diagnostic` | Returns route diagnostics for the file |
| `textDocument/codeAction` | Returns route fix actions + asset actions |
| `workspace/executeCommand` | Executes `rust-php.openAssetInZed` |

**14 input message types total.**

---

## LSP Output points (responses sent back to the editor)

| Response | What it produces |
|----------|-----------------|
| `initialize` reply | Server capabilities declaration (`server.rs:537`) |
| `completion` reply | Completion items list — up to **11 context branches**: `livewire-component-tag`, `blade-component-tag`, `blade-component-attr`, `view-data`, `blade-variable`, `livewire-directive-value`, `builder-arg`, `vendor-chain`, `vendor-make`, `symbol`, `route-action`, `helper` |
| `definition` reply | Location array (4 contexts: livewire tag, blade tag, symbol, route-action) |
| `hover` reply | Markdown hover string (4 contexts: same as definition) |
| `diagnostic` reply | Array of route-related diagnostic items |
| `codeAction` reply | Array of code actions (route fixes + asset open actions) |
| `executeCommand` reply | Null on success, or an error object |

**7 response types**, with `completion` being the most complex (11 distinct context detectors).

The transport is vanilla LSP over stdio: `Content-Length: N\r\n\r\n<JSON>` framing in both directions (`server.rs:586–626`).
