<?php

use Illuminate\Support\Facades\Route;

Route::get('/broken/missing-semicolon', function () {
    return 'missing semicolon'
});

Route::get('/broken/unclosed-array', [BrokenController::class, 'index');

Route::post('/broken/trailing-comma', function ($request,) {
    return 'trailing comma';
});
