<?php

use Illuminate\Support\Facades\Route;

Route::prefix('package')->group(function () {
    Route::get('/info', function () {
        return 'local-package';
    })->name('package.info');
});
