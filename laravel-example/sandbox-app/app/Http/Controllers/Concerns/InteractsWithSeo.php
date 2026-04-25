<?php

namespace App\Http\Controllers\Concerns;

trait InteractsWithSeo
{
    // Accessible from outside: yes, public trait method becomes a controller action.
    public function seoDefaults(): array
    {
        return ['robots' => 'index,follow'];
    }

    // Accessible from outside: no, protected helper only supports public actions.
    protected function normalizeMetaTitle(string $title): string
    {
        return strtoupper($title);
    }
}
