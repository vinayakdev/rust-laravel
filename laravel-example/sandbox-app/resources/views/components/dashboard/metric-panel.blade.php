<section>
    <h3>{{ $title }}</h3>
    <p>{{ $value }}</p>

    @if ($hasTrend())
        <small>{{ $trend }}</small>
    @endif
</section>
