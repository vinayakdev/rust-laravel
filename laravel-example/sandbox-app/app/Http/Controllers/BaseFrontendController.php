<?php

namespace App\Http\Controllers;

abstract class BaseFrontendController extends Controller
{
    // Accessible from outside: yes, inherited public instance method.
    public function team(): string
    {
        return 'team';
    }

    // Accessible from outside: no, protected helper used by page actions.
    protected function pageMeta(): array
    {
        return ['title' => 'frontend'];
    }
}
