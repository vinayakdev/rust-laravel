use std::path::Path;

use rust_php::analyzers::{controllers, routes};
use rust_php::project;

fn sandbox_project() -> project::LaravelProject {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("laravel-example")
        .join("sandbox-app");
    project::from_root(root).expect("sandbox app should resolve")
}

#[test]
fn controller_report_flattens_class_trait_and_parent_methods() {
    let project = sandbox_project();
    let report = controllers::analyze(&project).expect("controller analysis should succeed");

    let website = report
        .controllers
        .iter()
        .find(|controller| controller.fqn == "App\\Http\\Controllers\\WebsiteController")
        .expect("WebsiteController should be discovered");

    assert_eq!(website.callable_method_count, 7);

    let team = website
        .methods
        .iter()
        .find(|method| method.name == "team")
        .expect("inherited team method should exist");
    assert!(team.accessible_from_route);
    assert_eq!(team.source_kind, "parent");

    let publish = website
        .methods
        .iter()
        .find(|method| method.name == "publish")
        .expect("trait publish method should exist");
    assert!(publish.accessible_from_route);
    assert_eq!(publish.source_kind, "trait");

    let sustainability = website
        .methods
        .iter()
        .find(|method| method.name == "sustainability")
        .expect("protected method should exist");
    assert!(!sustainability.accessible_from_route);
    assert_eq!(
        sustainability.accessibility,
        "protected methods are not callable from routes"
    );

    let docs = website
        .methods
        .iter()
        .find(|method| method.name == "docs")
        .expect("static method should exist");
    assert!(!docs.accessible_from_route);
    assert_eq!(
        docs.accessibility,
        "static methods are not callable as controller actions"
    );

    let blade_sandbox = report
        .controllers
        .iter()
        .find(|controller| controller.fqn == "App\\Http\\Controllers\\BladeSandboxController")
        .expect("BladeSandboxController should be discovered");

    let orders = blade_sandbox
        .methods
        .iter()
        .find(|method| method.name == "orders")
        .expect("orders method should exist");
    let order_vars = orders
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect::<Vec<_>>();
    assert!(order_vars.contains(&"pageTitle"));
    assert!(order_vars.contains(&"currentUser"));
    assert!(order_vars.contains(&"orders"));
    assert!(order_vars.contains(&"stats"));
    assert!(order_vars.contains(&"filters"));
    assert!(order_vars.contains(&"teamMembers"));
    assert!(order_vars.contains(&"breadcrumbs"));
    assert!(order_vars.contains(&"flashMessage"));
    assert!(order_vars.contains(&"internalAuditLog"));
    assert!(order_vars.contains(&"draftInvoice"));

    let components = blade_sandbox
        .methods
        .iter()
        .find(|method| method.name == "components")
        .expect("components method should exist");
    let component_vars = components
        .variables
        .iter()
        .map(|variable| variable.name.as_str())
        .collect::<Vec<_>>();
    assert!(component_vars.contains(&"pageTitle"));
    assert!(component_vars.contains(&"currentUser"));
    assert!(component_vars.contains(&"summary"));
    assert!(component_vars.contains(&"metrics"));
    assert!(component_vars.contains(&"tone"));
    assert!(component_vars.contains(&"hiddenExperiment"));
}

#[test]
fn route_report_flags_missing_and_inaccessible_controller_actions() {
    let project = sandbox_project();
    let report = routes::analyze(&project).expect("route analysis should succeed");

    let missing = report
        .routes
        .iter()
        .find(|route| route.name.as_deref() == Some("missingLanding"))
        .expect("missing route should exist");
    assert_eq!(
        missing
            .controller_target
            .as_ref()
            .expect("controller target should exist")
            .status,
        "missing-method"
    );

    let sustainability = report
        .routes
        .iter()
        .find(|route| route.name.as_deref() == Some("sustainability"))
        .expect("sustainability route should exist");
    assert_eq!(
        sustainability
            .controller_target
            .as_ref()
            .expect("controller target should exist")
            .status,
        "not-route-callable"
    );

    let team = report
        .routes
        .iter()
        .find(|route| route.name.as_deref() == Some("team"))
        .expect("team route should exist");
    let team_target = team
        .controller_target
        .as_ref()
        .expect("team target should exist");
    assert_eq!(team_target.status, "ok");
    assert_eq!(team_target.source_kind.as_deref(), Some("parent"));

    let health = report
        .routes
        .iter()
        .find(|route| route.name.as_deref() == Some("health"))
        .expect("health route should exist");
    assert_eq!(
        health
            .controller_target
            .as_ref()
            .expect("invokable controller target should exist")
            .method,
        "__invoke"
    );
}
