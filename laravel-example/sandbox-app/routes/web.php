<?php

use Illuminate\Support\Facades\Route;

Route::get('/', function () {
    return 'sandbox';
})->name('sandbox.home');

Route::prefix('labs')->middleware('web')->group(function () {
    Route::get('/ping', function () {
        return 'pong';
    })->name('labs.ping');
});
