use std::env;
use std::path::{Component, Path, PathBuf};

/// Path utilities for flux.
pub trait PathExt {
    /// Matches Python's Path.expanduser() behavior.
    /// Only expands if the first component is exactly '~'.
    fn expand_tilde(&self) -> PathBuf;
}

impl PathExt for Path {
    fn expand_tilde(&self) -> PathBuf {
        let mut components = self.components();

        // Peek at the first segment.
        match components.next() {
            // Only expand if the first segment is LITERALLY just "~"
            Some(Component::Normal(c)) if c == "~" => {
                let home = env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.to_path_buf()); // Fallback if HOME unset

                // Safely join the rest of the segments (the "suffix")
                home.join(components.as_path())
            }
            // For "aaaaa~aa.txt", the first component is the whole string.
            // It doesn't match the branch above, so it remains untouched.
            _ => self.to_path_buf(),
        }
    }
}

impl PathExt for PathBuf {
    fn expand_tilde(&self) -> PathBuf {
        self.as_path().expand_tilde()
    }
}

/// Standalone resolve utility.
pub fn resolve<P: AsRef<Path>>(path: P) -> PathBuf {
    path.as_ref().expand_tilde()
}
