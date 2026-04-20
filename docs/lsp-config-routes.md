# LSP Plan For Config And Route Support

This project already has the hard part in place: static analyzers that produce normalized route and config records with file, line, column, source attribution, and resolved defaults/env values.

For the first LSP slice, we should not build a new parser. We should add an LSP-facing index/query layer on top of the existing analyzers.

## What We Already Have

### Config analyzer

`src/analyzers/configs/*` already gives us:

- normalized config keys like `app.name`
- source file, line, and column
- provider merge attribution via `mergeConfigFrom(...)`
- env key
- default value when statically visible
- resolved env value from `.env` / `.env.example`

### Route analyzer

`src/analyzers/routes/*` already gives us:

- method + URI + name + action
- source file, line, and column
- registration source attribution
- provider-loaded route files
- effective middleware and route parameter patterns

That is enough to power:

- completion
- go to definition
- hover

## LSP Features Mapped To Current Engine

### 1. Config completion

Target contexts:

- `config('app.`...`)`
- `Config::get('app.`...`)`
- `env('APP_`...`)` later, as a follow-up

How it works:

1. analyze the Laravel project with `analyzers::configs::analyze(...)`
2. build an in-memory index keyed by config key
3. when completion is requested inside a config-key string, filter keys by prefix
4. return completion items with:
   - `label`: full key
   - `detail`: default value or env binding summary
   - `documentation`: source file + registration source
   - `kind`: `Value` or `Property`

Useful extra fields from current data:

- `default_value`
- `env_key`
- `env_value`
- `source.provider_class`

### 2. Route completion

Target contexts:

- `route('home')`
- `to_route('dashboard')`
- `redirect()->route('profile.show')`
- `Route::has('...')`

How it works:

1. analyze the Laravel project with `analyzers::routes::analyze(...)`
2. index routes by name
3. when completion is requested inside a route-name string, filter named routes by prefix
4. return completion items with:
   - `label`: route name
   - `detail`: methods + URI
   - `documentation`: action + source file
   - `kind`: `Reference`

Unnamed routes do not participate in route-name completion.

### 3. Go To Definition

Config:

- resolve the string under cursor to a config key
- look up the `ConfigItem`
- jump to `file + line + column`

Routes:

- resolve the string under cursor to a route name
- look up the `RouteEntry`
- jump to the route declaration location

Important behavior:

- if multiple config items or route registrations map to the same symbol, return multiple locations
- prefer exact key/name matches first

### 4. Hover

Config hover should show:

- full key
- default value
- env key
- resolved env value if present
- registration source

Route hover should show:

- route name
- methods
- URI
- action
- middleware
- parameter patterns
- registration source

This is already mostly present in `ConfigItem` and `RouteEntry`.

## Required New Layer

We need a new module, separate from CLI rendering:

- `src/lsp/mod.rs`
- `src/lsp/index.rs`
- `src/lsp/query.rs`
- `src/lsp/context.rs`

Responsibility split:

- analyzers: scan Laravel projects and emit normalized facts
- LSP index: cache and organize those facts for editor queries
- LSP query layer: answer completion/definition/hover requests
- transport later: wire the query layer into actual LSP JSON-RPC

## Index Shape

The first index can stay simple and full-project:

- `config_by_key: BTreeMap<String, Vec<ConfigItem>>`
- `route_by_name: BTreeMap<String, Vec<RouteEntry>>`
- `routes: Vec<RouteEntry>`
- `configs: Vec<ConfigItem>`

We should also store:

- project root
- indexed-at timestamp or generation counter
- source file dependency set for later invalidation

## Cursor Context Detection

The first version does not need a full PHP semantic model for editor buffers.

A pragmatic first pass is enough:

- inspect the current line / local text around the cursor
- detect whether the cursor is inside:
  - `config('...')`
  - `Config::get('...')`
  - `route('...')`
  - `to_route('...')`
  - `redirect()->route('...')`

That gives us fast completion and definition support without solving generic PHP symbol analysis yet.

## Why This Fits The Existing Engine

This codebase already stores the fields an LSP needs:

- exact source locations in `src/types.rs`
- provider/source attribution
- env/default metadata for config
- normalized route names and URIs

So the engine remains the source of truth, and the LSP layer becomes a query adapter over it.

## Implementation Order

### Phase 1

- add `src/lsp/` with project index types
- add builders from `ConfigReport` and `RouteReport`
- add pure query functions:
  - `complete_config_keys(...)`
  - `complete_route_names(...)`
  - `goto_config_key(...)`
  - `goto_route_name(...)`
  - `hover_config_key(...)`
  - `hover_route_name(...)`

### Phase 2

- add a small CLI/debug command that exercises the LSP query layer without a real editor
- test against `laravel-example/sandbox-app`
- verify duplicate-name and package-merged cases

### Phase 3

- add actual LSP transport
- support reindex-on-change
- support open-buffer overlays for unsaved files

## Expected First User Experience

For config:

- typing `config('app.` suggests keys from local config and merged package config
- hover on `config('app.name')` shows default/env details
- go to definition opens `config/app.php` at the key

For routes:

- typing `route('` suggests named routes
- hover on `route('dashboard')` shows methods + URI + action
- go to definition opens the route file or provider-loaded route file at the declaration

## Constraints

- config extraction is intentionally heuristic, not full PHP evaluation
- route-name features only work for named routes
- provider/package source may be missing when Composer sources are unavailable
- initial indexing can be whole-project; incremental invalidation comes after correctness
