# sandbox-app

Synthetic Laravel-style fixture app used for analyzer development.

This project exists only to test analyzer edge cases.

It is intentionally small and may not be runnable as a real Laravel app.

Covered scenarios:

- direct routes under `routes/`
- bootstrap provider registration
- app service providers
- local package source under `packages/`
- composer-discovered package metadata
- a declared package that is intentionally missing from `vendor/`
- config definitions using `env(...)`
- config merging via providers
- app models with relations, casts, scopes, accessors, mutators, and soft deletes
- conventional migrations with both named and anonymous classes
- migrations whose filenames and class names differ
- alter migrations that add and drop columns over time
- package-provided migrations loaded through `loadMigrationsFrom(...)`

This fixture lets us debug static analysis behavior without requiring Composer.
