use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct FileOverrides {
    files: HashMap<PathBuf, String>,
}

impl FileOverrides {
    pub fn insert(&mut self, path: PathBuf, text: String) {
        self.files.insert(path, text);
    }

    pub fn get_string(&self, path: &Path) -> Option<String> {
        self.files.get(path).cloned()
    }

    pub fn get_bytes(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.get(path).map(|text| text.as_bytes().to_vec())
    }
}
