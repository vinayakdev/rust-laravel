@props([
    'title',
    'tone' => 'info',
    'actionUrl' => null,
])

<section data-tone="{{ $tone }}">
    <strong>{{ $title }}</strong>
    <div>{{ $slot }}</div>

    @if ($actionUrl)
        <a href="{{ $actionUrl }}">Open</a>
    @endif

    {{-- Available in this component: $title, $tone, $actionUrl, $slot, $attributes --}}
    {{-- Parent variables are not automatically available unless passed as props. --}}
</section>
