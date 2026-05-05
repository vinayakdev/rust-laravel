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
    let vendor_index = foundation_vendor::VendorClassIndex::load(project_root);
    let mut classes: Vec<VendorClass> = vendor_index
        .classmap()
        .iter()
        .map(|(fqn, path)| VendorClass {
            fqn: fqn.clone(),
            file: path.display().to_string(),
        })
        .collect();
    classes.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    classes
}

pub fn class_detail(project_root: &Path, fqn: &str) -> Option<VendorClassDetail> {
    let vendor_index = foundation_vendor::VendorClassIndex::load(project_root);
    let file = vendor_index.class_path(fqn)?.display().to_string();

    let methods = vendor_index
        .collect_chainable_methods_with_source(fqn)
        .into_iter()
        .map(|(name, source)| VendorMethod { name, source })
        .collect();

    let properties = vendor_index
        .collect_class_properties_with_source(fqn)
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
