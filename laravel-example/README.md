Put Laravel projects in this directory.

Recommended layout:

```text
laravel-example/
  demo-app/
    app/
    config/
    routes/
```

Examples:

```bash
cargo run -- route:list --project demo-app
cargo run -- route:list --project demo-app --json
cargo run -- config:list --project demo-app
./target/release/rust-php route:list --project demo-app
```

Notes:

- `--project demo-app` resolves to `./laravel-example/demo-app`
- `--project ./some/other/path` uses that path directly
- if the current directory already looks like a Laravel project, the CLI uses it by default
