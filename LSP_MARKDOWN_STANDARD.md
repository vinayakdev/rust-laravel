# LSP Markdown Standard

This file shows the intended markdown shape for hover and completion documentation across the LSP surface.

## Asset Hover

Resolved:

```md
## logo.svg

---

Size: `1.50 KiB`

File: [open file](file:///Users/you/project/public/assets/logo.svg)
```

Missing:

```md
## logo.svg

---

Status: `missing`
```

Unresolved:

```md
## assets://logo.svg

---

Status: `unresolved`
```

## Asset Completion

```md
## logo.svg

---

Path: `public/assets/logo.svg`

Extension: `.svg`

Size: `1.50 KiB`

Usages: `2`
```

## Config Hover

```md
## app.debug

---

Current value: `true`

Env key: `APP_DEBUG`

Default: `false`

Resolved env: `true`
```

## Route Hover

```md
## home

---

Methods: `GET, HEAD`

Uri: `/`

Action: `App\Http\Controllers\HomeController@index`

Middleware: `web`

Parameter patterns: `id=[0-9]+`

Source: [routes/web.php:15:13](file:///Users/you/project/routes/web.php)
```

## Env Hover

```md
## APP_URL

---

Value: `http://localhost`

Source: `.env:1:1`
```

## View Hover

```md
## dashboard

---

Kind: `blade`

File: `resources/views/dashboard.blade.php`

Usages: `3`

Props: `title, user`
```

## Controller Hover

```md
## App\Http\Controllers\UserController

---

Callable methods: `2`

Total methods: `8`

Source: `app/Http/Controllers/UserController.php:14`

Extends: `App\Http\Controllers\Controller`

Traits: `Illuminate\Foundation\Auth\AuthenticatesUsers`
```

## Controller Method Hover

```md
## UserController::store

---

Controller: `App\Http\Controllers\UserController`

Route callable: `true`

Visibility: `public`

Source kind: `method`

Notes: `public`

Source: `app/Http/Controllers/UserController.php:52`
```

## Blade Component Hover

```md
## x-user-card

---

Class: [`UserCard`](file:///Users/you/project/app/View/Components/UserCard.php)

Blade file: [resources/views/components/user-card.blade.php](file:///Users/you/project/resources/views/components/user-card.blade.php)

Props: `title = Guest, user`
```

## Blade Prop Completion

```md
## title

---

Default: `Guest`
```

Required prop:

```md
## title

---

Status: `required`
```

## Blade Variable Completion

```md
## $user

---

Available in the current Blade view
```

## View Data Variable Completion

```md
## $user

---

Available as a local variable in the current controller method
```
