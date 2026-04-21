<?php

use App\Http\Controllers\Admin\ReportController;
use App\Http\Controllers\HealthCheckController;
use App\Http\Controllers\WebsiteController;

Route::get('/', [WebsiteController::class, 'home'])->name('home');
Route::get('/about', [WebsiteController::class, 'about'])->name('about');
Route::get('/contact', [WebsiteController::class, 'contact'])->name('contact');
Route::get('/sustainability', [WebsiteController::class, 'sustainability'])->name('sustainability');
Route::get('/team', [WebsiteController::class, 'team'])->name('team');
Route::get('/publish', [WebsiteController::class, 'publish'])->name('publish');
Route::get('/seo-defaults', [WebsiteController::class, 'seoDefaults'])->name('seo-defaults');
Route::get('/missing', [WebsiteController::class, 'missingLanding'])->name('missingLanding');
Route::get('/docs', [WebsiteController::class, 'docs'])->name('docs');
Route::get('/health', HealthCheckController::class)->name('health');
Route::get('/reports', [ReportController::class, 'index'])->name('reports.index');
Route::get('/reports/export', [ReportController::class, 'export'])->name('reports.export');
