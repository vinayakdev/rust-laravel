<x-layouts.marketing :title="$pageTitle">
    <h1>{{ $pageTitle }}</h1>
    <p>Owner: {{ $owner->name }} ({{ $owner->email }})</p>

    <ul>
        @foreach ($visibleProjects as $project)
            <li>{{ $project->name }} - {{ $project->status }}</li>
        @endforeach
    </ul>

    <x-ui.notice title="Visibility" tone="warning">
        The controller created extra variables that are intentionally not available here.
    </x-ui.notice>

    {{-- Available in this view: $pageTitle, $owner, $visibleProjects --}}
    {{-- Not available in this view: $internalToken, $queryContext, $neverPassed --}}
</x-layouts.marketing>
