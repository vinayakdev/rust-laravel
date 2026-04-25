<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Http\Controllers\Concerns\PublishesContent;

class ReportController extends Controller
{
    use PublishesContent;

    // Accessible from outside: yes, standard route action.
    public function index(): string
    {
        return 'reports.index';
    }

    // Accessible from outside: yes, standard route action.
    public function export(): string
    {
        return 'reports.export';
    }

    // Accessible from outside: no, private implementation detail.
    private function compileDataset(): array
    {
        return [];
    }
}
