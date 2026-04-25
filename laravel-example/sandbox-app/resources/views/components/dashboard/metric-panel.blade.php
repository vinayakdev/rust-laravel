<section>
    <h3>{{ $title }}</h3>
    <p>{{ $value }}</p>

    {{ $polamyre $ad }}

    @if ($hasTrend())
        <small>{{ $trend }}</small>
    @endif
</section>
