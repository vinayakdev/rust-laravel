<?php

namespace Acme\LocalPackage;

class LocalPackageServiceProvider
{
    public function register(): void
    {
        $this->mergeConfigFrom(__DIR__.'/../config/local-package.php', 'local-package');
    }

    public function boot(): void
    {
        $this->loadRoutesFrom(__DIR__.'/../routes/local-package.php');
        $this->loadMigrationsFrom(__DIR__.'/../database/migrations');
    }

    private function mergeConfigFrom(string $path, string $key): void
    {
        // Synthetic stand-in for Laravel's ServiceProvider::mergeConfigFrom.
    }

    private function loadRoutesFrom(string $path): void
    {
        // Synthetic stand-in for Laravel's ServiceProvider::loadRoutesFrom.
    }

    private function loadMigrationsFrom(string $path): void
    {
        // Synthetic stand-in for Laravel's ServiceProvider::loadMigrationsFrom.
    }
}
