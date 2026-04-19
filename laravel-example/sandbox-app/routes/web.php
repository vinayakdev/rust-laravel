<?php

use Illuminate\Support\Facades\Route;

Route::get('/', function () {
    return 'sandbox';
})->name('sandbox.home');

Route::prefix('labs')
    ->middleware('web')
    ->group(function () {
        Route::get('/ping', function () {
            return 'pong';
        })->name('labs.ping');
    });

Route::prefix('admin')
    ->middleware('admin')
    ->group(function () {
        Route::get('/products/{slug}', function (string $slug) {
            return $slug;
        })->middleware('auth')->name('admin.products.show');
    });

require __DIR__ . '/starter.php';
