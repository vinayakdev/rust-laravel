<?php

namespace App\Http\Controllers\Concerns;

trait PublishesContent
{
    // Accessible from outside: yes, public trait method becomes a controller action.
    public function publish(): string
    {
        return 'published';
    }

    // Accessible from outside: no, protected trait helper is not route-callable.
    protected function previewDraft(): string
    {
        return 'preview';
    }

    // Accessible from outside: no, private trait helper stays internal.
    private function recordEditorialAudit(): void
    {
    }
}
