@props(['title' => 'Sandbox'])

<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>{{ $title }}</title>
</head>
<body>
    <main>
        {{ $slot }}
    </main>
</body>
</html>
