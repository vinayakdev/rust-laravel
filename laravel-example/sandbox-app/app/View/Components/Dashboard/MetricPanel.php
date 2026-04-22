<?php

namespace App\View\Components\Dashboard;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class MetricPanel extends Component
{
    public function __construct(
        public string $title,
        public int|float|string $value,
        public ?string $trend = null,
    ) {
    }

    public function hasTrend(): bool
    {
        return $this->trend !== null && $this->trend !== '';
    }

    public function render(): View|Closure|string
    {
        return view('components.dashboard.metric-panel');
    }
}
