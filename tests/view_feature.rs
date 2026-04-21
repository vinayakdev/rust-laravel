use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rust_php::analyzers::views;
use rust_php::project;

fn unique_temp_project_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    std::env::temp_dir().join(format!("rust-php-view-fixtures-{nonce}"))
}

fn write_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should exist");
    }
    fs::write(path, contents).expect("fixture file should be written");
}

fn build_view_fixture_project() -> project::LaravelProject {
    let root = unique_temp_project_root();
    fs::create_dir_all(&root).expect("fixture root should exist");

    write_file(
        &root,
        "composer.json",
        r#"{
  "autoload": {
    "psr-4": {
      "App\\": "app/"
    }
  }
}"#,
    );

    write_file(
        &root,
        "config/app.php",
        r#"<?php

return [
    'providers' => [
        App\Providers\ViewFixtureServiceProvider::class,
    ],
];
"#,
    );

    write_file(
        &root,
        "routes/web.php",
        r#"<?php

use Illuminate\Support\Facades\Route;

Route::view('/debug/views/welcome', 'welcome', ['routeFlag' => true])->name('debug.views.welcome');
Route::view('/debug/views/missing', 'pages::home', ['section' => 'hero'])->name('debug.views.missing');
"#,
    );

    write_file(
        &root,
        "app/Support/ViewDebugFixtures.php",
        r#"<?php

namespace App\Support;

use Illuminate\Contracts\View\View as ViewContract;
use Illuminate\Support\Facades\View;

class ViewDebugFixtures
{
    public function helper(): ViewContract
    {
        return view(
            'welcome',
            compact(
                'headline',
                'cta'
            ),
            ['version' => 2]
        );
    }

    public function facadeMake(): ViewContract
    {
        return View::make('demo::page', ['posts' => $posts], compact('filters'));
    }

    public function facadeFirst(): ViewContract
    {
        return View::first(['missing-page', 'welcome'], ['status' => 'fallback']);
    }

    public function responseView()
    {
        return response()->view('demo::page', compact('subject'));
    }

    public function missingHelper(): ViewContract
    {
        return view('ghost.page', ['ghost' => true]);
    }
}
"#,
    );

    write_file(
        &root,
        "app/Providers/ViewFixtureServiceProvider.php",
        r#"<?php

namespace App\Providers;

use Illuminate\Support\Facades\Blade;
use Livewire\Livewire;

class ViewFixtureServiceProvider
{
    public function boot(): void
    {
        $this->loadViewsFrom(__DIR__.'/../../module/resources/views', 'demo');

        Blade::component('demo::card', \App\View\Components\DemoCard::class);
        Livewire::component('demo.panel', \App\Livewire\DemoPanel::class);
    }

    private function loadViewsFrom(string $path, string $namespace): void
    {
        // Synthetic stand-in for Laravel's ServiceProvider::loadViewsFrom.
    }
}
"#,
    );

    write_file(
        &root,
        "app/View/Components/DemoCard.php",
        r#"<?php

namespace App\View\Components;

use Illuminate\Contracts\View\View;
use Illuminate\View\Component;

class DemoCard extends Component
{
    public function render(): View
    {
        return view('demo::components.card');
    }
}
"#,
    );

    write_file(
        &root,
        "app/Livewire/DemoPanel.php",
        r#"<?php

namespace App\Livewire;

use Illuminate\Contracts\View\View;
use Livewire\Component;

class DemoPanel extends Component
{
    public function render(): View
    {
        return view('demo::livewire.panel');
    }
}
"#,
    );

    write_file(&root, "resources/views/welcome.blade.php", "<div>Welcome</div>\n");
    write_file(&root, "module/resources/views/page.blade.php", "<div>Page</div>\n");
    write_file(
        &root,
        "module/resources/views/components/card.blade.php",
        "<div>Card</div>\n",
    );
    write_file(
        &root,
        "module/resources/views/livewire/panel.blade.php",
        "<div>Panel</div>\n",
    );

    project::from_root(root).expect("fixture project should resolve")
}

fn run_with_large_stack<T>(f: impl FnOnce() -> T + Send + 'static) -> T
where
    T: Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(f)
        .expect("thread should start")
        .join()
        .expect("thread should finish")
}

#[test]
fn view_report_collects_real_world_view_entry_points_and_missing_targets() {
    let report = run_with_large_stack(|| {
        let project = build_view_fixture_project();
        views::analyze(&project).expect("view analysis should succeed")
    });

    let welcome = report
        .views
        .iter()
        .find(|view| view.name == "welcome")
        .expect("welcome view should exist");
    assert!(welcome.variables.iter().any(|variable| variable.name == "headline"));
    assert!(welcome.variables.iter().any(|variable| variable.name == "cta"));
    assert!(welcome.variables.iter().any(|variable| variable.name == "version"));
    assert!(welcome.variables.iter().any(|variable| variable.name == "status"));
    assert!(welcome.variables.iter().any(|variable| variable.name == "routeFlag"));
    assert!(welcome.usages.iter().any(|usage| usage.kind == "view-call"));
    assert!(welcome.usages.iter().any(|usage| usage.kind == "view-facade-first"));
    assert!(welcome.usages.iter().any(|usage| usage.kind == "route-view"));

    let page = report
        .views
        .iter()
        .find(|view| view.name == "demo::page")
        .expect("demo::page should exist");
    assert!(page.variables.iter().any(|variable| variable.name == "posts"));
    assert!(page.variables.iter().any(|variable| variable.name == "filters"));
    assert!(page.variables.iter().any(|variable| variable.name == "subject"));
    assert!(page.usages.iter().any(|usage| usage.kind == "view-facade-make"));
    assert!(page.usages.iter().any(|usage| usage.kind == "response-view"));

    let missing_route = report
        .missing_views
        .iter()
        .find(|view| view.name == "pages::home")
        .expect("pages::home should be tracked as missing");
    assert_eq!(
        missing_route.expected_file,
        Path::new("resources/views/vendor/pages/home.blade.php")
    );
    assert!(missing_route
        .usages
        .iter()
        .any(|usage| usage.kind == "route-view"));

    let ghost = report
        .missing_views
        .iter()
        .find(|view| view.name == "ghost.page")
        .expect("ghost.page should be tracked as missing");
    assert_eq!(ghost.expected_file, Path::new("resources/views/ghost/page.blade.php"));
    assert!(ghost.usages.iter().any(|usage| usage.kind == "view-call"));

    let fallback = report
        .missing_views
        .iter()
        .find(|view| view.name == "missing-page")
        .expect("View::first should preserve missing candidates");
    assert!(fallback
        .usages
        .iter()
        .any(|usage| usage.kind == "view-facade-first"));
}

#[test]
fn view_report_resolves_namespaced_component_view_files_from_load_views_from() {
    let report = run_with_large_stack(|| {
        let project = build_view_fixture_project();
        views::analyze(&project).expect("view analysis should succeed")
    });

    let demo_card = report
        .blade_components
        .iter()
        .find(|component| {
            component.component == "demo::card" && component.kind == "blade-class-manual"
        })
        .expect("demo::card component should exist");
    assert_eq!(
        demo_card.view_file.as_deref(),
        Some(Path::new("module/resources/views/components/card.blade.php"))
    );

    let demo_panel = report
        .livewire_components
        .iter()
        .find(|component| component.component == "demo.panel")
        .expect("demo.panel component should exist");
    assert_eq!(
        demo_panel.view_file.as_deref(),
        Some(Path::new("module/resources/views/livewire/panel.blade.php"))
    );
}
