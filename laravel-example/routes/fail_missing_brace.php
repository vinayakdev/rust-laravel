<?php

use Illuminate\Support\Facades\Route;

Route::prefix('broken')->group(function () {
    Route::get('/missing-brace', function () {
        return 'oops';
    });

Route::get('/after-unclosed-group', function () {
    return 'still here';
});
