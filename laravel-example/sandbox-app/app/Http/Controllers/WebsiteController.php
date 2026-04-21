<?php

namespace App\Http\Controllers;

use App\Http\Controllers\Concerns\InteractsWithSeo;
use App\Http\Controllers\Concerns\PublishesContent;

class WebsiteController extends BaseFrontendController
{
    use InteractsWithSeo;
    use PublishesContent;

    // Accessible from outside: yes, standard route action.
    public function home(): string
    {
        return 'home';
    }

    // Accessible from outside: yes, standard route action.
    public function about(): string
    {
        return 'about';
    }

    // Accessible from outside: yes, standard route action.
    public function contact(): string
    {
        return 'contact';
    }

    // Accessible from outside: no, protected methods should be flagged by debug output.
    protected function sustainability(): string
    {
        return 'sustainability';
    }

    // Accessible from outside: no, static methods are helpers, not controller actions.
    public static function docs(): string
    {
        return 'docs';
    }
}
