<?php

namespace App\Providers;

class AppServiceProvider
{
    public function register(): void
    {
        $this->mergeConfigFrom(__DIR__.'/../../config/debug.php', 'debug');
    }

    public function boot(): void
    {
        //
    }

    private function mergeConfigFrom(string $path, string $key): void
    {
        // Synthetic stand-in for Laravel's ServiceProvider::mergeConfigFrom.
    }
}
