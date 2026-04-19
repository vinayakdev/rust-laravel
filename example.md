# Examples

This file shows what the tool does without requiring you to run it.

The examples below use the Laravel code currently stored under [laravel-example](/Users/hotdogb/Work/rust-php/laravel-example).

## Example Input

Project layout:

```text
laravel-example/
  .env
  config/
    app.php
    auth.php
    cache.php
    database.php
    filesystems.php
    logging.php
    mail.php
    media-library.php
    queue.php
    services.php
    session.php
  routes/
    console.php
    fail.php
    fail_missing_brace.php
    fail_tokens.php
    web.php
```

## Example Commands

| Command | Purpose |
| --- | --- |
| `cargo run -- route:list` | Show discovered Laravel routes in a readable table |
| `cargo run -- route:list --json` | Emit the same route data as JSON |
| `cargo run -- config:list` | Show config definitions, env usage, and env-file declarations |
| `cargo run -- config:list --json` | Emit normalized config data as JSON |

## Example Output: `route:list`

Input:

```bash
cargo run -- route:list
```

Output:

```text
routes/fail.php
  LINE:COL  METHOD    URI                                          NAME                         ACTION                                              MIDDLEWARE
  --------  --------  -------------------------------------------  ---------------------------  --------------------------------------------------  ----------
  5:18      GET       /broken/missing-semicolon                    -                            closure                                             -
  11:19     POST      /broken/trailing-comma                       -                            closure                                             -

routes/fail_missing_brace.php
  LINE:COL  METHOD    URI                                          NAME                         ACTION                                              MIDDLEWARE
  --------  --------  -------------------------------------------  ---------------------------  --------------------------------------------------  ----------
  6:22      GET       /broken/missing-brace                        -                            closure                                             -
  10:18     GET       /after-unclosed-group                        -                            closure                                             -

routes/web.php
  LINE:COL  METHOD    URI                                          NAME                         ACTION                                              MIDDLEWARE
  --------  --------  -------------------------------------------  ---------------------------  --------------------------------------------------  ----------
  16:12     GET       /products/{slug}                             products.show                ProductController@show                              -
  18:12     GET       /categories                                  categories.index             CategoryController@index                            -
  19:12     GET       /categories/{slug}                           categories.show              CategoryController@show                             -
  27:12     GET       /careers/{slug}                              careers.show                 CareersController@show                              web
  67:14     GET|POST  /fixture/match                               fixture.match                closure                                             -
```

What this shows:

- route file path
- `Line:Column` for quick navigation
- HTTP method(s)
- resolved URI
- route name if present
- action/controller target
- middleware collected from chains/groups

## Example Output: `route:list --json`

Input:

```bash
cargo run -- route:list --json
```

Output:

```json
{
  "project_name": "laravel-example",
  "project_root": "/Users/hotdogb/Work/rust-php/laravel-example",
  "route_count": 53,
  "routes": [
    {
      "file": "routes/fail.php",
      "line": 5,
      "column": 18,
      "methods": ["GET"],
      "uri": "/broken/missing-semicolon",
      "name": null,
      "action": "closure",
      "middleware": []
    },
    {
      "file": "routes/fail.php",
      "line": 11,
      "column": 19,
      "methods": ["POST"],
      "uri": "/broken/trailing-comma",
      "name": null,
      "action": "closure",
      "middleware": []
    }
  ]
}
```

What this shows:

- the CLI can act as a machine-readable backend
- every route has `file`, `line`, and `column`
- this shape is suitable for future editor/LSP integration

## Example Output: `config:list`

Input:

```bash
cargo run -- config:list
```

Output:

```text
.env
  LINE:COL  KIND        KEY
  --------  ----------  ----------------------------------------------------------------
  1:1       env-file    APP_NAME
  2:1       env-file    APP_ENV
  3:1       env-file    APP_KEY

config/app.php
  LINE:COL  KIND        KEY
  --------  ----------  ----------------------------------------------------------------
  16:5      definition  app.name
  16:15     env-usage   APP_NAME
  29:5      definition  app.env
  29:14     env-usage   APP_ENV
  42:5      definition  app.debug
  42:23     env-usage   APP_DEBUG
  55:5      definition  app.url
  55:14     env-usage   APP_URL
```

What this shows:

- `.env` and `.env.example` declarations
- normalized config keys from `config/*.php`
- referenced env key, default value, and resolved env value
- `Line:Column` for every item
- long cells are truncated with `…` on smaller terminals
- color in the real terminal view highlights env state

## Example Output: `config:list --json`

Input:

```bash
cargo run -- config:list --json
```

Output:

```json
{
  "project_name": "laravel-example",
  "project_root": "/Users/hotdogb/Work/rust-php/laravel-example",
  "item_count": 431,
  "items": [
    {
      "file": "config/app.php",
      "line": 122,
      "column": 9,
      "key": "app.maintenance.driver",
      "env_key": "APP_MAINTENANCE_DRIVER",
      "default_value": "file",
      "env_value": "axis"
    },
    {
      "file": "config/app.php",
      "line": 123,
      "column": 9,
      "key": "app.maintenance.store",
      "env_key": "APP_MAINTENANCE_STORE",
      "default_value": "database",
      "env_value": null
    }
  ]
}
```

What this shows:

- the config analyzer is now structured enough for extension/LSP consumption
- each config item carries the resolved env-backed view of the setting

## Why This File Exists

This project is meant to evolve into Rust-powered Laravel debugging and editor tooling.

`example.md` gives you:

- a static explanation of what the commands do
- a known-good example shape for future refactors
- something easy to link from the README or extension docs
