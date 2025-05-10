use std::collections::HashMap;
use std::path::{Path, PathBuf};

// VirtualFileSystem represents an in-memory file system
#[derive(Debug, Default)]
pub struct VirtualFileSystem {
    pub(crate) files: HashMap<PathBuf, Vec<u8>>,
}

impl Clone for VirtualFileSystem {
    fn clone(&self) -> Self {
        let mut new_files = HashMap::new();
        for (path, content) in &self.files {
            new_files.insert(path.clone(), content.clone());
        }
        Self { files: new_files }
    }
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: PathBuf, content: Vec<u8>) {
        self.files.insert(path, content);
    }

    pub fn get_file(&self, path: &Path) -> Option<&Vec<u8>> {
        self.files.get(path)
    }

    pub fn get_paths(&self) -> Vec<PathBuf> {
        self.files.keys().cloned().collect()
    }
}
