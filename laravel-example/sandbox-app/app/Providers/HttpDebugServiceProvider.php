<?php

namespace App\Providers;

use Illuminate\Support\Facades\Route;

class HttpDebugServiceProvider
{
    public function boot(): void
    {
        Route::aliasMiddleware('tenant', 'App\Http\Middleware\ResolveTenant');
        Route::aliasMiddleware('audit', 'App\Http\Middleware\AuditTrail');

        Route::middlewareGroup('web', [
            'bindings',
            'tenant',
        ]);

        Route::middlewareGroup('admin', [
            'web',
            'auth',
            'verified',
            'audit',
        ]);

        Route::pattern('slug', '[a-z0-9-]+');
    }
}
