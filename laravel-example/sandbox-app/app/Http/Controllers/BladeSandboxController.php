<?php

namespace App\Http\Controllers;

use Illuminate\Contracts\View\View;

class BladeSandboxController extends Controller
{
    public $nameBoi;

    public function index(): View
    {
        $pageTitle = 'Blade IDE Sandbox';
        $currentUser = (object) [
            'name' => 'Maya Chen',
            'email' => 'maya@example.test',
            'role' => 'Operations Lead',
            'timezone' => 'Asia/Kolkata',
        ];
        $breadcrumbs = ['Home', 'Blade Sandbox'];
        $examples = [
            [
                'title' => 'Orders Overview',
                'route' => 'blade-sandbox.orders',
                'description' => 'Controller variables, compact, merged arrays, includes, loops, and component props.',
            ],
            [
                'title' => 'Components Showcase',
                'route' => 'blade-sandbox.components',
                'description' => 'Anonymous and class components, with props, without props, slots, and aware values.',
            ],
            [
                'title' => 'Missing Variables',
                'route' => 'blade-sandbox.missing',
                'description' => 'Variables created in the controller but intentionally not exposed to the Blade file.',
            ],
        ];
        $internalNotes = ['never exported'];

        return view('ide-lab.index', compact('currentUser', 'examples', 'pageTitle'))->with(
            'breadcrumbs',
            $breadcrumbs,
        );
    }

    public function orders(): View
    {
        $pageTitle = 'Orders Overview';
        $currentUser = (object) [
            'name' => 'Maya Chen',
            'email' => 'maya@example.test',
            'role' => 'Operations Lead',
            'timezone' => 'Asia/Kolkata',
        ];
        $orders = [
            (object) ['number' => 'SO-1001', 'customer' => 'Acme Retail', 'total' => 1490, 'status' => 'paid'],
            (object) ['number' => 'SO-1002', 'customer' => 'Northwind Studio', 'total' => 320, 'status' => 'pending'],
            (object) ['number' => 'SO-1003', 'customer' => 'Delta Foods', 'total' => 880, 'status' => 'refunded'],
        ];
        $stats = [
            'open_orders' => 12,
            'revenue_today' => 2690,
            'trend' => '+8.4%',
        ];
        $filters = (object) [
            'status' => 'all',
            'search' => 'SO-10',
        ];
        $teamMembers = [
            (object) ['name' => 'Nora', 'role' => 'Support'],
            (object) ['name' => 'Dev', 'role' => 'Finance'],
        ];
        $breadcrumbs = ['Home', 'Orders'];
        $flashMessage = '3 orders synced from ERP';
        $internalAuditLog = ['secret' => true];
        $draftInvoice = (object) ['number' => 'DRAFT-44'];

        return view(
            'ide-lab.orders',
            compact('pageTitle', 'currentUser', 'orders', 'filters')
            + [
                'breadcrumbs' => $breadcrumbs,
                'flashMessage' => $flashMessage,
            ],
        )->with([
            'stats' => $stats,
            'teamMembers' => $teamMembers,
            'activeTab' => 'orders',
        ]);
    }

    public function components(): View
    {
        $pageTitle = 'Components Showcase';
        $currentUser = (object) [
            'name' => 'Maya Chen',
            'email' => 'maya@example.test',
            'role' => 'Operations Lead',
            'timezone' => 'Asia/Kolkata',
        ];
        $summary = (object) [
            'title' => 'Quarterly pipeline',
            'description' => 'Anonymous and class components receiving data through props.',
        ];
        $metrics = [
            ['title' => 'Open tickets', 'value' => 14, 'trend' => '+2'],
            ['title' => 'Resolved today', 'value' => 31, 'trend' => '+11'],
        ];
        $tone = 'info';
        $hiddenExperiment = 'not passed to the view';

        return view('ide-lab.components', compact('pageTitle', 'currentUser', 'summary', 'metrics'))->with(
            'tone',
            $tone,
        );
    }

    public function missing(): View
    {
        $pageTitle = 'Missing Variables';
        $owner = (object) [
            'name' => 'Arjun Patel',
            'email' => 'arjun@example.test',
        ];
        $visibleProjects = [
            (object) ['name' => 'Apollo', 'status' => 'active'],
            (object) ['name' => 'Beacon', 'status' => 'planning'],
        ];
        $internalToken = 'sandbox-secret';
        $queryContext = ['debug' => true];
        $neverPassed = (object) ['value' => 'hidden'];

        return view('ide-lab.missing-data', [
            'pageTitle' => $pageTitle,
            'visibleProjects' => $visibleProjects,
        ])->with('owner', $owner);
    }
}
