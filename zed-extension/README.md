# rust-php Zed Extension

Dev extension for running the `rust-php` language server inside Zed.

Expected binary:

- `rust-php lsp`

Zed can either:

- find `rust-php` on your `PATH`
- or use `lsp.rust-php-lsp.binary.path` from your Zed settings

Example Zed settings:

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
