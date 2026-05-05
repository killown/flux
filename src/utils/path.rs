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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_expand_tilde_config_path() {
        // We mock HOME to ensure the test is deterministic across different machines.
        // If we used the "real" user, the test might pass on your machine but fail
        // on a CI server or another developer's machine with different permissions.
        let original_home = env::var_os("HOME");
        let mock_home = "/home/developer";
        env::set_var("HOME", mock_home);

        let path = PathBuf::from("~/.config/flux/config.toml");
        assert_eq!(
            path.expand_tilde(),
            PathBuf::from("/home/developer/.config/flux/config.toml")
        );

        if let Some(home) = original_home {
            env::set_var("HOME", home);
        }
    }

    #[test]
    fn test_expand_tilde_real_user_fallback() {
        // This test verifies that if we DON'T mock, it correctly picks up the actual environment.
        let real_home = env::var_os("HOME").map(PathBuf::from);

        if let Some(home_path) = real_home {
            let path = PathBuf::from("~/Downloads");
            let expected = home_path.join("Downloads");
            assert_eq!(path.expand_tilde(), expected);
        }
    }

    #[test]
    fn test_expand_tilde_no_expansion_scenarios() {
        // Real-world cases where a tilde exists but should NOT be expanded.
        let path = PathBuf::from("/home/user/Documents/notes.txt~");
        assert_eq!(
            path.expand_tilde(),
            PathBuf::from("/home/user/Documents/notes.txt~")
        );

        let path = PathBuf::from("/var/www/html/site~backup/index.html");
        assert_eq!(
            path.expand_tilde(),
            PathBuf::from("/var/www/html/site~backup/index.html")
        );

        let path = PathBuf::from("/etc/flux~/settings.conf");
        assert_eq!(
            path.expand_tilde(),
            PathBuf::from("/etc/flux~/settings.conf")
        );
    }

    #[test]
    fn test_relative_path_integrity() {
        let path = PathBuf::from("./local/file.txt");
        assert_eq!(path.expand_tilde(), PathBuf::from("./local/file.txt"));

        let path = PathBuf::from("../parent/file.txt");
        assert_eq!(path.expand_tilde(), PathBuf::from("../parent/file.txt"));
    }

    #[test]
    fn test_expand_tilde_lone_tilde() {
        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", "/home/developer");

        let path = PathBuf::from("~");
        assert_eq!(path.expand_tilde(), PathBuf::from("/home/developer"));

        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        }
    }
}
