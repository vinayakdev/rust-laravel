<?php

namespace App\View\Components;

use Closure;
use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class ProfileCard extends Component
{
    public function __construct(
        public object $user,
        public string $status = 'active',
    ) {
    }

    public function badgeColor(): string
    {
        return match ($this->status) {
            'online' => 'green',
            'away' => 'yellow',
            default => 'slate',
        };
    }

    public function render(): View|Closure|string
    {
        return view('components.profile-card');
    }
}
