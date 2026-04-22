<article data-badge-color="{{ $badgeColor() }}">
    <h2>{{ $user->name }}</h2>
    <p>{{ $user->email }}</p>
    <p>{{ $user->role }}</p>
    <p>Status: {{ $status }}</p>

    {{-- Available in this class component view: $user, $status, component methods like $badgeColor() --}}
    {{-- Parent-only variables such as $pageTitle are not available unless passed down. --}}
</article>
