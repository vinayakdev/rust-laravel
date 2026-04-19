<?php

use Illuminate\Support\Facades\Route;

Route::prefix('provider')->group(function () {
    Route::get('/status', function () {
        return 'ok';
    })->name('provider.status');
});
