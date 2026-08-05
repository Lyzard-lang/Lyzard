use std::path::{Path, PathBuf};

use super::version::Version;

pub struct PackageCache {
    root: PathBuf,
}

impl PackageCache {
    /// Standard location: ~/.lyzard/cache/
    pub fn default_location() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PackageCache {
            root: PathBuf::from(home).join(".lyzard").join("cache"),
        }
    }

    /// For tests — use a custom (typically temp) directory
    pub fn at(root: impl Into<PathBuf>) -> Self {
        PackageCache { root: root.into() }
    }

    fn package_dir(&self, name: &str, version: &Version) -> PathBuf {
        self.root.join(format!("{}-{}", name, version))
    }

    /// Is this exact name+version already cached locally?
    pub fn is_cached(&self, name: &str, version: &Version) -> bool {
        self.package_dir(name, version).is_dir()
    }

    /// Store downloaded package source into the cache.
    /// `files` is a list of (relative_path, content) pairs — mimics what
    /// extracting a downloaded archive would produce.
    pub fn store(
        &self,
        name: &str,
        version: &Version,
        files: &[(String, String)],
    ) -> std::io::Result<()> {
        let dir = self.package_dir(name, version);
        std::fs::create_dir_all(&dir)?;
        for (rel_path, content) in files {
            let full_path = dir.join(rel_path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(full_path, content)?;
        }
        Ok(())
    }

    /// Read a cached package's file (returns None if not cached or file missing)
    pub fn read_file(&self, name: &str, version: &Version, rel_path: &str) -> Option<String> {
        let path = self.package_dir(name, version).join(rel_path);
        std::fs::read_to_string(path).ok()
    }

    /// Full path to a cached package's directory (for compiling against it)
    pub fn path_for(&self, name: &str, version: &Version) -> PathBuf {
        self.package_dir(name, version)
    }

    /// Remove a specific cached version (used by `lyzard cache clean`)
    pub fn evict(&self, name: &str, version: &Version) -> std::io::Result<()> {
        let dir = self.package_dir(name, version);
        if dir.is_dir() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Total size in bytes of everything cached (for `lyzard cache size`)
    pub fn total_size(&self) -> u64 {
        fn dir_size(path: &Path) -> u64 {
            let mut total = 0;
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_dir() {
                            total += dir_size(&entry.path());
                        } else {
                            total += meta.len();
                        }
                    }
                }
            }
            total
        }
        dir_size(&self.root)
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn temp_cache() -> (PackageCache, tempfile_dir::TempDir) {
        let dir = tempfile_dir::TempDir::new();
        let cache = PackageCache::at(dir.path());
        (cache, dir)
    }

    // Minimal temp-dir helper (avoids adding the `tempfile` crate as a dependency
    // for this learning-focused MVP — swap for the real crate in production)
    mod tempfile_dir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicUsize, Ordering};

        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        pub struct TempDir(PathBuf);
        impl TempDir {
            pub fn new() -> Self {
                let id = COUNTER.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("lyzard_test_{}_{}", std::process::id(), id));
                let _ = std::fs::create_dir_all(&path);
                TempDir(path)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }
        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn test_not_cached_initially() {
        let (cache, _dir) = temp_cache();
        let v = Version::new(1, 0, 0);
        assert!(!cache.is_cached("foo", &v));
    }

    #[test]
    fn test_store_and_is_cached() {
        let (cache, _dir) = temp_cache();
        let v = Version::new(1, 0, 0);
        cache
            .store("foo", &v, &[("main.lyz".to_string(), "fn f() {}".to_string())])
            .unwrap();
        assert!(cache.is_cached("foo", &v));
    }

    #[test]
    fn test_read_stored_file() {
        let (cache, _dir) = temp_cache();
        let v = Version::new(1, 0, 0);
        cache
            .store("foo", &v, &[("main.lyz".to_string(), "fn f() {}".to_string())])
            .unwrap();
        let content = cache.read_file("foo", &v, "main.lyz").unwrap();
        assert_eq!(content, "fn f() {}");
    }

    #[test]
    fn test_read_missing_file_none() {
        let (cache, _dir) = temp_cache();
        let v = Version::new(1, 0, 0);
        assert!(cache.read_file("foo", &v, "missing.lyz").is_none());
    }

    #[test]
    fn test_evict_removes_package() {
        let (cache, _dir) = temp_cache();
        let v = Version::new(1, 0, 0);
        cache
            .store("foo", &v, &[("a.lyz".to_string(), "x".to_string())])
            .unwrap();
        assert!(cache.is_cached("foo", &v));
        cache.evict("foo", &v).unwrap();
        assert!(!cache.is_cached("foo", &v));
    }

    #[test]
    fn test_different_versions_dont_collide() {
        let (cache, _dir) = temp_cache();
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);
        cache
            .store("foo", &v1, &[("a.lyz".to_string(), "v1".to_string())])
            .unwrap();
        cache
            .store("foo", &v2, &[("a.lyz".to_string(), "v2".to_string())])
            .unwrap();
        assert_eq!(cache.read_file("foo", &v1, "a.lyz").unwrap(), "v1");
        assert_eq!(cache.read_file("foo", &v2, "a.lyz").unwrap(), "v2");
    }
}

#[cfg(test)]
mod registry_tests {
    use super::super::registry::*;
    use super::super::version::Version;

    #[test]
    fn test_fake_registry_list_versions() {
        let reg = FakeRegistry::new()
            .publish("json", "1.0.0", vec![], vec![])
            .publish("json", "1.5.0", vec![], vec![]);
        let versions = reg.list_versions("json");
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_fake_registry_unknown_package_empty() {
        let reg = FakeRegistry::new();
        assert!(reg.list_versions("nonexistent").is_empty());
    }

    #[test]
    fn test_fake_registry_get_metadata() {
        let reg = FakeRegistry::new().publish("json", "1.0.0", vec![("core", "^1.0.0")], vec![]);
        let meta = reg.get_metadata("json", &Version::new(1, 0, 0)).unwrap();
        assert_eq!(meta.dependencies.len(), 1);
    }

    #[test]
    fn test_fake_registry_download() {
        let reg = FakeRegistry::new()
            .publish("json", "1.0.0", vec![], vec![("src/main.lyz", "fn f() {}")]);
        let files = reg.download("json", &Version::new(1, 0, 0)).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "src/main.lyz");
    }

    #[test]
    fn test_fake_registry_download_missing_errors() {
        let reg = FakeRegistry::new();
        assert!(reg.download("nothing", &Version::new(1, 0, 0)).is_err());
    }
}
