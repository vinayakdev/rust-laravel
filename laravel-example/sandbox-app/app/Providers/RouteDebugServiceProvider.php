<?php

namespace App\Providers;

class RouteDebugServiceProvider
{
    public function boot(): void
    {
        $this->loadRoutesFrom(__DIR__.'/../../routes/provider.php');
    }

    private function loadRoutesFrom(string $path): void
    {
        // Synthetic stand-in for Laravel's ServiceProvider::loadRoutesFrom.
    }
}
