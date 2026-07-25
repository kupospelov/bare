use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

pub struct Directory {
    path: PathBuf,
}

impl Directory {
    pub fn new() -> Self {
        let name = format!(
            "bare-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);

        fs::create_dir(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, path: impl AsRef<Path>, contents: &str) {
        let p = self.path.join(path.as_ref());
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        fs::write(p, contents).unwrap();
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
