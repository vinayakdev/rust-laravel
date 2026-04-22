<x-layouts.marketing :title="$pageTitle">
    <h1>{{ $pageTitle }}</h1>
    <p>{{ $summary->title }}</p>
    <p>{{ $summary->description }}</p>

    <x-ui.notice title="Parent scope" :tone="$tone">
        This anonymous component receives its props explicitly.
    </x-ui.notice>

    <x-ui.card>
        <p>Slot-only component with no declared props.</p>
        <p>Parent variables still belong to the parent view, not the component internals.</p>
    </x-ui.card>

    <x-profile-card :user="$currentUser" />

    @foreach ($metrics as $metric)
        <x-dashboard.metric-panel
            :title="$metric['title']"
            :value="$metric['value']"
            :trend="$metric['trend']"
        />
    @endforeach

    {{-- Available in this view: $pageTitle, $currentUser, $summary, $metrics, $tone --}}
    {{-- Not available in this view: $hiddenExperiment --}}
</x-layouts.marketing>
