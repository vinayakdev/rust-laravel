<x-layouts.marketing :title="$pageTitle">
    <x-ui.notice title="Sync status" tone="success">
        {{ $flashMessage }}
    </x-ui.notice>

    <h1>{{ $pageTitle }}</h1>
    <p>Viewer: {{ $currentUser->name }} ({{ $currentUser->timezone }})</p>
    <p>Filter search: {{ $filters->search }}</p>

    <x-profile-card :user="$currentUser" status="online" />

    <div>
        <x-dashboard.metric-panel title="Open orders" :value="$stats['open_orders']" :trend="$stats['trend']" />
        <x-dashboard.metric-panel title="Revenue today" :value="$stats['revenue_today']" />
    </div>

    <x-ui.divider />

    <ul>
        @foreach ($orders as $order)
            @include('partials.order-row', ['order' => $order])
        @endforeach
    </ul>

    <h2>Team</h2>
    <ul>
        @foreach ($teamMembers as $member)
            <li>{{ $loop->iteration }}. {{ $member->name }} - {{ $member->role }}</li>
        @endforeach
    </ul>

    <p>Breadcrumbs: {{ implode(' / ', $breadcrumbs) }}</p>
    <p>Active tab from controller: {{ $activeTab }}</p>

    {{-- Available here: $pageTitle, $currentUser, $orders, $filters, $stats, $teamMembers, $breadcrumbs, $flashMessage, $activeTab --}}
    {{-- Not available here because they were never passed: $internalAuditLog, $draftInvoice --}}
</x-layouts.marketing>
