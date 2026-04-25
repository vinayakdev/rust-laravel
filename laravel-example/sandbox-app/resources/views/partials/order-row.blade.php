<li>
    {{ $order->number }} - {{ $order->customer }} - {{ $order->total }} - {{ $order->status }}
    <em>tab={{ $activeTab }}</em>

    {{-- Includes inherit the parent view scope, so $activeTab is visible here even though only $order was explicitly passed. --}}
    {{-- Variables never exposed by the controller, such as $internalAuditLog, are still unavailable here. --}}
</li>
