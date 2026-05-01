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
pub struct VendorClassDetail {
    pub fqn: String,
    pub file: String,
    pub methods: Vec<VendorMethod>,
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

    Some(VendorClassDetail {
        fqn: fqn.to_string(),
        file,
        methods,
    })
}
