<?php

namespace App\Http\Controllers;

abstract class Controller
{
    // Accessible from outside: yes, inherited public instance method.
    public function sharedHealthSummary(): array
    {
        return ['status' => 'ok'];
    }

    // Accessible from outside: no, protected helper for child controllers only.
    protected function bootSharedContext(): void
    {
    }

    // Accessible from outside: no, static helpers are not route controller actions.
    public static function maintenanceWindow(): string
    {
        return '02:00-03:00 UTC';
    }
}
