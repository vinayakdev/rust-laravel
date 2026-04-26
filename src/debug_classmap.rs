use std::path::Path;

use rust_php_foundation::vendor;

const PROJECT_ROOT: &str = "/Users/hotdogb/Work/rust-php/test/calcium";
const TARGET_CLASS: &str = "Filament\\Forms\\Components\\TextInput";

fn main() {
    let classmap = vendor::load_classmap(Path::new(PROJECT_ROOT));
    let all_methods =
        vendor::collect_chainable_methods_with_source(TARGET_CLASS, &classmap);

    println!("=== Chainable methods for {TARGET_CLASS} ===\n");
    for (method, source) in &all_methods {
        println!("  ->{}()  [from {}]", method, source);
    }
    println!("\nTotal: {}", all_methods.len());
}
