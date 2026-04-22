<x-layouts.marketing :title="$pageTitle">
    <h1>{{ $pageTitle }}</h1>
    <p>Signed in as {{ $currentUser->name }} ({{ $currentUser->role }})</p>

    <x-ui.divider />

    <ul>
        @foreach ($examples as $example)
            <li>
                <a href="{{ route($example['route']) }}">{{ $example['title'] }}</a>
                <span>{{ $example['description'] }}</span>
            </li>
        @endforeach
    </ul>

    <p>Breadcrumb trail: {{ implode(' / ', $breadcrumbs) }}</p>

    {{-- $internalNotes exists in the controller but is not passed to the view. --}}
</x-layouts.marketing>
