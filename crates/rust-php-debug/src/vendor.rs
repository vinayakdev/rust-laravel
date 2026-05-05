use std::path::Path;

use rust_php_foundation::vendor as foundation_vendor;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VendorClass {
    pub fqn: String,
    pub file: String,
}

#[derive(Debug, Serialize)]
pub struct VendorMethod {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct VendorProperty {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct VendorClassDetail {
    pub fqn: String,
    pub file: String,
    pub methods: Vec<VendorMethod>,
    pub properties: Vec<VendorProperty>,
}

pub fn list_vendor_classes(project_root: &Path) -> Vec<VendorClass> {
    let mut classes: Vec<VendorClass> = foundation_vendor::load_classmap(project_root)
        .into_iter()
        .map(|(fqn, path)| VendorClass {
            fqn,
            file: path.display().to_string(),
        })
        .collect();
    classes.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    classes
}

pub fn class_detail(project_root: &Path, fqn: &str) -> Option<VendorClassDetail> {
    let classmap = foundation_vendor::load_classmap(project_root);
    let file = classmap.get(fqn)?.display().to_string();

    let methods = foundation_vendor::collect_chainable_methods_with_source(fqn, &classmap)
        .into_iter()
        .map(|(name, source)| VendorMethod { name, source })
        .collect();

    let properties = foundation_vendor::collect_class_properties_with_source(fqn, &classmap)
        .into_iter()
        .map(|(name, source)| VendorProperty { name, source })
        .collect();

    Some(VendorClassDetail {
        fqn: fqn.to_string(),
        file,
        methods,
        properties,
    })
}

#[derive(Debug, Serialize)]
pub struct ClassWithProperties {
    pub class_fqn: String,
    pub parent_fqn: String,
    pub file: String,
    pub properties: Vec<ClassPropEntry>,
}

#[derive(Debug, Serialize)]
pub struct ClassPropEntry {
    pub name: String,
    pub source_class: String,
}

/// Scan the vendor classmap for classes that extend another vendor class and
/// collect properties from the parent hierarchy. Useful for inspecting what
/// property completions will be offered in a given project.
pub fn list_class_properties(project_root: &Path) -> Vec<ClassWithProperties> {
    use std::fs;
    let classmap = foundation_vendor::load_classmap(project_root);
    let mut result: Vec<ClassWithProperties> = classmap
        .iter()
        .filter_map(|(fqn, path)| {
            let source = fs::read_to_string(path).ok()?;
            let namespace = foundation_vendor::parse_namespace_pub(&source);
            let use_map = foundation_vendor::parse_file_use_statements(&source);
            let parent_short = foundation_vendor::parse_extends_pub(&source)?;
            let parent_fqn =
                foundation_vendor::resolve_fqn(&parent_short, &namespace, &use_map);
            // Only include if parent is itself in the classmap (i.e. a vendor class)
            if !classmap.contains_key(&parent_fqn) {
                return None;
            }
            let properties = foundation_vendor::collect_class_properties_with_source(
                &parent_fqn,
                &classmap,
            )
            .into_iter()
            .map(|(name, source_class)| ClassPropEntry { name, source_class })
            .collect::<Vec<_>>();
            if properties.is_empty() {
                return None;
            }
            Some(ClassWithProperties {
                class_fqn: fqn.clone(),
                parent_fqn,
                file: path.display().to_string(),
                properties,
            })
        })
        .collect();
    result.sort_by(|a, b| a.class_fqn.cmp(&b.class_fqn));
    result
}
