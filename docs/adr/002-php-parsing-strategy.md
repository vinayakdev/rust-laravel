# ADR 002 — PHP Parsing Strategy

## Status
Accepted

## Rule for Future AI / Contributors

> **Always use `php-parser` (the `bumpalo`-backed crate) when parsing PHP files.**
> Do not write regex-based PHP parsers or hand-rolled tokenizers for structured PHP.
> The only exception is the config-file extractor, which uses line-by-line scanning
> for performance on large config arrays — see the rationale below.

## Context

PHP is a complex language with closures, heredocs, namespaces, use-imports,
string interpolation, and many edge cases that make regex/line-by-line parsing
unreliable. We have three situations that need PHP parsing:

1. **Provider files** — well-formed class definitions. Full AST parse is correct and fast.
2. **Route files** — may be well-formed, or may contain closure bodies that the parser
   cannot handle in isolation when a file is snipped into chunks.
3. **Config files** — very large arrays of `'key' => env('KEY', 'default')` entries.
   Full AST parse works but is slow on files with thousands of keys.

## Decision

### Tier 1: Full AST Parse (`php-parser`)
Used for: providers, middleware, bootstrap/providers.php, provider source files.

These files are well-formed PHP classes. `php-parser` produces a complete, correct AST.
We use `bumpalo::Bump` as the arena allocator so all AST nodes are freed at once when
the `Bump` drops, with no per-node allocations.

### Tier 2: Full Parse with Chunk Fallback (route files)
Used for: route files in `routes/`.

Attempt a full parse first. If `php-parser` reports errors (e.g., closures with complex
bodies), fall back to `split_route_chunks()` which extracts each `Route::` statement as
a self-contained slice, sanitises closure bodies to `{}`, and re-parses each chunk.

**Why two tiers?** Route files often contain large closure bodies (middleware, controllers)
that fail partial parsing. Sanitising them to `{}` lets us still extract the route
declaration while discarding the unreachable action body.

### Tier 3: Line-by-Line Scanner (config files only)
Used for: `src/analyzers/configs/extractor.rs`.

Config arrays are predictable: `'key' => env('ENV_KEY', 'default'),`. The line-by-line
scanner is ~10× faster than a full AST parse for files with 500+ keys and handles
the limited syntax actually present. A stack tracks array nesting depth.

**When to switch back to full AST:** If config files start containing dynamic keys,
computed defaults, or complex PHP expressions, move to `php-parser`.

## Debugging PHP Parse Failures

If a file produces unexpected empty results or missing entries:

1. **Check `program.errors`** — the parser fills this slice when PHP is malformed.
   Log or print `errors` to see which tokens caused a failure.
2. **Print the raw source bytes** around the failure offset to identify the offending PHP.
3. **Use the chunk fallback** — if a route file fails full parse, `split_route_chunks`
   shows which statements were extracted. Add a debug print of `chunk.text` to trace.
4. **Verify PSR-4 mappings** — if a provider's source file is `None`, check
   `collect_psr4_mappings()` output. The class FQN must match a prefix in `autoload.psr-4`.
5. **Check `source_available`** — providers with `source_missing` status are silently
   skipped by middleware and config analyzers. This is intentional but can hide issues
   when a mapping is wrong.

## Consequences

- All structured PHP analysis is auditable through a single AST representation.
- Regex-based PHP "parsers" are banned from the codebase.
- Config extraction is fast but limited — do not extend it with more PHP syntax.
- When adding new analyzers, start with full AST parse; only optimise to line scanning
  if profiling shows it is needed.
