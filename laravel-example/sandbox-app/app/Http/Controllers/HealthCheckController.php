<?php

namespace App\Http\Controllers;

class HealthCheckController extends Controller
{
    // Accessible from outside: yes, invokable controllers are valid route targets.
    public function __invoke(): string
    {
        return 'ok';
    }

    // Accessible from outside: no, protected helper should not be offered as a route action.
    protected function heartbeat(): string
    {
        return 'heartbeat';
    }
}
