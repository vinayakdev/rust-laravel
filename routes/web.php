<?php

use App\Http\Controllers\BlogController;
use App\Http\Controllers\CareersController;
use App\Http\Controllers\CategoryController;
use App\Http\Controllers\ContactController;
use App\Http\Controllers\ProductController;
use Illuminate\Support\Facades\Route;

// Auth::loginUsingId(1);

Route::get('/', function () {
    return view('welcome');
})->name('home');

Route::get('/products/{slug}', [ProductController::class, 'show'])->name('products.show');

Route::get('/categories', [CategoryController::class, 'index'])->name('categories.index');
Route::get('/categories/{slug}', [CategoryController::class, 'show'])->name('categories.show');
Route::get('/categories/{slug}', [CategoryController::class, 'lowmaintain'])->name('categories.poda');
Route::get('/categories/{slug}', [CategoryController::class, 'lowmaintain'])->name('categories.myrsddr');

Route::get('/blogs', [BlogController::class, 'index'])->name('blog.index');
Route::get('/blog/{slug}', [BlogController::class, 'show'])->name('blog.show');

Route::get('/careers', [CareersController::class, 'index'])->name('careers.index');
Route::get('/careers/{slug}', [CareersController::class, 'show'])->middleware('web')->name('careers.show');

Route::get('/contact', [ContactController::class, 'index'])->middleware('web')->name('contact.index');

/*
 |--------------------------------------------------------------------------
 | Parser Fixture: Multiple Route Declaration Styles
 |--------------------------------------------------------------------------
 |
 | This section intentionally contains a wide spread of Laravel route
 | declaration patterns so a PHP parser / analyzer can be tested against
 | closures, controller arrays, invokable controllers, legacy strings,
 | groups, resources, redirects, constraints, and chained modifiers.
 |
 */

Route::get('/fixture/basic-get', function () {
    return 'basic-get';
});

Route::post('/fixture/basic-post', function () {
    return 'basic-post';
});

Route::put('/fixture/basic-put', function () {
    return 'basic-put';
});

Route::patch('/fixture/basic-patch', function () {
    return 'basic-patch';
});

Route::delete('/fixture/basic-delete', function () {
    return 'basic-delete';
});

Route::options('/fixture/basic-options', function () {
    return response('', 204);
});

Route::match(['get', 'post'], '/fixture/match', function () {
    return 'match';
})->name('fixture.match');

Route::any('/fixture/any', function () {
    return 'any';
})->middleware(['web']);

Route::view('/fixture/view', 'fixture.view')->name('fixture.view');
Route::view('/fixture/view-data', 'fixture.view-data', ['section' => 'examples']);

Route::redirect('/fixture/from', '/fixture/to');
Route::permanentRedirect('/fixture/legacy', '/fixture/new-home');

Route::get('/fixture/controller-array', [BlogController::class, 'index']);
Route::get('/fixture/controller-array-fqcn', [\App\Http\Controllers\ExampleController::class, 'show']);
Route::post('/fixture/invokable', \App\Http\Controllers\SingleActionController::class);

Route::get('/fixture/legacy-controller-string', 'LegacyRouteController@index');
Route::post('/fixture/legacy-controller-string-store', 'LegacyRouteController@store');

Route::get('/fixture/where/{id}', function (string $id) {
    return $id;
})->where('id', '[0-9]+');

Route::get('/fixture/where-number/{id}', function (string $id) {
    return $id;
})->whereNumber('id');

Route::get('/fixture/where-alpha/{name}', function (string $name) {
    return $name;
})->whereAlpha('name');

Route::get('/fixture/where-alpha-dash/{slug}', function (string $slug) {
    return $slug;
})->whereAlphaNumeric('slug');

Route::get('/fixture/where-in/{category}', function (string $category) {
    return $category;
})->whereIn('category', ['books', 'games', 'music']);

Route::get('/fixture/where-uuid/{uuid}', function (string $uuid) {
    return $uuid;
})->whereUuid('uuid');

Route::get('/fixture/where-ulid/{ulid}', function (string $ulid) {
    return $ulid;
})->whereUlid('ulid');

Route::get('/fixture/optional/{slug?}', function (?string $slug = null) {
    return $slug ?? 'default';
});

Route::get('/fixture/chained/{id}', [ProductController::class, 'show'])
    ->whereNumber('id')
    ->middleware(['web', 'auth'])
    ->can('view-products')
    ->name('fixture.products.show');

Route::prefix('fixture/prefix')->group(function () {
    Route::get('/one', function () {
        return 'prefix-one';
    });

    Route::get('/two', function () {
        return 'prefix-two';
    })->name('fixture.prefix.two');
});

Route::middleware(['web', 'auth'])->group(function () {
    Route::get('/fixture/middleware-group/a', function () {
        return 'a';
    });

    Route::post('/fixture/middleware-group/b', function () {
        return 'b';
    });
});

Route::name('fixture.named.')->group(function () {
    Route::get('/fixture/named/one', function () {
        return 'one';
    })->name('one');

    Route::get('/fixture/named/two', function () {
        return 'two';
    })->name('two');
});

Route::prefix('fixture/admin')
    ->middleware(['web', 'auth', 'verified'])
    ->name('fixture.admin.')
    ->group(function () {
        Route::get('/dashboard', function () {
            return 'dashboard';
        })->name('dashboard');

        Route::controller(\App\Http\Controllers\Admin\ReportController::class)->group(function () {
            Route::get('/reports', 'index')->name('reports.index');
            Route::get('/reports/{report}', 'show')->name('reports.show');
            Route::post('/reports', 'store')->name('reports.store');
        });
    });

Route::controller(ContactController::class)->group(function () {
    Route::get('/fixture/controller-group/contact', 'index')->name('fixture.contact.index');
    Route::post('/fixture/controller-group/contact', 'store')->name('fixture.contact.store');
});

Route::group(['prefix' => 'fixture/array-group', 'as' => 'fixture.array.', 'middleware' => ['web']], function () {
    Route::get('/alpha', function () {
        return 'alpha';
    })->name('alpha');

    Route::get('/beta', [CategoryController::class, 'index'])->name('beta');
});

Route::domain('{account}.example.com')->group(function () {
    Route::get('/fixture/domain', function (string $account) {
        return $account;
    })->name('fixture.domain');
});

Route::scopeBindings()->group(function () {
    Route::get('/fixture/users/{user}/posts/{post}', [BlogController::class, 'show'])->name('fixture.scoped-bindings');
});

Route::withoutScopedBindings()->group(function () {
    Route::get('/fixture/unscoped/users/{user}/posts/{post}', [BlogController::class, 'show'])->name(
        'fixture.unscoped-bindings',
    );
});

Route::resources([
    'fixture/resource/posts' => \App\Http\Controllers\PostController::class,
    'fixture/resource/photos' => \App\Http\Controllers\PhotoController::class,
]);

Route::resource('fixture/resource/comments', \App\Http\Controllers\CommentController::class);
Route::resource('fixture/resource/tags', \App\Http\Controllers\TagController::class)->only(['index', 'store']);
Route::resource('fixture/resource/profiles', \App\Http\Controllers\ProfileController::class)->except(['destroy']);
Route::apiResource('fixture/api/articles', \App\Http\Controllers\ArticleController::class);

Route::apiResources([
    'fixture/api/videos' => \App\Http\Controllers\VideoController::class,
    'fixture/api/podcasts' => \App\Http\Controllers\PodcastController::class,
]);

Route::singleton('fixture/singleton/profile', \App\Http\Controllers\AccountProfileController::class);

Route::fallback(function () {
    return response('fallback', 404);
});
