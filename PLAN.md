# Roadmap

This document tracks the growth of `rust-php` from a CLI analyzer into a Laravel-aware editor/LSP backend.

## Working Rules

- Grow the project in small milestones.
- After each completed milestone:
  - update this plan
  - verify the change locally
  - create a Git commit
- Prefer static analysis first.
- Add runtime verification later as an optional layer, not as a hard dependency.
- Never require the user to run Composer just to get baseline analyzer output.

## Current Status

Implemented:

- `route:list`
- `route:list --json`
- `config:list`
- `config:list --json`
- `provider:list`
- `provider:list --json`
- `route:sources`
- route registration-source attribution
- `config:sources`
- config registration-source attribution
- terminal-friendly text tables
- basic malformed-route recovery
- basic provider graph
- synthetic Laravel-style fixture support under `laravel-example/`

Gaps:

- no tests yet
- no LSP transport yet

## Design Principles

### 1. Layered analysis

Use these layers:

1. filesystem scan
2. provider/package registration scan
3. effective model builder
4. optional runtime verification

### 2. Confidence levels

Every finding should eventually be marked as one of:

- `static_exact`
- `static_inferred`
- `source_missing`
- `runtime_needed`

### 3. Missing vendor support

Users may not have run Composer, so `vendor/` may be incomplete or absent.

Handling strategy:

- parse root `composer.json` anyway
- record discovered package/provider declarations even if package source is unavailable
- mark those findings as `source_missing`
- hide missing-source details in normal output if they create noise
- expose them in JSON and later in dedicated debug commands

Do not require:

- `composer install`
- autoload generation
- vendor class loading

## Milestones

## Milestone 1: Roadmap And Synthetic Fixture

Goal:

- create a clear implementation plan
- add a synthetic Laravel-style project for debugging analyzer behavior

Deliverables:

- `PLAN.md`
- `laravel-example/sandbox-app`
- examples for:
  - app providers
  - bootstrap provider registration
  - composer package discovery metadata
  - local package source present
  - declared package source missing

Status: `completed`

## Milestone 2: Provider Graph

Goal:

- index which providers can affect routes/config

Planned outputs:

- `provider:list`
- JSON provider graph

Sources to inspect:

- `bootstrap/providers.php`
- `composer.json`
- app provider classes
- package provider declarations

Questions to answer:

- which providers are app-local
- which providers are package-discovered
- which providers are declared but unresolved

Status: `completed`

## Milestone 3: Route Sources

Goal:

- attribute routes to their registration source, not only their final route file

Planned extraction:

- `Route::...` in route files
- `loadRoutesFrom(...)`
- provider boot logic that wraps route registration
- discovered package route files when source is available

Planned outputs:

- `route:list`
- `route:sources`
- JSON source graph

Status: `completed`

## Milestone 4: Config Sources

Goal:

- attribute config keys to:
  - direct file definitions
  - provider merges
  - env values
  - defaults

Planned extraction:

- `config/*.php`
- `mergeConfigFrom(...)`
- package config files
- env file values and substitutions
- visible runtime `config([...])` mutations where statically detectable

Planned outputs:

- `config:list`
- `config:sources`
- effective config model in JSON

Status: `completed`

## Milestone 5: Middleware And Route Enrichment

Goal:

- move from raw route extraction to effective route modeling

Planned extraction:

- middleware aliases
- middleware groups
- route patterns
- route model binding hints where statically visible

Planned outputs:

- richer `route:list`
- `middleware:list`

Status: `pending`

## Milestone 6: Verification And Diffing

Goal:

- compare static analyzer results with framework/runtime output when available

Optional integrations:

- `php artisan route:list --json`
- custom artisan commands for effective config dumps

Important:

- verification mode stays optional
- it must not block static analysis on machines without Composer/vendor

Status: `pending`

## Milestone 7: LSP-Oriented Core

Goal:

- make analyzer outputs easy to serve to an editor

Needed pieces:

- stable JSON shapes
- range-aware file references
- incremental file indexing
- diagnostics representation
- symbol/source attribution model

Potential LSP features:

- go to route definition
- hover for route/config source
- diagnostics for unresolved env keys/providers/packages
- completion for route names/config keys

Status: `pending`

## Debug Commands To Add

Short-term:

- `project:list`
- `provider:list`
- `route:sources`
- `config:sources`

Later:

- `middleware:list`
- `package:list`
- `analyzer:diff`

## Fixture Requirements

The synthetic fixture app should exercise:

- direct route files
- provider-loaded route files
- app config files using env defaults
- provider `mergeConfigFrom(...)`
- composer package discovery
- missing vendor package declarations
- local package source present without needing Composer

## Open Questions

- how much dynamic PHP execution do we tolerate before we stop and mark as `runtime_needed`?
- should normal text output hide unresolved package/provider entries unless `--debug` is used?
- do we want one synthetic app or several targeted fixtures?
