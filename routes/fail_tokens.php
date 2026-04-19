<?php

use Illuminate\Support\Facades\Route;

Route::get('/broken/bad-variable', function () {
    $value = ;
    return $value;
});

Route::get('/broken/double-arrow', function () {
    return => 'bad';
});

Route::get('/broken/if', function () {
    if (true {
        return 'missing paren';
    }
});
