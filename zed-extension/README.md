# rust-laravel Zed Extension

Dev extension for running the `rust-php` language server inside Zed.

Expected binary:

- `rust-php lsp`

The extension auto-attaches to both `PHP` and `Blade`.

Zed can either:

- find `rust-php` on your `PATH`
- or automatically download the matching binary from GitHub Releases for supported platforms
- or use `lsp.rust-php-lsp.binary.path` from your Zed settings

For people installing this normally, the goal should be:

1. install the extension
2. let it download `rust-php` automatically, or install `rust-php` somewhere on `PATH`
3. open a Laravel project in Zed

In that setup, no `languages.PHP.language_servers` or `languages.Blade.language_servers`
override is required.

Optional override when you want to pin a custom binary path:

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

GitHub release source:

- `https://github.com/vinayakdev/rust-laravel`
